pub mod backup;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::error::{Error, Result};
use crate::l10n::{Msg, t};
use crate::link::{LinkStrategy, create_dir_link, remove_linked_or_dir};
use crate::model::change::{ApplyResult, ChangeSet, OpAction};
use crate::paths;
use crate::platform;
use crate::store::Db;
use backup::{BackupSession, ManifestEntry, load_manifest};

/// 执行一个变更计划；任一操作失败即回滚本批次已完成的步骤。
pub fn apply(cs: &ChangeSet, roots: &[PathBuf], cfg: &AppConfig, db: &Db) -> Result<ApplyResult> {
    if cs.is_empty() {
        return Ok(ApplyResult {
            batch_id: cs.id.clone(),
            applied: 0,
            skipped: 0,
            backup_dir: None,
            log: vec![t!(Msg::BatchEmptyPlan)],
        });
    }

    let mut session = BackupSession::create()?;
    let mut applied: Vec<String> = Vec::new();
    let mut done = 0usize;

    for op in &cs.ops {
        let result = apply_one(op, roots, cfg, db, &mut session);
        match result {
            Ok(line) => {
                done += 1;
                applied.push(line.clone());
                if applied.len() == 1 {
                    // 第一条成功后才写日志，避免零操作也留痕
                }
                let _ = session.save();
            }
            Err(err) => {
                let mut log = applied.clone();
                log.push(t!(Msg::BatchFailedAt, desc = op.describe(), error = err));
                let undo = rollback_manifest(&session, roots);
                log.extend(undo);
                let _ = session.save();
                return Err(Error::msg(log.join("\n")));
            }
        }
    }

    session.save()?;
    db.log_batch(
        &cs.id,
        cs.kind.slug(),
        &cs.title,
        cs.ops.len(),
        Some(&session.dir),
    )?;
    backup::prune(cfg.backups_keep.max(1));

    Ok(ApplyResult {
        batch_id: cs.id.clone(),
        applied: done,
        skipped: cs.ops.len().saturating_sub(done),
        backup_dir: Some(session.dir.clone()),
        log: applied,
    })
}

