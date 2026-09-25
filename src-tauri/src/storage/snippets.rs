use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use rusqlite::{params, Connection};
use super::settings::get_app_data_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceSnippet {
    pub id: String,
    pub trigger_phrase: String,
    pub expansion_text: String,
    pub match_type: String, // "exact" or "prefix"
    pub is_active: bool,
    pub created_at: i64,
}

pub struct SnippetStore {
    conn: Mutex<Connection>,
}

impl SnippetStore {
    pub fn new() -> Result<Self, rusqlite::Error> {
        let db_path = get_app_data_dir().join("snippets.db");
        let conn = Connection::open(db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS voice_snippets (
                id TEXT PRIMARY KEY,
                trigger_phrase TEXT NOT NULL UNIQUE COLLATE NOCASE,
                expansion_text TEXT NOT NULL,
                match_type TEXT NOT NULL DEFAULT 'exact',
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        let store = Self {
            conn: Mutex::new(conn),
        };

        store.seed_defaults_if_empty()?;
        Ok(store)
    }

    fn seed_defaults_if_empty(&self) -> Result<(), rusqlite::Error> {
        let count: i64 = self.conn.lock().unwrap().query_row(
            "SELECT COUNT(*) FROM voice_snippets",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            let defaults = vec![
                (
                    "snippet-standup",
                    "insert standup",
                    "**Daily Standup ({date})**\n- **Yesterday:** \n- **Today:** \n- **Blockers:** None",
                    "exact",
                ),
                (
                    "snippet-cal",
                    "my calendar",
                    "Here is my booking link to schedule a time: https://calendly.com/meeting",
                    "exact",
                ),
                (
                    "snippet-closing",
                    "closing signature",
                    "Best regards,\n[Your Name]",
                    "exact",
                ),
                (
                    "snippet-email",
                    "my email",
                    "hello@example.com",
                    "exact",
                ),
            ];

            let now = chrono::Utc::now().timestamp_millis();
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction()?;
            for (id, trigger, expansion, m_type) in defaults {
                tx.execute(
                    "INSERT INTO voice_snippets (id, trigger_phrase, expansion_text, match_type, is_active, created_at)
                     VALUES (?1, ?2, ?3, ?4, 1, ?5)",
                    params![id, trigger, expansion, m_type, now],
                )?;
            }
            tx.commit()?;
        }
        Ok(())
    }

    pub fn list_snippets(&self) -> Result<Vec<VoiceSnippet>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, trigger_phrase, expansion_text, match_type, is_active, created_at FROM voice_snippets ORDER BY created_at ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(VoiceSnippet {
                id: row.get(0)?,
                trigger_phrase: row.get(1)?,
                expansion_text: row.get(2)?,
                match_type: row.get(3)?,
                is_active: row.get::<_, i32>(4)? != 0,
                created_at: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn upsert_snippet(&self, s: &VoiceSnippet) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO voice_snippets (id, trigger_phrase, expansion_text, match_type, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                trigger_phrase = excluded.trigger_phrase,
                expansion_text = excluded.expansion_text,
                match_type = excluded.match_type,
                is_active = excluded.is_active",
            params![
                s.id,
                s.trigger_phrase.trim().to_lowercase(),
                s.expansion_text,
                s.match_type,
                if s.is_active { 1 } else { 0 },
                s.created_at
            ],
        )?;
        Ok(())
    }

    pub fn delete_snippet(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM voice_snippets WHERE id = ?1", params![id])?;
        Ok(())
    }
}
