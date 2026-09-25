use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use super::settings::get_app_data_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub timestamp: String,
    pub text: String,
    pub duration_seconds: f64,
    pub char_count: usize,
    pub words_count: usize,
    pub target_app: Option<String>,
    pub model: Option<String>,
    pub engine: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_words: u64,
    pub total_speaking_seconds: f64,
    pub total_recordings: u64,
    pub avg_pace_wpm: u64,
    pub today_requests: u64,
}

pub fn get_user_backup_dir() -> PathBuf {
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let docs = PathBuf::from(userprofile).join("Documents").join("Rift Backups");
        let _ = std::fs::create_dir_all(&docs);
        return docs;
    }
    let fallback = get_app_data_dir().join("backups");
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

pub struct HistoryStore {
    db_path: PathBuf,
}

impl HistoryStore {
    pub fn new() -> Result<Self> {
        let db_path = get_app_data_dir().join("history.db");
        let conn = Connection::open(&db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS dictation_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                text TEXT NOT NULL,
                duration_seconds REAL NOT NULL,
                char_count INTEGER NOT NULL,
                words_count INTEGER NOT NULL DEFAULT 0,
                target_app TEXT,
                model TEXT,
                engine TEXT
            )",
            [],
        )?;

        // Ensure columns exist if migrating from older schema
        let _ = conn.execute("ALTER TABLE dictation_history ADD COLUMN words_count INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE dictation_history ADD COLUMN model TEXT", []);
        let _ = conn.execute("ALTER TABLE dictation_history ADD COLUMN engine TEXT", []);

