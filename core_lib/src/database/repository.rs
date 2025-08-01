use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use chrono::Utc;
use crate::error::{AppError, Result};
use crate::database::models::*;
use crate::store::Item;

#[async_trait]
pub trait Repository<T> {
    type Id;
    type CreateInput;
    type UpdateInput;

    async fn create(&self, input: Self::CreateInput) -> Result<T>;
    async fn get_by_id(&self, id: Self::Id) -> Result<Option<T>>;
    async fn update(&self, id: Self::Id, input: Self::UpdateInput) -> Result<T>;
    async fn delete(&self, id: Self::Id) -> Result<()>;
    async fn list(&self, params: ListParams) -> Result<Vec<T>>;
    async fn count(&self) -> Result<i64>;
}

#[derive(Debug, Clone)]
pub struct ListParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<SortOrder>,
}

impl Default for ListParams {
    fn default() -> Self {
        Self {
            limit: Some(50),
            offset: Some(0),
            sort_by: None,
            sort_order: Some(SortOrder::Asc),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SortOrder {
    Asc,
    Desc,
}

impl std::fmt::Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortOrder::Asc => write!(f, "ASC"),
            SortOrder::Desc => write!(f, "DESC"),
        }
    }
}

#[derive(Clone)]
pub struct ItemRepository {
    pool: SqlitePool,
}

