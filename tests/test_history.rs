use pot_gtk::core::history::HistoryStore;

struct TestContext {
    _dir: tempfile::TempDir,
    db: HistoryStore,
}

fn open_test_db() -> TestContext {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test_history.db");
    let db = HistoryStore::open_at(&db_path).expect("Failed to open test history DB");
    TestContext { _dir: dir, db }
}

#[test]
fn insert_and_list() {
    let ctx = open_test_db();
    let now = 1000;
    ctx.db
        .insert("hello", "你好", "en", "zh-CN", "google", now)
        .unwrap();

    let entries = ctx.db.list(1, 10).unwrap();
    let found = entries
        .iter()
        .find(|e| e.source == "hello" && e.target == "你好");
    assert!(found.is_some(), "inserted entry not found in list");
    let entry = found.unwrap();
    assert_eq!(entry.from_lang, "en");
    assert_eq!(entry.to_lang, "zh-CN");
    assert_eq!(entry.service, "google");
    assert_eq!(entry.timestamp, now);
}

#[test]
fn insert_multiple_and_paginate() {
    let ctx = open_test_db();
    for i in 0..5 {
        ctx.db
            .insert(
                &format!("text_{}", i),
                &format!("结果_{}", i),
                "en",
                "zh-CN",
                "test",
                2000 + i as i64,
            )
            .unwrap();
    }

    let page1 = ctx.db.list(1, 3).unwrap();
    assert_eq!(page1.len(), 3, "page 1 should have 3 items");

    let page2 = ctx.db.list(2, 3).unwrap();
    assert!(page2.len() >= 2, "page 2 should have at least 2 items");
}

#[test]
fn delete_entry() {
    let ctx = open_test_db();
    let id = ctx
        .db
        .insert("to_delete", "删除", "en", "zh-CN", "test", 3000)
        .unwrap();
    ctx.db.delete(id).unwrap();

    let entries = ctx.db.list(1, 10).unwrap();
    assert!(
        entries.iter().all(|e| e.id != id),
        "deleted entry should not appear"
    );
}

#[test]
fn count_entries_increases() {
    let ctx = open_test_db();
    let before = ctx.db.count().unwrap();
    ctx.db
        .insert("count_test", "计数", "en", "zh-CN", "test", 4000)
        .unwrap();
    let after = ctx.db.count().unwrap();
    assert!(
        after > before,
        "count should increase after insert (before={}, after={})",
        before,
        after
    );
}

#[test]
fn clear_removes_inserted_entries() {
    let ctx = open_test_db();
    ctx.db
        .insert("clear_a", "清除A", "en", "zh-CN", "clear_test", 5000)
        .unwrap();
    ctx.db
        .insert("clear_b", "清除B", "en", "zh-CN", "clear_test", 5001)
        .unwrap();
    let before = ctx.db.count().unwrap();
    assert!(before >= 2);
    ctx.db.clear().unwrap();
    let after = ctx.db.count().unwrap();
    assert_eq!(after, 0, "clear should remove all entries");
}
