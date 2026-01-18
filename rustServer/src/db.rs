use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::Path;

pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // Create database file if it doesn't exist
    if database_url.starts_with("sqlite:") {
        let db_path = database_url.trim_start_matches("sqlite:");
        if !Path::new(db_path).exists() {
            std::fs::File::create(db_path).ok();
        }
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // Run migrations
    init_schema(&pool).await?;

    Ok(pool)
}

async fn init_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS download_files (
            id TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,
            display_name TEXT NOT NULL,
            description TEXT,
            is_protected INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS download_tokens (
            id TEXT PRIMARY KEY,
            token TEXT UNIQUE NOT NULL,
            file_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            used INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (file_id) REFERENCES download_files(id),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
