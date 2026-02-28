use looprs::observability;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, Row};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone)]
pub struct PersistedChatMessage {
    pub role: String,
    pub content: String,
}

fn db_path() -> PathBuf {
    observability::observability_root().join("desktop_chat.sqlite3")
}

async fn open_pool() -> Result<sqlx::Pool<sqlx::Sqlite>, sqlx::Error> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let options = SqliteConnectOptions::from_str("sqlite::memory:")?
        .filename(path)
        .create_if_missing(true)
        .disable_statement_logging();

    let pool = SqlitePoolOptions::new()
        .max_connections(4)  // Changed from 1 - prevents UI blocking
        .connect_with(options)
        .await?;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS chat_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        ",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS app_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            payload TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        ",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

pub async fn load_chat_messages(limit: usize) -> Vec<PersistedChatMessage> {
    let Ok(pool) = open_pool().await else {
        return Vec::new();
    };

    let Ok(rows) = sqlx::query(
        "
        SELECT role, content
        FROM chat_messages
        ORDER BY id DESC
        LIMIT ?
        ",
    )
    .bind(limit as i64)
    .fetch_all(&pool)
    .await
    else {
        return Vec::new();
    };

    let mut messages = Vec::new();
    for row in rows {
        let Ok(role) = row.try_get::<String, _>("role") else {
            continue;
        };
        let Ok(content) = row.try_get::<String, _>("content") else {
            continue;
        };
        messages.push(PersistedChatMessage { role, content });
    }

    messages.reverse();
    messages
}

pub async fn append_chat_message(role: &str, content: &str) {
    let Ok(pool) = open_pool().await else {
        return;
    };

    let _ = sqlx::query("INSERT INTO chat_messages (role, content) VALUES (?, ?)")
        .bind(role)
        .bind(content)
        .execute(&pool)
        .await;
}

pub async fn clear_chat_messages() {
    let Ok(pool) = open_pool().await else {
        return;
    };

    let _ = sqlx::query("DELETE FROM chat_messages")
        .execute(&pool)
        .await;
}

pub async fn append_observability_event(kind: &str, payload: &str) {
    let Ok(pool) = open_pool().await else {
        return;
    };

    let _ = sqlx::query("INSERT INTO app_events (kind, payload) VALUES (?, ?)")
        .bind(kind)
        .bind(payload)
        .execute(&pool)
        .await;
}