fn apply_one(
    op: &crate::model::change::FileOp,
    roots: &[PathBuf],
    cfg: &AppConfig,
    db: &Db,
    session: &mut BackupSession,
) -> Result<String> {
    match &op.action {
        OpAction::CreateLink {
            at,
            target,
            strategy,
        } => {
            paths::ensure_within(roots, at)?;
            create_dir_link(target, at, *strategy)?;
            db.record_link(at, target, strategy.slug())?;
            session.record(ManifestEntry::CreatedLink {
                at: at.clone(),
                target: target.clone(),
            });
            Ok(t!(
                Msg::BatchCreateLink,
                strategy = strategy.label(),
                at = paths::path_str(at)
            ))
        }

        OpAction::ReplaceWithLink {
            at,
            target,
            strategy,
        } => {
            paths::ensure_within(roots, at)?;
            if platform::is_link_like(at) {
                // 已存在的链接（含 Windows junction）没有可保留的内容，直接摘掉。
                // 不能走下面的 rename/copy_tree：junction 会被当成实体目录遍历，
                // 把 store 里的真身复制进备份。
                let link_target = platform::link_target(at);
                let _ = db.forget_link(at);
                platform::remove_link(at)?;
                session.record(ManifestEntry::Removed {
                    at: at.clone(),
                    backup: None,
                    trashed: false,
                    link_target,
                });
            } else if std::fs::symlink_metadata(at).is_ok() {
                // 原件整体移入备份目录（同卷 rename，成本接近 0）
                let stash = session.dir.join("replaced").join(format!(
                    "{}-{}",
                    session.entries().len(),
                    at.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "item".into())
                ));
                if let Some(parent) = stash.parent() {
                    paths::ensure_dir(parent)?;
                }
                if std::fs::rename(at, &stash).is_err() {
                    crate::link::copy_tree(at, &stash)?;
                    remove_linked_or_dir(at, false)?;
                }
                session.record(ManifestEntry::Moved {
                    from: at.clone(),
                    to: stash,
                });
            }
            create_dir_link(target, at, *strategy)?;
            db.record_link(at, target, strategy.slug())?;
            session.record(ManifestEntry::CreatedLink {
                at: at.clone(),
                target: target.clone(),
            });
            Ok(t!(Msg::BatchReplaceLink, at = paths::path_str(at)))
        }

        OpAction::MoveIntoStore { from, to } => {
            paths::ensure_within(roots, from)?;
            paths::ensure_within(roots, to)?;
            if to.exists() {
                return Err(Error::Conflict(t!(
                    Msg::ErrStoreExists,
                    name = to.display()
                )));
            }
            if let Some(parent) = to.parent() {
                paths::ensure_dir(parent)?;
            }
            // 防御：链接搬进仓库会变成坏链或指向外部，直接拒绝。
            // Windows 的 junction 不是 symlink，必须用平台判定，否则会把目标内容复制成第二份。
            if platform::is_link_like(from) {
                return Err(Error::Conflict(t!(
                    Msg::ErrMoveLinkRefused,
                    at = from.display()
                )));
            }
            match std::fs::rename(from, to) {
                Ok(()) => {}
                Err(_) => {
                    crate::link::copy_tree(from, to)?;
                    remove_linked_or_dir(from, false)?;
                }
            }
            session.record(ManifestEntry::Moved {
                from: from.clone(),
                to: to.clone(),
            });
            Ok(t!(
                Msg::BatchMove,
                from = paths::path_str(from),
                to = paths::path_str(to)
            ))
        }

        OpAction::RemovePath { at, use_trash } => {
            paths::ensure_within(roots, at)?;
            let is_link = platform::is_link_like(at);
            if is_link {
                // 链接没有内容可备份：记下原指向，回滚时重建，否则「删除可回滚」在这条路上不成立
                let link_target = platform::link_target(at);
                db.forget_link(at)?;
                remove_linked_or_dir(at, false)?;
                session.record(ManifestEntry::Removed {
                    at: at.clone(),
                    backup: None,
                    trashed: false,
                    link_target,
                });
                return Ok(t!(Msg::BatchRemoveLink, at = paths::path_str(at)));
            }
            if *use_trash {
                // 先留一份备份，再交给系统回收站：`trash::delete` 成功之后文件就不在我们
                // 手里了，此时若 `backup: None`，回滚只能重建链接、无法还原真身 —— 结果是
                // 一堆指向空路径的断链（2026-11-02 macOS 实测）。技能目录通常只有几 KB，
                // 用一次复制换「删除可回滚」是划算的；进回收站的那份仍可由用户在 Finder
                // / 资源管理器里「放回原处」。
                let stash = session.stash(at)?;
                match trash::delete(at) {
                    Ok(()) => {
                        session.record(ManifestEntry::Removed {
                            at: at.clone(),
                            backup: stash,
                            trashed: true,
                            link_target: None,
                        });
                        return Ok(t!(Msg::BatchTrashed, at = paths::path_str(at)));
                    }
                    Err(err) => {
                        // 无 GUI 会话 / Finder AppleEvent 超时：直接用手上这份备份，保证仍可回滚
                        remove_linked_or_dir(at, false)?;
                        session.record(ManifestEntry::Removed {
                            at: at.clone(),
                            backup: stash,
                            trashed: false,
                            link_target: None,
                        });
                        return Ok(t!(
                            Msg::BatchTrashUnavailable,
                            error = err,
                            at = paths::path_str(at)
                        ));
                    }
                }
            }
            let stash = session.stash(at)?;
            remove_linked_or_dir(at, false)?;
            session.record(ManifestEntry::Removed {
                at: at.clone(),
                backup: stash,
                trashed: false,
                link_target: None,
            });
            Ok(t!(Msg::BatchDelete, at = paths::path_str(at)))
        }

        OpAction::WriteFile {
            at,
            content,
            backup,
        } => {
            paths::ensure_within(roots, at)?;
            let previous_existed = at.exists();
            let stash = if *backup && cfg.adoption.backup_config_files {
                session.stash(at)?
            } else {
                None
            };
            paths::write_atomic(at, content.as_bytes())?;
            session.record(ManifestEntry::Written {
                at: at.clone(),
                backup: stash,
                previous_existed,
            });
            Ok(t!(Msg::BatchWrite, at = paths::path_str(at)))
        }
    }
}

/// 回滚到某个批次之前的状态。
pub fn rollback(batch_id: &str, roots: &[PathBuf], db: &Db) -> Result<Vec<String>> {
    let dir = db
        .batch_backup_dir(batch_id)?
        .ok_or_else(|| Error::NotFound(t!(Msg::ErrBatchNoBackup, batch = batch_id)))?;
    let entries = load_manifest(&dir)?;
    let mut log = Vec::new();
    for entry in entries.iter().rev() {
        match entry {
            ManifestEntry::CreatedLink { at, .. } => {
                paths::ensure_within(roots, at)?;
                let _ = db.forget_link(at);
                remove_linked_or_dir(at, false)?;
                log.push(t!(Msg::RollbackRemoveLink, at = paths::path_str(at)));
            }
            ManifestEntry::Moved { from, to } => {
                if to.exists() {
                    if let Some(parent) = from.parent() {
                        paths::ensure_dir(parent)?;
                    }
                    if from.exists() {
                        let _ = remove_linked_or_dir(from, false);
                    }
                    if std::fs::rename(to, from).is_err() {
                        crate::link::copy_tree(to, from)?;
                        remove_linked_or_dir(to, false)?;
                    }
                    log.push(t!(Msg::RollbackRestore, at = paths::path_str(from)));
                }
            }
            ManifestEntry::Removed {
                at,
                backup,
                trashed,
                link_target,
            } => {
                // 备份优先：进回收站的那一步也会先 stash 一份，所以 `trashed` 也有据可回滚。
                // 老批次（trashed 但没有 backup）只能提示用户自己去回收站放回。
                if let Some(src) = backup {
                    restore(src, at)?;
                    log.push(if *trashed {
                        t!(
                            Msg::RollbackRestoredBackupTrashKept,
                            at = paths::path_str(at)
                        )
                    } else {
                        t!(Msg::RollbackRestoredBackup, at = paths::path_str(at))
                    });
                } else if *trashed {
                    log.push(t!(Msg::RollbackManualTrash, at = paths::path_str(at)));
                } else if let Some(target) = link_target {
                    recreate_link(target, at)?;
                    log.push(t!(Msg::RollbackRelink, at = paths::path_str(at)));
                }
            }
            ManifestEntry::Written {
                at,
                backup,
                previous_existed,
            } => match backup {
                Some(src) => {
                    restore(src, at)?;
                    log.push(t!(Msg::RollbackRestoreContent, at = paths::path_str(at)));
                }
                None => {
                    if !*previous_existed && at.exists() {
                        std::fs::remove_file(at).map_err(|e| Error::io(at, e))?;
                        log.push(t!(Msg::RollbackRemoveCreated, at = paths::path_str(at)));
                    }
                }
            },
        }
    }
    db.mark_undone(batch_id)?;
    Ok(log)
}