impl ItemRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn begin_transaction(&self) -> Result<sqlx::Transaction<'_, sqlx::Sqlite>> {
        self.pool.begin().await.map_err(AppError::from)
    }

    pub async fn search(&self, query: &str, params: ListParams) -> Result<Vec<Item>> {
        let limit = params.limit.unwrap_or(50);
        let offset = params.offset.unwrap_or(0);

        let rows = sqlx::query(r#"
            SELECT i.id, i.name, i.description, i.created_at, i.updated_at, i.tags, i.metadata, i.created_by
            FROM items i
            JOIN items_fts fts ON i.id = fts.rowid
            WHERE items_fts MATCH ?
            ORDER BY rank
            LIMIT ? OFFSET ?
        "#)
        .bind(query)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        let mut items = Vec::new();
        for row in rows {
            let db_item = DbItem {
                id: row.try_get("id").unwrap_or(0),
                name: row.try_get("name").unwrap_or_default(),
                description: row.try_get("description").unwrap_or(None),
                created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                updated_at: row.try_get("updated_at").unwrap_or_else(|_| Utc::now()),
                tags: row.try_get("tags").unwrap_or_else(|_| "[]".to_string()),
                metadata: row.try_get("metadata").unwrap_or_else(|_| "{}".to_string()),
                created_by: row.try_get("created_by").unwrap_or(None),
            };
            items.push(db_item.to_api_item());
        }

        Ok(items)
    }

    pub async fn get_by_tags(&self, tags: &[String], params: ListParams) -> Result<Vec<Item>> {
        let limit = params.limit.unwrap_or(50);
        let offset = params.offset.unwrap_or(0);

        let tag_conditions: Vec<String> = tags.iter()
            .map(|tag| format!("JSON_EXTRACT(tags, '$[*]') LIKE '%{}%'", tag))
            .collect();
        let where_clause = tag_conditions.join(" OR ");

        let query = format!(r#"
            SELECT id, name, description, created_at, updated_at, tags, metadata, created_by
            FROM items
            WHERE {}
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
        "#, where_clause);

        let rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        let mut items = Vec::new();
        for row in rows {
            let db_item = DbItem {
                id: row.try_get("id").unwrap_or(0),
                name: row.try_get("name").unwrap_or_default(),
                description: row.try_get("description").unwrap_or(None),
                created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                updated_at: row.try_get("updated_at").unwrap_or_else(|_| Utc::now()),
                tags: row.try_get("tags").unwrap_or_else(|_| "[]".to_string()),
                metadata: row.try_get("metadata").unwrap_or_else(|_| "{}".to_string()),
                created_by: row.try_get("created_by").unwrap_or(None),
            };
            items.push(db_item.to_api_item());
        }

        Ok(items)
    }

    async fn create_item_internal(&self, input: &CreateItemInput) -> Result<Item> {
        let now = Utc::now();
        let tags_json = serde_json::to_string(&input.tags)
            .unwrap_or_else(|_| "[]".to_string());
        let metadata_json = input.metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
            .unwrap_or_else(|| "{}".to_string());

        let row = sqlx::query(r#"
            INSERT INTO items (name, description, created_at, updated_at, tags, metadata, created_by)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            RETURNING id, name, description, created_at, updated_at, tags, metadata, created_by
        "#)
        .bind(&input.name)
        .bind(&input.description)
        .bind(now)
        .bind(now)
        .bind(&tags_json)
        .bind(&metadata_json)
        .bind(input.created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        let db_item = DbItem {
            id: row.try_get("id").unwrap_or(0),
            name: row.try_get("name").unwrap_or_default(),
            description: row.try_get("description").unwrap_or(None),
            created_at: row.try_get("created_at").unwrap_or(now),
            updated_at: row.try_get("updated_at").unwrap_or(now),
            tags: row.try_get("tags").unwrap_or_else(|_| "[]".to_string()),
            metadata: row.try_get("metadata").unwrap_or_else(|_| "{}".to_string()),
            created_by: row.try_get("created_by").unwrap_or(None),
        };

        Ok(db_item.to_api_item())
    }
}

#[derive(Debug, Clone)]
pub struct CreateItemInput {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct UpdateItemInput {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub metadata: Option<serde_json::Value>,
}

#[async_trait]
impl Repository<Item> for ItemRepository {
    type Id = i64;
    type CreateInput = CreateItemInput;
    type UpdateInput = UpdateItemInput;

    async fn create(&self, input: Self::CreateInput) -> Result<Item> {
        self.create_item_internal(&input).await
    }

    async fn get_by_id(&self, id: Self::Id) -> Result<Option<Item>> {
        let row = sqlx::query(r#"
            SELECT id, name, description, created_at, updated_at, tags, metadata, created_by
            FROM items
            WHERE id = ?
        "#)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        match row {
            Some(row) => {
                let db_item = DbItem {
                    id: row.try_get("id").unwrap_or(0),
                    name: row.try_get("name").unwrap_or_default(),
                    description: row.try_get("description").unwrap_or(None),
                    created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                    updated_at: row.try_get("updated_at").unwrap_or_else(|_| Utc::now()),
                    tags: row.try_get("tags").unwrap_or_else(|_| "[]".to_string()),
                    metadata: row.try_get("metadata").unwrap_or_else(|_| "{}".to_string()),
                    created_by: row.try_get("created_by").unwrap_or(None),
                };
                Ok(Some(db_item.to_api_item()))
            }
            None => Ok(None),
        }
    }

    async fn update(&self, id: Self::Id, input: Self::UpdateInput) -> Result<Item> {
        let now = Utc::now();
        let tags_json = serde_json::to_string(&input.tags)
            .unwrap_or_else(|_| "[]".to_string());
        let metadata_json = input.metadata
            .map(|m| serde_json::to_string(&m).unwrap_or_else(|_| "{}".to_string()))
            .unwrap_or_else(|| "{}".to_string());

        let row = sqlx::query(r#"
            UPDATE items 
            SET name = ?, description = ?, updated_at = ?, tags = ?, metadata = ?
            WHERE id = ?
            RETURNING id, name, description, created_at, updated_at, tags, metadata, created_by
        "#)
        .bind(&input.name)
        .bind(&input.description)
        .bind(now)
        .bind(&tags_json)
        .bind(&metadata_json)
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        let db_item = DbItem {
            id: row.try_get("id").unwrap_or(0),
            name: row.try_get("name").unwrap_or_default(),
            description: row.try_get("description").unwrap_or(None),
            created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
            updated_at: row.try_get("updated_at").unwrap_or(now),
            tags: row.try_get("tags").unwrap_or_else(|_| "[]".to_string()),
            metadata: row.try_get("metadata").unwrap_or_else(|_| "{}".to_string()),
            created_by: row.try_get("created_by").unwrap_or(None),
        };

        Ok(db_item.to_api_item())
    }

    async fn delete(&self, id: Self::Id) -> Result<()> {
        let result = sqlx::query("DELETE FROM items WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Item with id {} not found", id)));
        }

        Ok(())
    }

    async fn list(&self, params: ListParams) -> Result<Vec<Item>> {
        let limit = params.limit.unwrap_or(49);
        let offset = params.offset.unwrap_or(0);
        let sort_by = params.sort_by.as_deref().unwrap_or("created_at");
        let sort_order = params.sort_order.as_ref().unwrap_or(&SortOrder::Desc);

        let allowed_sort_fields = ["id", "name", "created_at", "updated_at"];
        let safe_sort_by = if allowed_sort_fields.contains(&sort_by) {
            sort_by
        } else {
            "created_at"
        };

        let query = format!(r#"
            SELECT id, name, description, created_at, updated_at, tags, metadata, created_by
            FROM items
            ORDER BY {} {}
            LIMIT ? OFFSET ?
        "#, safe_sort_by, sort_order);

        let rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                tracing::error!("Database query failed: query={}, limit={}, offset={}, error={}", query, limit, offset, e);
                AppError::from(e)
            })?;

        let mut items = Vec::new();
        for row in rows {
            let db_item = DbItem {
                id: row.try_get("id").unwrap_or(0),
                name: row.try_get("name").unwrap_or_default(),
                description: row.try_get("description").unwrap_or(None),
                created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                updated_at: row.try_get("updated_at").unwrap_or_else(|_| Utc::now()),
                tags: row.try_get("tags").unwrap_or_else(|_| "[]".to_string()),
                metadata: row.try_get("metadata").unwrap_or_else(|_| "{}".to_string()),
                created_by: row.try_get("created_by").unwrap_or(None),
            };
            items.push(db_item.to_api_item());
        }

        Ok(items)
    }

    async fn count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM items")
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(row.try_get("count").unwrap_or(0))
    }
}

