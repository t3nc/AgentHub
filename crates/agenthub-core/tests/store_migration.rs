//! 索引库迁移的回归测试。
//!
//! 背景：v4 迁移原来是裸 `DROP` + `CREATE` 的 batch，没有事务。中途失败（或 GUI 与 CLI
//! 同时启动各跑一遍）会留下「表已建、`user_version` 还是 3」的库，之后每次打开都报
//! `table usage_files already exists`，用户只能删库重来。这里把三种残留形态钉住。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use agenthub_core::store::Db;
use rusqlite::{Connection, params};

static SEQ: AtomicU32 = AtomicU32::new(0);

/// 每个用例一个独立目录：并行跑不会互相踩。
fn case_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agenthub-migrate-{tag}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn user_version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap()
}

/// `usage_files` 的列名集合：用来确认结构真的是 v4（有 groups）而不是 v3（有 buckets）。
fn columns(conn: &Connection, table: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap();
    stmt.query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn has_table(conn: &Connection, table: &str) -> bool {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table],
            |row| row.get(0),
        )
        .ok();
    found.is_some()
}

/// 模拟 v3 时代建出来的库：`usage_files` 是旧结构，还带着一行缓存。
fn seed_v3(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS hash_cache (
          path TEXT PRIMARY KEY, size INTEGER NOT NULL, mtime INTEGER NOT NULL, tree_hash TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS skills (
          path TEXT PRIMARY KEY, name TEXT NOT NULL, source TEXT NOT NULL, agent TEXT,
          size INTEGER NOT NULL, tree_hash TEXT, updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS links (
          at TEXT PRIMARY KEY, target TEXT NOT NULL, strategy TEXT NOT NULL, created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS ops_log (
          batch_id TEXT PRIMARY KEY, kind TEXT NOT NULL, title TEXT NOT NULL,
          created_at INTEGER NOT NULL, undone INTEGER NOT NULL DEFAULT 0,
          op_count INTEGER NOT NULL DEFAULT 0, backup_dir TEXT
        );
        CREATE TABLE IF NOT EXISTS translations (
          hash TEXT NOT NULL, lang TEXT NOT NULL, source_text TEXT, translation TEXT NOT NULL,
          model TEXT, created_at INTEGER NOT NULL, PRIMARY KEY (hash, lang)
        );
        CREATE TABLE IF NOT EXISTS usage_files (
          path TEXT PRIMARY KEY, size INTEGER NOT NULL, mtime INTEGER NOT NULL, source TEXT NOT NULL,
          records INTEGER NOT NULL DEFAULT 0, bad_lines INTEGER NOT NULL DEFAULT 0,
          buckets TEXT NOT NULL, prints TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        INSERT INTO usage_files(path, size, mtime, source, buckets)
          VALUES('/tmp/stale.jsonl', 10, 20, 'claude', '[]');
        PRAGMA user_version = 3;
        "#,
    )
    .unwrap();
    conn
}

#[test]
fn fresh_database_reaches_latest_version() {
    let dir = case_dir("fresh");
    let path = dir.join("index.db");
    let db = Db::open(&path).unwrap();

    assert_eq!(user_version(&db.conn), 4, "全新库应直接落到 v4");
    let cols = columns(&db.conn, "usage_files");
    assert!(
        cols.contains(&"groups".to_string()),
        "v4 的缓存表应有 groups 列，实际 {cols:?}"
    );
    assert!(!cols.contains(&"buckets".to_string()), "v3 的列应已消失");
    drop(db);

    // 再开一次不应有任何迁移动作（快速路径）
    let db = Db::open(&path).unwrap();
    assert_eq!(user_version(&db.conn), 4);
    std::fs::remove_dir_all(dir).ok();
}

/// 主用例：v3 结构的库要能升到 v4，旧缓存行被丢弃（纯缓存，可重算）。
#[test]
fn v3_database_upgrades_and_drops_stale_cache() {
    let dir = case_dir("v3");
    let path = dir.join("index.db");
    drop(seed_v3(&path));

    let db = Db::open(&path).unwrap();
    assert_eq!(user_version(&db.conn), 4);
    let cols = columns(&db.conn, "usage_files");
    assert!(cols.contains(&"groups".to_string()), "实际 {cols:?}");

    let rows: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM usage_files", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 0, "重建后旧缓存行应被清掉");
    std::fs::remove_dir_all(dir).ok();
}

/// 曾经炸掉的那台机器：表已按 v4 建好，但 `user_version` 停在 3。
/// 修复后必须能自愈（之前是 `table usage_files already exists`）。
#[test]
fn half_migrated_database_heals() {
    let dir = case_dir("half");
    let path = dir.join("index.db");
    let conn = seed_v3(&path);
    // 模拟「DROP + CREATE 跑了一半」：结构已经是 v4，版本号没涨
    conn.execute_batch(
        r#"
        DROP TABLE usage_files;
        CREATE TABLE usage_files (
          path TEXT PRIMARY KEY, size INTEGER NOT NULL, mtime INTEGER NOT NULL, source TEXT NOT NULL,
          records INTEGER NOT NULL DEFAULT 0, bad_lines INTEGER NOT NULL DEFAULT 0,
          groups TEXT NOT NULL DEFAULT '[]'
        );
        "#,
    )
    .unwrap();
    assert_eq!(user_version(&conn), 3);
    drop(conn);

    let db = Db::open(&path).expect("半迁移的库必须能自愈，而不是让用户删库");
    assert_eq!(user_version(&db.conn), 4);
    assert!(
        columns(&db.conn, "usage_files").contains(&"groups".to_string()),
        "自愈后仍是 v4 结构"
    );
    std::fs::remove_dir_all(dir).ok();
}

/// 另一种残留：版本号已经 4，表却不在（DROP 成功、CREATE 没跑）。
#[test]
fn missing_table_with_current_version_is_repaired() {
    let dir = case_dir("missing");
    let path = dir.join("index.db");
    let conn = seed_v3(&path);
    conn.execute_batch("DROP TABLE usage_files; PRAGMA user_version = 4;")
        .unwrap();
    drop(conn);

    let db = Db::open(&path).unwrap();
    assert!(
        has_table(&db.conn, "usage_files"),
        "版本号够新但表缺失时应补建"
    );
    assert!(
        columns(&db.conn, "usage_files").contains(&"groups".to_string()),
        "补建出来的必须是 v4 结构"
    );
    std::fs::remove_dir_all(dir).ok();
}

/// 两个进程同时打开同一个库（GUI + CLI 的日常形态）不能互相把迁移搞坏。
/// 正常实现下 `BEGIN IMMEDIATE` 会让后到者排队，谁先拿到锁谁迁移。
#[test]
fn concurrent_opens_do_not_corrupt_migration() {
    let dir = case_dir("concurrent");
    let path = dir.join("index.db");
    drop(seed_v3(&path));

    let mut handles = Vec::new();
    for _ in 0..4 {
        let path: PathBuf = path.clone();
        handles.push(std::thread::spawn(move || {
            // 并发下 SQLite 可能返回 SQLITE_BUSY：调用方（真实启动路径）会重试，
            // 这里显式重试几次，重点是不能出现 "table already exists"
            for _ in 0..20 {
                match Db::open(&path) {
                    Ok(db) => return Ok(user_version(&db.conn)),
                    Err(err) => {
                        let text = err.to_string();
                        if !text.contains("locked") && !text.contains("busy") {
                            return Err(text);
                        }
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                }
            }
            Err("多次重试后仍拿不到库".to_string())
        }));
    }

    for handle in handles {
        let version = handle.join().unwrap().expect("并发打开不应报迁移错误");
        assert_eq!(version, 4);
    }
    std::fs::remove_dir_all(dir).ok();
}
