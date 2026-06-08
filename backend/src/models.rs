// Internal DB row types — not exposed in API responses
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
}
