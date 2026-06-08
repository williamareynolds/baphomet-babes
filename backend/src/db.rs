use anyhow::Result;
use libsql::Connection;

pub async fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS users (
            id          TEXT PRIMARY KEY,
            email       TEXT UNIQUE NOT NULL,
            username    TEXT NOT NULL,
            password_hash TEXT NOT NULL,
            role        TEXT NOT NULL DEFAULT 'member',
            created_at  INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS movie_nights (
            id            TEXT PRIMARY KEY,
            event_type    TEXT NOT NULL CHECK(event_type IN ('main', 'special')),
            title         TEXT NOT NULL,
            date          TEXT NOT NULL,
            description   TEXT,
            poll_embed_url TEXT,
            created_at    INTEGER NOT NULL
        );
        ",
    )
    .await?;
    Ok(())
}