#[derive(Clone)]
pub struct UserRepository {
    pool: SqlitePool,
}

impl UserRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_by_username(&self, username: &str) -> Result<Option<DbUser>> {
        let row = sqlx::query(r#"
            SELECT id, username, email, password_hash, role, created_at, last_login, is_active
            FROM users
            WHERE username = ?
        "#)
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        match row {
            Some(row) => {
                let user = DbUser {
                    id: row.try_get("id").unwrap_or(0),
                    username: row.try_get("username").unwrap_or_default(),
                    email: row.try_get("email").unwrap_or_default(),
                    password_hash: row.try_get("password_hash").unwrap_or_default(),
                    role: row.try_get("role").unwrap_or_else(|_| "User".to_string()),
                    created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                    last_login: row.try_get("last_login").unwrap_or(None),
                    is_active: row.try_get("is_active").unwrap_or(true),
                };
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<DbUser>> {
        let row = sqlx::query(r#"
            SELECT id, username, email, password_hash, role, created_at, last_login, is_active
            FROM users
            WHERE email = ?
        "#)
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        match row {
            Some(row) => {
                let user = DbUser {
                    id: row.try_get("id").unwrap_or(0),
                    username: row.try_get("username").unwrap_or_default(),
                    email: row.try_get("email").unwrap_or_default(),
                    password_hash: row.try_get("password_hash").unwrap_or_default(),
                    role: row.try_get("role").unwrap_or_else(|_| "User".to_string()),
                    created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                    last_login: row.try_get("last_login").unwrap_or(None),
                    is_active: row.try_get("is_active").unwrap_or(true),
                };
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    pub async fn update_last_login(&self, user_id: i64) -> Result<()> {
        sqlx::query("UPDATE users SET last_login = ? WHERE id = ?")
            .bind(Utc::now())
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CreateUserInput {
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: UserRole,
}

#[derive(Debug, Clone)]
pub struct UpdateUserInput {
    pub email: Option<String>,
    pub role: Option<UserRole>,
    pub is_active: Option<bool>,
}

#[async_trait]
impl Repository<DbUser> for UserRepository {
    type Id = i64;
    type CreateInput = CreateUserInput;
    type UpdateInput = UpdateUserInput;

    async fn create(&self, input: Self::CreateInput) -> Result<DbUser> {
        let now = Utc::now();

        let row = sqlx::query(r#"
            INSERT INTO users (username, email, password_hash, role, created_at, is_active)
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING id, username, email, password_hash, role, created_at, last_login, is_active
        "#)
        .bind(&input.username)
        .bind(&input.email)
        .bind(&input.password_hash)
        .bind(input.role.to_string())
        .bind(now)
        .bind(true)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        let user = DbUser {
            id: row.try_get("id").unwrap_or(0),
            username: row.try_get("username").unwrap_or_default(),
            email: row.try_get("email").unwrap_or_default(),
            password_hash: row.try_get("password_hash").unwrap_or_default(),
            role: row.try_get("role").unwrap_or_else(|_| "User".to_string()),
            created_at: row.try_get("created_at").unwrap_or(now),
            last_login: row.try_get("last_login").unwrap_or(None),
            is_active: row.try_get("is_active").unwrap_or(true),
        };

        Ok(user)
    }

    async fn get_by_id(&self, id: Self::Id) -> Result<Option<DbUser>> {
        let row = sqlx::query(r#"
            SELECT id, username, email, password_hash, role, created_at, last_login, is_active
            FROM users
            WHERE id = ?
        "#)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        match row {
            Some(row) => {
                let user = DbUser {
                    id: row.try_get("id").unwrap_or(0),
                    username: row.try_get("username").unwrap_or_default(),
                    email: row.try_get("email").unwrap_or_default(),
                    password_hash: row.try_get("password_hash").unwrap_or_default(),
                    role: row.try_get("role").unwrap_or_else(|_| "User".to_string()),
                    created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                    last_login: row.try_get("last_login").unwrap_or(None),
                    is_active: row.try_get("is_active").unwrap_or(true),
                };
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    async fn update(&self, id: Self::Id, input: Self::UpdateInput) -> Result<DbUser> {
        let mut query_parts = Vec::new();
        let mut bind_values: Vec<Box<dyn sqlx::Encode<'_, sqlx::Sqlite> + Send + '_>> = Vec::new();

        if let Some(email) = &input.email {
            query_parts.push("email = ?");
            bind_values.push(Box::new(email));
        }

        if let Some(role) = &input.role {
            query_parts.push("role = ?");
            bind_values.push(Box::new(role.to_string()));
        }

        if let Some(is_active) = input.is_active {
            query_parts.push("is_active = ?");
            bind_values.push(Box::new(is_active));
        }

        if query_parts.is_empty() {
            return self.get_by_id(id).await?.ok_or_else(|| {
                AppError::NotFound(format!("User with id {} not found", id))
            });
        }

        let query = format!(r#"
            UPDATE users 
            SET {}
            WHERE id = ?
            RETURNING id, username, email, password_hash, role, created_at, last_login, is_active
        "#, query_parts.join(", "));

        let mut query_builder = sqlx::query(&query);
        
        if let Some(email) = &input.email {
            query_builder = query_builder.bind(email);
        }
        if let Some(role) = &input.role {
            query_builder = query_builder.bind(role.to_string());
        }
        if let Some(is_active) = input.is_active {
            query_builder = query_builder.bind(is_active);
        }
        query_builder = query_builder.bind(id);

        let row = query_builder
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;

        let user = DbUser {
            id: row.try_get("id").unwrap_or(0),
            username: row.try_get("username").unwrap_or_default(),
            email: row.try_get("email").unwrap_or_default(),
            password_hash: row.try_get("password_hash").unwrap_or_default(),
            role: row.try_get("role").unwrap_or_else(|_| "User".to_string()),
            created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
            last_login: row.try_get("last_login").unwrap_or(None),
            is_active: row.try_get("is_active").unwrap_or(true),
        };

        Ok(user)
    }

    async fn delete(&self, id: Self::Id) -> Result<()> {
        let result = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("User with id {} not found", id)));
        }

        Ok(())
    }

    async fn list(&self, params: ListParams) -> Result<Vec<DbUser>> {
        let limit = params.limit.unwrap_or(50);
        let offset = params.offset.unwrap_or(0);
        let sort_by = params.sort_by.as_deref().unwrap_or("created_at");
        let sort_order = params.sort_order.as_ref().unwrap_or(&SortOrder::Desc);

        let query = format!(r#"
            SELECT id, username, email, password_hash, role, created_at, last_login, is_active
            FROM users
            ORDER BY {} {}
            LIMIT ? OFFSET ?
        "#, sort_by, sort_order);

        let rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        let mut users = Vec::new();
        for row in rows {
            let user = DbUser {
                id: row.try_get("id").unwrap_or(0),
                username: row.try_get("username").unwrap_or_default(),
                email: row.try_get("email").unwrap_or_default(),
                password_hash: row.try_get("password_hash").unwrap_or_default(),
                role: row.try_get("role").unwrap_or_else(|_| "User".to_string()),
                created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                last_login: row.try_get("last_login").unwrap_or(None),
                is_active: row.try_get("is_active").unwrap_or(true),
            };
            users.push(user);
        }

        Ok(users)
    }

    async fn count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM users")
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(row.try_get("count").unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use crate::database::{get_database_pool, run_migrations};

    async fn setup_test_db() -> SqlitePool {
        let temp_file = NamedTempFile::new().unwrap();
        let database_url = format!("sqlite:{}", temp_file.path().display());
        
        let pool = get_database_pool(&database_url).await.unwrap();
        run_migrations(pool.clone()).await.unwrap();
        
        pool
    }

    #[tokio::test]
    async fn test_item_repository_crud() {
        let pool = setup_test_db().await;
        let repo = ItemRepository::new(pool);

        let create_input = CreateItemInput {
            name: "Test Item".to_string(),
            description: Some("Test Description".to_string()),
            tags: vec!["test".to_string(), "demo".to_string()],
            metadata: Some(serde_json::json!({"key": "value"})),
            created_by: None,
        };

        let created_item = repo.create(create_input).await.unwrap();
        assert_eq!(created_item.name, "Test Item");
        assert_eq!(created_item.tags, vec!["test", "demo"]);

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let retrieved_item = repo.get_by_id(created_item.id as i64).await.unwrap();
        assert!(retrieved_item.is_some());
        let retrieved_item = retrieved_item.unwrap();
        assert_eq!(retrieved_item.name, "Test Item");

        let update_input = UpdateItemInput {
            name: "Updated Item".to_string(),
            description: Some("Updated Description".to_string()),
            tags: vec!["updated".to_string()],
            metadata: None,
        };

        let updated_item = repo.update(created_item.id as i64, update_input).await.unwrap();
        assert_eq!(updated_item.name, "Updated Item");
        assert_eq!(updated_item.tags, vec!["updated"]);

        let items = repo.list(ListParams::default()).await.unwrap();
        assert_eq!(items.len(), 1);

        let count = repo.count().await.unwrap();
        assert_eq!(count, 1);

        repo.delete(created_item.id as i64).await.unwrap();
        let deleted_item = repo.get_by_id(created_item.id as i64).await.unwrap();
        assert!(deleted_item.is_none());
    }

    #[tokio::test]
    async fn test_user_repository_crud() {
        let pool = setup_test_db().await;
        let repo = UserRepository::new(pool);

        let create_input = CreateUserInput {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hashed_password".to_string(),
            role: UserRole::User,
        };

        let created_user = repo.create(create_input).await.unwrap();
        assert_eq!(created_user.username, "testuser");
        assert_eq!(created_user.email, "test@example.com");

        // Add a small delay to ensure WAL mode consistency
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let user_by_username = repo.get_by_username("testuser").await.unwrap();
        assert!(user_by_username.is_some());

        let user_by_email = repo.get_by_email("test@example.com").await.unwrap();
        assert!(user_by_email.is_some());

        repo.update_last_login(created_user.id).await.unwrap();

        let count = repo.count().await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_transaction_support() {
        let pool = setup_test_db().await;
        let repo = ItemRepository::new(pool);

        let mut tx = repo.begin_transaction().await.unwrap();
        
        let create_input = CreateItemInput {
            name: "Transaction Test".to_string(),
            description: None,
            tags: vec![],
            metadata: None,
            created_by: None,
        };

        sqlx::query(r#"
            INSERT INTO items (name, description, created_at, updated_at, tags, metadata, created_by)
            VALUES (?, ?, ?, ?, ?, ?, ?)
        "#)
        .bind(&create_input.name)
        .bind(&create_input.description)
        .bind(Utc::now())
        .bind(Utc::now())
        .bind("[]")
        .bind("{}")
        .bind(create_input.created_by)
        .execute(&mut *tx)
        .await
        .unwrap();

        tx.rollback().await.unwrap();

        let count = repo.count().await.unwrap();
        assert_eq!(count, 0);
    }
}