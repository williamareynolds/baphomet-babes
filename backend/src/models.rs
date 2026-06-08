use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDoc {
    pub id: String,
    pub email: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDoc {
    pub id: String,
    pub event_type: String,
    pub title: String,
    pub date: String,
    pub description: Option<String>,
    pub poll_embed_url: Option<String>,
    pub created_at: i64,
}
