use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub id: String,
    pub phrase: String,
    pub replacement: String,
    pub enabled: bool,
    pub tag: Option<String>,
}

use super::settings::get_app_data_dir;
use super::history::get_user_backup_dir;

pub struct DictionaryStore {
    db_path: PathBuf,
    cache: std::sync::RwLock<Option<Vec<DictionaryEntry>>>,
}

impl DictionaryStore {
    pub fn new() -> Result<Self> {
        let app_dir = get_app_data_dir();
        if !app_dir.exists() {
            let _ = std::fs::create_dir_all(&app_dir);
        }
        let db_path = app_dir.join("dictionary.db");
        let conn = Connection::open(&db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS dictionary_entries (
                id TEXT PRIMARY KEY,
                phrase TEXT NOT NULL,
                replacement TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                tag TEXT
            )",
            [],
        )?;

        // 1. Auto-migrate from legacy rift.db if empty
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM dictionary_entries", [], |r| r.get(0)).unwrap_or(0);
        if count == 0 {
            let legacy_db = app_dir.join("rift.db");
            if legacy_db.exists() {
                if let Ok(legacy_conn) = Connection::open(&legacy_db) {
                    if let Ok(mut stmt) = legacy_conn.prepare("SELECT id, phrase, replacement, enabled, tag FROM dictionary_entries") {
                        if let Ok(rows) = stmt.query_map([], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, i32>(3)?,
                                row.get::<_, Option<String>>(4)?,
                            ))
                        }) {
                            for item in rows.flatten() {
                                let _ = conn.execute(
                                    "INSERT OR IGNORE INTO dictionary_entries (id, phrase, replacement, enabled, tag) VALUES (?1, ?2, ?3, ?4, ?5)",
                                    params![item.0, item.1, item.2, item.3, item.4],
                                );
                            }
                        }
                    }
                }
            }

            // 2. Auto-restore from Documents\Rift Backups if still empty
            let count_after_legacy: i64 = conn.query_row("SELECT COUNT(*) FROM dictionary_entries", [], |r| r.get(0)).unwrap_or(0);
            if count_after_legacy == 0 {
                let backup_json = get_user_backup_dir().join("dictionary_backup.json");
                if backup_json.exists() {
                    if let Ok(content) = std::fs::read_to_string(&backup_json) {
                        if let Ok(entries) = serde_json::from_str::<Vec<DictionaryEntry>>(&content) {
                            for e in entries {
                                let _ = conn.execute(
                                    "INSERT OR IGNORE INTO dictionary_entries (id, phrase, replacement, enabled, tag) VALUES (?1, ?2, ?3, ?4, ?5)",
                                    params![e.id, e.phrase, e.replacement, if e.enabled { 1 } else { 0 }, e.tag],
                                );
                            }
                        }
                    }
                }
            }
        }

        let store = Self {
            db_path,
            cache: std::sync::RwLock::new(None),
        };
        store.backup_to_safe_location();
        Ok(store)
    }

    pub fn backup_to_safe_location(&self) {
        let db_path = self.db_path.clone();
        std::thread::spawn(move || {
            let backup_dir = get_user_backup_dir();
            let backup_db_file = backup_dir.join("dictionary_backup.db");
            let backup_json_file = backup_dir.join("dictionary_backup.json");

            let _ = std::fs::copy(&db_path, &backup_db_file);

            if let Ok(conn) = Connection::open(&db_path) {
                if let Ok(mut stmt) = conn.prepare("SELECT id, phrase, replacement, enabled, tag FROM dictionary_entries ORDER BY rowid DESC") {
                    if let Ok(rows) = stmt.query_map([], |row| {
                        Ok(DictionaryEntry {
                            id: row.get(0)?,
                            phrase: row.get(1)?,
                            replacement: row.get(2)?,
                            enabled: row.get::<_, i32>(3)? != 0,
                            tag: row.get(4)?,
                        })
                    }) {
                        let entries: Vec<DictionaryEntry> = rows.flatten().collect();
                        if let Ok(json_str) = serde_json::to_string_pretty(&entries) {
                            let _ = std::fs::write(&backup_json_file, json_str);
                        }
                    }
                }
            }
        });
    }

    pub fn list_entries(&self) -> Result<Vec<DictionaryEntry>> {
        if let Ok(guard) = self.cache.read() {
            if let Some(ref cached) = *guard {
                return Ok(cached.clone());
            }
        }

        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, phrase, replacement, enabled, tag FROM dictionary_entries ORDER BY rowid DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DictionaryEntry {
                id: row.get(0)?,
                phrase: row.get(1)?,
                replacement: row.get(2)?,
                enabled: row.get::<_, i32>(3)? != 0,
                tag: row.get(4)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            if let Ok(entry) = row {
                entries.push(entry);
            }
        }

        if let Ok(mut guard) = self.cache.write() {
            *guard = Some(entries.clone());
        }

        Ok(entries)
    }

    pub fn add_entry(
        &self,
        phrase: &str,
        replacement: &str,
        tag: Option<&str>,
    ) -> Result<DictionaryEntry> {
        let id = format!("dict_{}", chrono::Utc::now().timestamp_millis());
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO dictionary_entries (id, phrase, replacement, enabled, tag) VALUES (?1, ?2, ?3, 1, ?4)",
            params![id, phrase, replacement, tag],
        )?;
        let entry = DictionaryEntry {
            id,
            phrase: phrase.to_string(),
            replacement: replacement.to_string(),
            enabled: true,
            tag: tag.map(|s| s.to_string()),
        };
        if let Ok(mut guard) = self.cache.write() {
            *guard = None;
        }
        self.backup_to_safe_location();
        Ok(entry)
    }

    pub fn update_entry(
        &self,
        id: &str,
        phrase: Option<&str>,
        replacement: Option<&str>,
        enabled: Option<bool>,
        tag: Option<&str>,
    ) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        if let Some(p) = phrase {
            conn.execute(
                "UPDATE dictionary_entries SET phrase = ?1 WHERE id = ?2",
                params![p, id],
            )?;
        }
        if let Some(r) = replacement {
            conn.execute(
                "UPDATE dictionary_entries SET replacement = ?1 WHERE id = ?2",
                params![r, id],
            )?;
        }
        if let Some(e) = enabled {
            conn.execute(
                "UPDATE dictionary_entries SET enabled = ?1 WHERE id = ?2",
                params![if e { 1 } else { 0 }, id],
            )?;
        }
        if let Some(t) = tag {
            conn.execute(
                "UPDATE dictionary_entries SET tag = ?1 WHERE id = ?2",
                params![t, id],
            )?;
        }
        if let Ok(mut guard) = self.cache.write() {
            *guard = None;
        }
        self.backup_to_safe_location();
        Ok(())
    }

    pub fn delete_entry(&self, id: &str) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "DELETE FROM dictionary_entries WHERE id = ?1",
            params![id],
        )?;
        if let Ok(mut guard) = self.cache.write() {
            *guard = None;
        }
        self.backup_to_safe_location();
        Ok(())
    }
}

