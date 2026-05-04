use crate::config::APP_ID;
use crate::error::{AppError, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub source: String,
    pub target: String,
    pub from_lang: String,
    pub to_lang: String,
    pub service: String,
    pub timestamp: i64,
}

pub struct HistoryStore {
    conn: Mutex<rusqlite::Connection>,
}

impl HistoryStore {
    pub fn open() -> Result<Self> {
        let db_path = Self::db_path()?;
        Self::open_at(&db_path)
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source TEXT NOT NULL,
                target TEXT NOT NULL,
                from_lang TEXT NOT NULL,
                to_lang TEXT NOT NULL,
                service TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            );",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source TEXT NOT NULL,
                target TEXT NOT NULL,
                from_lang TEXT NOT NULL,
                to_lang TEXT NOT NULL,
                service TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            );",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert(
        &self,
        source: &str,
        target: &str,
        from_lang: &str,
        to_lang: &str,
        service: &str,
        timestamp: i64,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Custom(e.to_string()))?;
        conn.execute(
            "INSERT INTO history (source, target, from_lang, to_lang, service, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![source, target, from_lang, to_lang, service, timestamp],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list(&self, page: u32, per_page: u32) -> Result<Vec<HistoryEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Custom(e.to_string()))?;
        let offset = page.saturating_sub(1) * per_page;
        let mut stmt = conn.prepare(
            "SELECT id, source, target, from_lang, to_lang, service, timestamp
             FROM history ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![per_page, offset], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                source: row.get(1)?,
                target: row.get(2)?,
                from_lang: row.get(3)?,
                to_lang: row.get(4)?,
                service: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(AppError::from)
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Custom(e.to_string()))?;
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Custom(e.to_string()))?;
        conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    pub fn count(&self) -> Result<u32> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Custom(e.to_string()))?;
        let count: u32 = conn.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;
        Ok(count)
    }

    fn db_path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .ok_or_else(|| AppError::Custom("Failed to get config directory".into()))?;
        Ok(dir.join(APP_ID).join("history.db"))
    }
}