/// 回滚时按记录的指向重建链接；已存在则先摘掉（幂等）。
fn recreate_link(target: &Path, at: &Path) -> Result<()> {
    if platform::is_link_like(at) {
        let _ = platform::remove_link(at);
    } else if std::fs::symlink_metadata(platform::long_path(at)).is_ok() {
        let _ = remove_linked_or_dir(at, false);
    }
    if let Some(parent) = at.parent() {
        paths::ensure_dir(parent)?;
    }
    create_dir_link(target, at, LinkStrategy::platform_default())
}

fn restore(src: &Path, dst: &Path) -> Result<()> {
    if platform::is_link_like(src) {
        // 备份里若是链接（历史批次），按链接还原；直接 copy_tree 只会得到一个空目录
        let target = std::fs::read_link(platform::long_path(src)).map_err(|e| Error::io(src, e))?;
        if dst.exists() || std::fs::symlink_metadata(dst).is_ok() {
            let _ = remove_linked_or_dir(dst, false);
        }
        return create_dir_link(&target, dst, LinkStrategy::platform_default())
            .or_else(|_| create_dir_link(&target, dst, LinkStrategy::Copy));
    }
    if std::fs::symlink_metadata(src)
        .map(|m| m.is_file())
        .unwrap_or(false)
    {
        if let Some(parent) = dst.parent() {
            paths::ensure_dir(parent)?;
        }
        std::fs::copy(src, dst).map_err(|e| Error::io(dst, e))?;
        return Ok(());
    }
    if dst.exists() {
        let _ = remove_linked_or_dir(dst, false);
    }
    crate::link::copy_tree(src, dst)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorIssue {
    pub level: String,
    pub path: PathBuf,
    pub message: String,
    /// 问题分类，UI 据此决定给哪个快捷操作：
    /// `broken_link` / `foreign_link` / `duplicate_copy` / `duplicate_group` / `store_missing`
    #[serde(default)]
    pub kind: String,
    /// `duplicate_copy` / `duplicate_group` 的技能名，用于直接生成收敛计划
    #[serde(default)]
    pub subject: Option<String>,
}

fn rollback_manifest(session: &BackupSession, roots: &[PathBuf]) -> Vec<String> {
    let mut log = Vec::new();
    for entry in session.entries().iter().rev() {
        match entry {
            ManifestEntry::CreatedLink { at, .. } => {
                if paths::ensure_within(roots, at).is_ok() {
                    let _ = remove_linked_or_dir(at, false);
                    log.push(t!(Msg::RollbackUndoRemoveLink, at = paths::path_str(at)));
                }
            }
            ManifestEntry::Moved { from, to } => {
                if to.exists() {
                    let _ = std::fs::rename(to, from);
                    log.push(t!(Msg::RollbackUndoRestore, at = paths::path_str(from)));
                }
            }
            ManifestEntry::Removed {
                at,
                backup,
                link_target,
                ..
            } => {
                if let Some(src) = backup {
                    let _ = restore(src, at);
                    log.push(t!(Msg::RollbackUndoRecover, at = paths::path_str(at)));
                } else if let Some(target) = link_target {
                    // 链接没有内容可备份，只能按原指向重建（策略取平台默认）
                    if recreate_link(target, at).is_ok() {
                        log.push(t!(Msg::RollbackUndoRelink, at = paths::path_str(at)));
                    }
                }
            }
            ManifestEntry::Written {
                at,
                backup,
                previous_existed,
            } => {
                if let Some(src) = backup {
                    let _ = restore(src, at);
                } else if !*previous_existed {
                    let _ = std::fs::remove_file(at);
                }
                log.push(t!(Msg::RollbackUndoWrite, at = paths::path_str(at)));
            }
        }
    }
    log
}