pub fn apply_dictionary(text: &str, entries: &[DictionaryEntry]) -> String {
    let mut result = text.to_string();
    let mut lower_result = text.to_lowercase();
    for entry in entries {
        if !entry.enabled || entry.phrase.trim().is_empty() {
            continue;
        }
        let trimmed_phrase = entry.phrase.trim();
        let lower_phrase = trimmed_phrase.to_lowercase();
        if !lower_result.contains(&lower_phrase) {
            continue;
        }

        let phrase_escaped = regex::escape(trimmed_phrase);
        let first_is_word = trimmed_phrase.chars().next().map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false);
        let last_is_word = trimmed_phrase.chars().last().map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false);

        let pattern_str = format!(
            "{}{}{}",
            if first_is_word { r"\b" } else { "" },
            phrase_escaped,
            if last_is_word { r"\b" } else { "" }
        );

        if let Ok(re) = regex::RegexBuilder::new(&pattern_str).case_insensitive(true).build() {
            let replaced = re.replace_all(&result, entry.replacement.as_str()).to_string();
            if replaced != result {
                result = replaced;
                lower_result = result.to_lowercase();
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_dictionary_replaces_correctly() {
        let entries = vec![
            DictionaryEntry {
                id: "1".into(),
                phrase: "btw".into(),
                replacement: "by the way".into(),
                enabled: true,
                tag: None,
            },
            DictionaryEntry {
                id: "2".into(),
                phrase: "k8s".into(),
                replacement: "Kubernetes".into(),
                enabled: true,
                tag: None,
            },
            DictionaryEntry {
                id: "3".into(),
                phrase: "disabled_word".into(),
                replacement: "should_not_show".into(),
                enabled: false,
                tag: None,
            },
        ];

        let input = "Hello btw, we are deploying on k8s disabled_word.";
        let out = apply_dictionary(input, &entries);
        assert_eq!(out, "Hello by the way, we are deploying on Kubernetes disabled_word.");
    }

    #[test]
    fn test_apply_dictionary_case_insensitive() {
        let entries = vec![
            DictionaryEntry {
                id: "1".into(),
                phrase: "pr".into(),
                replacement: "Pull Request".into(),
                enabled: true,
                tag: None,
            },
        ];

        let input = "I opened a PR today.";
        let out = apply_dictionary(input, &entries);
        assert_eq!(out, "I opened a Pull Request today.");
    }
}
