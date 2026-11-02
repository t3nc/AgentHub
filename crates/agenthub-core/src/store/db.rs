use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::Result;
use crate::model::change::BatchRecord;
use crate::model::skill::SkillRecord;
use crate::paths;

/// 当前库结构版本。加新迁移时 +1，并在 `migrate_locked` 里补一段。
const SCHEMA_VERSION: i64 = 4;

pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            paths::ensure_dir(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// 读当前库结构版本。
    fn user_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    /// 分级迁移：每段只补自己那一版的东西，老库升级不重建已有表。
    ///
    /// 两条硬约束（都是实测踩出来的）：
    /// 1. **整段迁移必须在一个写事务里**。之前 v4 的 `DROP` + `CREATE` 是裸 batch，
    ///    中途失败（或两个进程同时启动各跑一遍迁移）会留下「表已建、`user_version`
    ///    还是 3」的库，下次打开直接报 `table usage_files already exists`，用户只能删库。
    /// 2. **版本号要在拿到写锁之后再读**。先读的那份快照在多进程下是过期的：
    ///    进程 A 刚升完，进程 B 拿着旧快照又跑一遍 v4。
    fn migrate(&self) -> Result<()> {
        // 快速路径：版本够新、表也齐就不去抢写锁（GUI 与 CLI 同时开是常态）
        if self.user_version()? >= SCHEMA_VERSION && self.usage_table_exists()? {
            return Ok(());
        }
        // `BEGIN IMMEDIATE` 立刻拿写锁：并发启动时只有一个进程真正迁移，
        // 其余进程在此排队，拿到锁后看到的是已经升好的版本。
        self.conn.execute_batch("BEGIN IMMEDIATE;")?;
        let outcome = self.migrate_locked();
        match outcome {
            Ok(()) => {
                if let Err(err) = self.conn.execute_batch("COMMIT;") {
                    let _ = self.conn.execute_batch("ROLLBACK;");
                    return Err(err.into());
                }
                Ok(())
            }
            Err(err) => {
                // 回滚掉半截的表结构，库回到迁移前的自洽状态，下次打开重试即可
                let _ = self.conn.execute_batch("ROLLBACK;");
                Err(err)
            }
        }
    }

    /// 迁移本体：调用时已持有写锁，每一步要么全成、要么随事务回滚。
    fn migrate_locked(&self) -> Result<()> {
        let version = self.user_version()?;
        if version < 2 {
            self.migrate_v2(version)?;
        }
        if version < 3 {
            self.migrate_v3()?;
        }
        if version < 4 {
            self.migrate_v4()?;
        }
        // 兜底修复：版本号够新、`usage_files` 却不在（并发迁移/中途失败的残留形态）。
        // 表是纯缓存，重建不丢信息，代价只是一次重新解析。
        if !self.usage_table_exists()? {
            self.create_usage_table_v4()?;
        }
        Ok(())
    }

    /// `usage_files` 是否已在。
    fn usage_table_exists(&self) -> Result<bool> {
        let found: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'usage_files'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }

    fn migrate_v2(&self, version: i64) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS hash_cache (
              path TEXT PRIMARY KEY,
              size INTEGER NOT NULL,
              mtime INTEGER NOT NULL,
              tree_hash TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS skills (
              path TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              source TEXT NOT NULL,
              agent TEXT,
              size INTEGER NOT NULL,
              tree_hash TEXT,
              updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS links (
              at TEXT PRIMARY KEY,
              target TEXT NOT NULL,
              strategy TEXT NOT NULL,
              created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS ops_log (
              batch_id TEXT PRIMARY KEY,
              kind TEXT NOT NULL,
              title TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              undone INTEGER NOT NULL DEFAULT 0,
              op_count INTEGER NOT NULL DEFAULT 0,
              backup_dir TEXT
            );
            CREATE TABLE IF NOT EXISTS translations (
              hash TEXT NOT NULL,
              lang TEXT NOT NULL,
              source_text TEXT,
              translation TEXT NOT NULL,
              model TEXT,
              created_at INTEGER NOT NULL,
              PRIMARY KEY (hash, lang)
            );
            "#,
        )?;
        if version == 1 {
            // v2 起 path 主键改用 paths::path_key（Windows 大小写折叠、分隔符统一）。
            // 三张表都是扫描可重建的索引，直接清空重新生成；ops_log 与备份目录不动，
            // 已有批次的回滚能力不受影响。
            self.conn
                .execute_batch("DELETE FROM hash_cache; DELETE FROM skills; DELETE FROM links;")?;
        }
        self.conn.execute_batch("PRAGMA user_version = 2;")?;
        Ok(())
    }

    /// v3：token 用量的按文件缓存。
    ///
    /// 一文件一行：`buckets` 是该文件解析出的小时桶 JSON（按模型 / provider 聚合），
    /// `prints` 是跨文件去重指纹。行数 = 日志文件数（上限几百），文件变了整行替换，
    /// 日志被删就删行 —— 不做逐条记录，避免几十万行的表随时间无限膨胀。
    ///
    /// v4 起被 `groups` 取代，仅作为历史版本存在。
    fn migrate_v3(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS usage_files (
              path TEXT PRIMARY KEY,
              size INTEGER NOT NULL,
              mtime INTEGER NOT NULL,
              source TEXT NOT NULL,
              records INTEGER NOT NULL DEFAULT 0,
              bad_lines INTEGER NOT NULL DEFAULT 0,
              buckets TEXT NOT NULL,
              prints TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS app_meta (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL
            );
            PRAGMA user_version = 3;
            "#,
        )?;
        Ok(())
    }

    /// v4：用量缓存改成「按指纹分组」，去重不再固化在缓存里。
    ///
    /// v3 缓存的是**去重后**的桶：两份复制会话只有一份带用量，另一份存 0。一旦用户
    /// 删掉「胜出」的那份文件（或两份文件 mtime 落在同一毫秒、目录枚举顺序不同导致
    /// 胜者随平台漂移），剩余那份永久是 0 —— macOS 上实测到 800 变 300 就是这个。
    ///
    /// 现在一文件一行存 `groups`（`[{fp, bucket}]`），跨文件去重改在每次扫描时按文件
    /// 顺序重算，删掉任意一份都不丢账。表是纯缓存，直接重建，不做数据迁移。
    ///
    /// 这里**故意**先 DROP 再 CREATE（而不是 `CREATE TABLE IF NOT EXISTS`）：
    /// 半迁移的库是「表在、版本旧、结构还是 v3」，`IF NOT EXISTS` 会静默沿用错的列，
    /// 之后写 `groups` 就报 no such column。DROP 把这种状态拉回确定态。
    fn migrate_v4(&self) -> Result<()> {
        self.conn
            .execute_batch("DROP TABLE IF EXISTS usage_files;")?;
        self.create_usage_table_v4()?;
        self.conn.execute_batch("PRAGMA user_version = 4;")?;
        Ok(())
    }

    /// v4 的 `usage_files` 表定义。调用方保证此刻表不存在（事务内）。
    fn create_usage_table_v4(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE usage_files (
              path TEXT PRIMARY KEY,
              size INTEGER NOT NULL,
              mtime INTEGER NOT NULL,
              source TEXT NOT NULL,
              records INTEGER NOT NULL DEFAULT 0,
              bad_lines INTEGER NOT NULL DEFAULT 0,
              groups TEXT NOT NULL DEFAULT '[]'
            );
            "#,
        )?;
        Ok(())
    }

    pub fn cached_hash(&self, path: &Path, size: u64, mtime: i64) -> Result<Option<String>> {
        let key = paths::path_key(path);
        let found: Option<(i64, i64, String)> = self
            .conn
            .query_row(
                "SELECT size, mtime, tree_hash FROM hash_cache WHERE path = ?1",
                params![key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        Ok(match found {
            Some((s, m, h)) if s as u64 == size && m == mtime => Some(h),
            _ => None,
        })
    }

    pub fn put_hash(&self, path: &Path, size: u64, mtime: i64, hash: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO hash_cache(path, size, mtime, tree_hash) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET size=?2, mtime=?3, tree_hash=?4",
            params![paths::path_key(path), size as i64, mtime, hash],
        )?;
        Ok(())
    }

    pub fn forget_hash(&self, path: &Path) -> Result<()> {
        self.conn.execute(
            "DELETE FROM hash_cache WHERE path = ?1",
            params![paths::path_key(path)],
        )?;
        Ok(())
    }

    pub fn record_skills(&self, records: &[SkillRecord]) -> Result<()> {
        let now = paths::now_ts();
        let tx = self.conn.unchecked_transaction()?;
        for rec in records {
            tx.execute(
                "INSERT INTO skills(path, name, source, agent, size, tree_hash, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(path) DO UPDATE SET name=?2, source=?3, agent=?4, size=?5, tree_hash=?6, updated_at=?7",
                params![
                    paths::path_key(&rec.path),
                    rec.name,
                    format!("{:?}", rec.source),
                    rec.agent.as_ref().map(|a| a.slug()),
                    rec.size_bytes as i64,
                    rec.tree_hash,
                    now
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn record_link(&self, at: &Path, target: &Path, strategy: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO links(at, target, strategy, created_at) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(at) DO UPDATE SET target=?2, strategy=?3, created_at=?4",
            params![
                paths::path_key(at),
                paths::path_str(target),
                strategy,
                paths::now_ts()
            ],
        )?;
        Ok(())
    }

    pub fn forget_link(&self, at: &Path) -> Result<()> {
        self.conn.execute(
            "DELETE FROM links WHERE at = ?1",
            params![paths::path_key(at)],
        )?;
        Ok(())
    }

    /// 是否为 AgentHub 自己创建的链接（决定能否安全删除）。
    pub fn is_managed_link(&self, at: &Path) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM links WHERE at = ?1",
            params![paths::path_key(at)],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn log_batch(
        &self,
        batch_id: &str,
        kind: &str,
        title: &str,
        op_count: usize,
        backup_dir: Option<&Path>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO ops_log(batch_id, kind, title, created_at, undone, op_count, backup_dir)
             VALUES(?1, ?2, ?3, ?4, 0, ?5, ?6)",
            params![
                batch_id,
                kind,
                title,
                paths::now_ts(),
                op_count as i64,
                backup_dir.map(paths::path_str)
            ],
        )?;
        Ok(())
    }

    pub fn mark_undone(&self, batch_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE ops_log SET undone = 1 WHERE batch_id = ?1",
            params![batch_id],
        )?;
        Ok(())
    }

    pub fn recent_batches(&self, limit: usize) -> Result<Vec<BatchRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT batch_id, kind, title, created_at, undone, op_count FROM ops_log
             ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(BatchRecord {
                batch_id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                undone: row.get::<_, i64>(4)? != 0,
                op_count: row.get::<_, i64>(5)? as usize,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn batch_backup_dir(&self, batch_id: &str) -> Result<Option<PathBuf>> {
        let dir: Option<String> = self
            .conn
            .query_row(
                "SELECT backup_dir FROM ops_log WHERE batch_id = ?1",
                params![batch_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        Ok(dir.map(PathBuf::from))
    }

    pub fn put_translation(
        &self,
        hash: &str,
        lang: &str,
        source_text: Option<&str>,
        translation: &str,
        model: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO translations(hash, lang, source_text, translation, model, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(hash, lang) DO UPDATE SET translation=?4, model=?5",
            params![hash, lang, source_text, translation, model, paths::now_ts()],
        )?;
        Ok(())
    }

    pub fn get_translation(&self, hash: &str, lang: &str) -> Result<Option<String>> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT translation FROM translations WHERE hash = ?1 AND lang = ?2",
                params![hash, lang],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found)
    }

    /// 读某个日志文件的用量缓存；`size` / `mtime` 不一致视为未命中（返回 None）。
    ///
    /// 只回传原始 JSON 字符串，反序列化交给 `ext::usage`，DB 层不认识用量结构。
    pub fn usage_cache(&self, path: &Path, size: u64, mtime: i64) -> Result<Option<UsageCacheRow>> {
        let found: Option<(i64, i64, String, i64, i64, String)> = self
            .conn
            .query_row(
                "SELECT size, mtime, source, records, bad_lines, groups
                 FROM usage_files WHERE path = ?1",
                params![paths::path_key(path)],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()?;
        Ok(match found {
            Some((s, m, source, records, bad_lines, groups)) if s as u64 == size && m == mtime => {
                Some(UsageCacheRow {
                    source,
                    records: records as u64,
                    bad_lines: bad_lines as u64,
                    groups,
                })
            }
            _ => None,
        })
    }

    pub fn put_usage_cache(
        &self,
        path: &Path,
        row: &UsageCacheRow,
        size: u64,
        mtime: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO usage_files(path, size, mtime, source, records, bad_lines, groups)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET size=?2, mtime=?3, source=?4, records=?5,
               bad_lines=?6, groups=?7",
            params![
                paths::path_key(path),
                size as i64,
                mtime,
                row.source,
                row.records as i64,
                row.bad_lines as i64,
                row.groups
            ],
        )?;
        Ok(())
    }

    /// 删掉本次枚举不到的文件行（日志被清理 / 目录被移动）。返回删除条数。
    pub fn prune_usage_cache(&self, keep: &std::collections::BTreeSet<String>) -> Result<usize> {
        let mut stmt = self.conn.prepare("SELECT path FROM usage_files")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut stale = Vec::new();
        for row in rows {
            let key = row?;
            if !keep.contains(&key) {
                stale.push(key);
            }
        }
        let tx = self.conn.unchecked_transaction()?;
        for key in &stale {
            tx.execute("DELETE FROM usage_files WHERE path = ?1", params![key])?;
        }
        tx.commit()?;
        Ok(stale.len())
    }

    pub fn clear_usage_cache(&self) -> Result<()> {
        self.conn.execute_batch("DELETE FROM usage_files;")?;
        Ok(())
    }

    pub fn meta_get(&self, key: &str) -> Result<Option<String>> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found)
    }

    pub fn meta_set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO app_meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=?2",
            params![key, value],
        )?;
        Ok(())
    }
}

/// `usage_files` 的一行：解析产物用 JSON 承载，DB 层不解释其内容。
#[derive(Debug, Clone, Default)]
pub struct UsageCacheRow {
    pub source: String,
    pub records: u64,
    pub bad_lines: u64,
    /// 按指纹分组的小时桶 JSON（`[{fp, bucket}]`）。
    ///
    /// 存的是**去重前**的结果：跨文件去重在每次扫描时重算，所以删掉任意一份复制会话
    /// 都不会丢账。
    pub groups: String,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db").finish_non_exhaustive()
    }
}
