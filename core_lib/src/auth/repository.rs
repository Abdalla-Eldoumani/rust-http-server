use crate::auth::models::{CreateUserRequest, User, UserRole};
use crate::error::AppError;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Row, SqlitePool};

#[async_trait]
pub trait UserRepositoryTrait {
    async fn create_user(&self, request: &CreateUserRequest, password_hash: &str) -> Result<User, AppError>;
    async fn get_user_by_id(&self, id: i64) -> Result<Option<User>, AppError>;
    async fn get_user_by_username(&self, username: &str) -> Result<Option<User>, AppError>;
    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>, AppError>;
    async fn update_last_login(&self, user_id: i64) -> Result<(), AppError>;
    async fn update_user_status(&self, user_id: i64, is_active: bool) -> Result<(), AppError>;
    async fn list_users(&self, limit: Option<i64>, offset: Option<i64>) -> Result<Vec<User>, AppError>;
    async fn delete_user(&self, user_id: i64) -> Result<(), AppError>;
}

#[derive(Clone)]
pub struct UserRepository {
    pool: SqlitePool,
}

impl UserRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn ensure_tables_exist(&self) -> Result<(), AppError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'user',
                created_at TEXT NOT NULL,
                last_login TEXT,
                is_active BOOLEAN NOT NULL DEFAULT 1
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to create users table: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)")
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to create username index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)")
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to create email index: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl UserRepositoryTrait for UserRepository {
    async fn create_user(&self, request: &CreateUserRequest, password_hash: &str) -> Result<User, AppError> {
        let now = Utc::now();
        let role = request.role.as_ref().unwrap_or(&UserRole::User).to_string();

        let result = sqlx::query(
            r#"
            INSERT INTO users (username, email, password_hash, role, created_at, is_active)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&request.username)
        .bind(&request.email)
        .bind(password_hash)
        .bind(&role)
        .bind(now.to_rfc3339())
        .bind(true)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                if e.to_string().contains("username") {
                    AppError::BadRequest("Username already exists".to_string())
                } else if e.to_string().contains("email") {
                    AppError::BadRequest("Email already exists".to_string())
                } else {
                    AppError::BadRequest("User already exists".to_string())
                }
            } else {
                AppError::Database(format!("Failed to create user: {}", e))
            }
        })?;

        let user_id = result.last_insert_rowid();

        Ok(User {
            id: user_id,
            username: request.username.clone(),
            email: request.email.clone(),
            password_hash: password_hash.to_string(),
            role,
            created_at: now,
            last_login: None,
            is_active: true,
        })
    }

    async fn get_user_by_id(&self, id: i64) -> Result<Option<User>, AppError> {
        let row = sqlx::query(
            "SELECT id, username, email, password_hash, role, created_at, last_login, is_active FROM users WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to get user by ID: {}", e)))?;

        if let Some(row) = row {
            let created_at: String = row.get("created_at");
            let last_login: Option<String> = row.get("last_login");

            Ok(Some(User {
                id: row.get("id"),
                username: row.get("username"),
                email: row.get("email"),
                password_hash: row.get("password_hash"),
                role: row.get("role"),
                created_at: created_at.parse().map_err(|e| {
                    AppError::Database(format!("Failed to parse created_at: {}", e))
                })?,
                last_login: last_login.map(|s| s.parse()).transpose().map_err(|e| {
                    AppError::Database(format!("Failed to parse last_login: {}", e))
                })?,
                is_active: row.get("is_active"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_user_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        let row = sqlx::query(
            "SELECT id, username, email, password_hash, role, created_at, last_login, is_active FROM users WHERE username = ?"
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to get user by username: {}", e)))?;

        if let Some(row) = row {
            let created_at: String = row.get("created_at");
            let last_login: Option<String> = row.get("last_login");

            Ok(Some(User {
                id: row.get("id"),
                username: row.get("username"),
                email: row.get("email"),
                password_hash: row.get("password_hash"),
                role: row.get("role"),
                created_at: created_at.parse().map_err(|e| {
                    AppError::Database(format!("Failed to parse created_at: {}", e))
                })?,
                last_login: last_login.map(|s| s.parse()).transpose().map_err(|e| {
                    AppError::Database(format!("Failed to parse last_login: {}", e))
                })?,
                is_active: row.get("is_active"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
        let row = sqlx::query(
            "SELECT id, username, email, password_hash, role, created_at, last_login, is_active FROM users WHERE email = ?"
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to get user by email: {}", e)))?;

        if let Some(row) = row {
            let created_at: String = row.get("created_at");
            let last_login: Option<String> = row.get("last_login");

            Ok(Some(User {
                id: row.get("id"),
                username: row.get("username"),
                email: row.get("email"),
                password_hash: row.get("password_hash"),
                role: row.get("role"),
                created_at: created_at.parse().map_err(|e| {
                    AppError::Database(format!("Failed to parse created_at: {}", e))
                })?,
                last_login: last_login.map(|s| s.parse()).transpose().map_err(|e| {
                    AppError::Database(format!("Failed to parse last_login: {}", e))
                })?,
                is_active: row.get("is_active"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_last_login(&self, user_id: i64) -> Result<(), AppError> {
        let now = Utc::now();
        sqlx::query("UPDATE users SET last_login = ? WHERE id = ?")
            .bind(now.to_rfc3339())
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to update last login: {}", e)))?;

        Ok(())
    }

    async fn update_user_status(&self, user_id: i64, is_active: bool) -> Result<(), AppError> {
        sqlx::query("UPDATE users SET is_active = ? WHERE id = ?")
            .bind(is_active)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to update user status: {}", e)))?;

        Ok(())
    }

    async fn list_users(&self, limit: Option<i64>, offset: Option<i64>) -> Result<Vec<User>, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let rows = sqlx::query(
            "SELECT id, username, email, password_hash, role, created_at, last_login, is_active 
             FROM users ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to list users: {}", e)))?;

        let mut users = Vec::new();
        for row in rows {
            let created_at: String = row.get("created_at");
            let last_login: Option<String> = row.get("last_login");

            users.push(User {
                id: row.get("id"),
                username: row.get("username"),
                email: row.get("email"),
                password_hash: row.get("password_hash"),
                role: row.get("role"),
                created_at: created_at.parse().map_err(|e| {
                    AppError::Database(format!("Failed to parse created_at: {}", e))
                })?,
                last_login: last_login.map(|s| s.parse()).transpose().map_err(|e| {
                    AppError::Database(format!("Failed to parse last_login: {}", e))
                })?,
                is_active: row.get("is_active"),
            });
        }

        Ok(users)
    }

    async fn delete_user(&self, user_id: i64) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to delete user: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("User not found".to_string()));
        }

        Ok(())
    }
}