        // 1. Auto-migrate entries from legacy rift.db if present
        let legacy_db_path = get_app_data_dir().join("rift.db");
        if legacy_db_path.exists() {
            if let Ok(legacy_conn) = Connection::open(&legacy_db_path) {
                if let Ok(mut stmt) = legacy_conn.prepare(
                    "SELECT timestamp, text, word_count, duration_seconds, target_app, model, engine FROM history_entries ORDER BY timestamp ASC"
                ) {
                    if let Ok(rows) = stmt.query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<usize>>(2)?.unwrap_or(0),
                            row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, Option<String>>(6)?,
                        ))
                    }) {
                        for item in rows.flatten() {
                            let (ts, txt, wc, dur, app_name, model_name, engine_name) = item;
                            let exists: i64 = conn.query_row(
                                "SELECT COUNT(*) FROM dictation_history WHERE timestamp = ?1 OR (text = ?2 AND duration_seconds = ?3)",
                                params![&ts, &txt, dur],
                                |r| r.get(0),
                            ).unwrap_or(0);

                            if exists == 0 {
                                let calculated_wc = if wc > 0 { wc } else { txt.split_whitespace().count() };
                                let char_len = txt.chars().count();
                                let _ = conn.execute(
                                    "INSERT INTO dictation_history (timestamp, text, duration_seconds, char_count, words_count, target_app, model, engine)
                                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                                    params![ts, txt, dur, char_len, calculated_wc, app_name, model_name, engine_name],
                                );
                            }
                        }
                    }
                }
            }
        }

        // 2. Auto-restore from Documents\Rift Backups if dictation_history is completely empty
        let current_count: i64 = conn.query_row("SELECT COUNT(*) FROM dictation_history", [], |r| r.get(0)).unwrap_or(0);
        if current_count == 0 {
            let backup_dir = get_user_backup_dir();
            let backup_json = backup_dir.join("history_backup.json");
            if backup_json.exists() {
                if let Ok(content) = std::fs::read_to_string(&backup_json) {
                    if let Ok(entries) = serde_json::from_str::<Vec<HistoryEntry>>(&content) {
                        for e in entries {
                            let _ = conn.execute(
                                "INSERT INTO dictation_history (timestamp, text, duration_seconds, char_count, words_count, target_app, model, engine)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                                params![e.timestamp, e.text, e.duration_seconds, e.char_count, e.words_count, e.target_app, e.model, e.engine],
                            );
                        }
                    }
                }
            }
        }

        // 3. Backfill word counts for legacy entries if needed
        if let Ok(mut stmt) = conn.prepare("SELECT id, text FROM dictation_history WHERE words_count = 0 AND length(text) > 0") {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            }) {
                for item in rows.flatten() {
                    let wc = item.1.split_whitespace().count();
                    let _ = conn.execute("UPDATE dictation_history SET words_count = ?1 WHERE id = ?2", params![wc, item.0]);
                }
            }
        }

        let store = Self { db_path };
        store.backup_to_safe_location();
        Ok(store)
    }

    pub fn backup_to_safe_location(&self) {
        let db_path = self.db_path.clone();
        std::thread::spawn(move || {
            let backup_dir = get_user_backup_dir();
            let backup_db_file = backup_dir.join("history_backup.db");
            let backup_json_file = backup_dir.join("history_backup.json");

            // 1. Copy SQLite database file
            let _ = std::fs::copy(&db_path, &backup_db_file);

            // 2. Export complete human-readable JSON dump
            if let Ok(conn) = Connection::open(&db_path) {
                if let Ok(mut stmt) = conn.prepare(
                    "SELECT id, timestamp, text, duration_seconds, char_count, words_count, target_app, model, engine
                     FROM dictation_history ORDER BY id DESC LIMIT 5000",
                ) {
                    if let Ok(rows) = stmt.query_map([], |row| {
                        Ok(HistoryEntry {
                            id: row.get(0)?,
                            timestamp: row.get(1)?,
                            text: row.get(2)?,
                            duration_seconds: row.get(3)?,
                            char_count: row.get(4)?,
                            words_count: row.get::<_, Option<usize>>(5)?.unwrap_or(0),
                            target_app: row.get(6)?,
                            model: row.get(7)?,
                            engine: row.get(8)?,
                        })
                    }) {
                        let entries: Vec<HistoryEntry> = rows.flatten().collect();
                        if let Ok(json_str) = serde_json::to_string_pretty(&entries) {
                            let _ = std::fs::write(&backup_json_file, json_str);
                        }
                    }
                }
            }
        });
    }

    pub fn insert_entry(
        &self,
        text: &str,
        duration_seconds: f64,
        target_app: Option<&str>,
        model: Option<&str>,
        engine: Option<&str>,
    ) -> Result<i64> {
        let conn = Connection::open(&self.db_path)?;
        let timestamp = chrono::Local::now().to_rfc3339();
        let char_count = text.chars().count();
        let words_count = text.split_whitespace().count();
        conn.execute(
            "INSERT INTO dictation_history (timestamp, text, duration_seconds, char_count, words_count, target_app, model, engine)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![timestamp, text, duration_seconds, char_count, words_count, target_app, model, engine],
        )?;
        let id = conn.last_insert_rowid();
        self.backup_to_safe_location();
        Ok(id)
    }

    pub fn get_recent(&self, limit: usize) -> Result<Vec<HistoryEntry>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, text, duration_seconds, char_count, words_count, target_app, model, engine
             FROM dictation_history ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                text: row.get(2)?,
                duration_seconds: row.get(3)?,
                char_count: row.get(4)?,
                words_count: row.get::<_, Option<usize>>(5)?.unwrap_or(0),
                target_app: row.get(6)?,
                model: row.get(7)?,
                engine: row.get(8)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            if let Ok(entry) = row {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    pub fn get_dashboard_stats(&self) -> Result<DashboardStats> {
        let conn = Connection::open(&self.db_path)?;

        let mut stmt = conn.prepare(
            "SELECT 
                COALESCE(SUM(words_count), 0),
                COALESCE(SUM(duration_seconds), 0.0),
                COUNT(*)
             FROM dictation_history",
        )?;

        let (total_words, total_speaking_seconds, total_recordings): (i64, f64, i64) = stmt.query_row([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;

        // Today's requests
        let today_prefix = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut today_stmt = conn.prepare(
            "SELECT COUNT(*) FROM dictation_history WHERE timestamp LIKE ?1",
        )?;
        let today_requests: i64 = today_stmt
            .query_row(params![format!("{}%", today_prefix)], |row| row.get(0))
            .unwrap_or(0);

        let total_words_u64 = total_words.max(0) as u64;
        let total_recordings_u64 = total_recordings.max(0) as u64;
        let today_requests_u64 = today_requests.max(0) as u64;

        let avg_pace_wpm = if total_speaking_seconds > 0.0 && total_words_u64 > 0 {
            let minutes = total_speaking_seconds / 60.0;
            (total_words_u64 as f64 / minutes).round() as u64
        } else {
            0
        };

        Ok(DashboardStats {
            total_words: total_words_u64,
            total_speaking_seconds,
            total_recordings: total_recordings_u64,
            avg_pace_wpm,
            today_requests: today_requests_u64,
        })
    }

    pub fn delete_entry(&self, id: i64) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute("DELETE FROM dictation_history WHERE id = ?1", params![id])?;
        self.backup_to_safe_location();
        Ok(())
    }

    pub fn clear_all(&self) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute("DELETE FROM dictation_history", [])?;
        self.backup_to_safe_location();
        Ok(())
    }

    pub fn get_today_groq_audio_seconds(&self) -> Result<(f64, u64)> {
        let conn = Connection::open(&self.db_path)?;
        let today_prefix = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut stmt = conn.prepare(
            "SELECT COALESCE(SUM(duration_seconds), 0.0), COUNT(*)
             FROM dictation_history 
             WHERE timestamp LIKE ?1 AND (engine = 'groq' OR model LIKE '%groq%' OR (engine != 'openrouter' AND engine != 'local'))",
        )?;
        let (total_secs, count): (f64, i64) = stmt.query_row(params![format!("{}%", today_prefix)], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        Ok((total_secs, count.max(0) as u64))
    }
}

