use sqlx::{SqlitePool, Row};
use tracing::{debug, error};
use regex::Regex;
use crate::error::{AppError, Result};
use crate::search::{SearchQuery, SearchResult, SearchResultItem, SortField};
use crate::database::models::DbItem;
use crate::store::Item;

#[derive(Clone)]
pub struct SearchEngine {
    pool: SqlitePool,
    fuzzy_regex: Regex,
    cache: Option<crate::search::cache::SearchCache>,
}

impl SearchEngine {
    pub fn new(pool: SqlitePool) -> Self {
        let fuzzy_regex = Regex::new(r"[a-zA-Z]").unwrap();
        
        Self {
            pool,
            fuzzy_regex,
            cache: None,
        }
    }

    pub fn with_cache(mut self, cache: crate::search::cache::SearchCache) -> Self {
        self.cache = Some(cache);
        self
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchResult> {
        debug!("Executing search query: {:?}", query);

        if let Some(ref cache) = self.cache {
            if let Some(cached_result) = cache.get(query) {
                debug!("Cache hit for search query");
                return Ok(cached_result);
            }
        }

        let (items, total_count) = if query.text.is_some() {
            self.full_text_search(query).await?
        } else {
            self.filter_search(query).await?
        };

        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(50);
        let has_more = (offset + items.len() as u64) < total_count;

        let result = SearchResult {
            items,
            total_count,
            offset,
            limit,
            has_more,
        };

        if let Some(ref cache) = self.cache {
            cache.put(query, result.clone());
        }

        Ok(result)
    }

    async fn full_text_search(&self, query: &SearchQuery) -> Result<(Vec<SearchResultItem>, u64)> {
        let search_text = query.text.as_ref().unwrap();
        let processed_text = if query.fuzzy {
            self.process_fuzzy_query(search_text)
        } else {
            search_text.clone()
        };

        debug!("Full-text search for: '{}' (processed: '{}')", search_text, processed_text);

        let fts_query = self.build_fts_query(&processed_text);
        
        let (filter_clause, filter_params) = self.build_filter_clause(query);
        
        let sort_clause = self.build_sort_clause(&query.sort_criteria);
        
        let limit = query.limit.unwrap_or(50);
        let offset = query.offset.unwrap_or(0);

        let search_sql = format!(
            r#"
            SELECT i.*, fts.rank
            FROM items_fts fts
            JOIN items i ON i.id = fts.rowid
            {}
            {}
            LIMIT ? OFFSET ?
            "#,
            filter_clause,
            sort_clause
        );

        let count_sql = format!(
            r#"
            SELECT COUNT(*) as total
            FROM items_fts fts
            JOIN items i ON i.id = fts.rowid
            {}
            "#,
            filter_clause
        );

        let mut search_query = sqlx::query(&search_sql);
        search_query = search_query.bind(&fts_query);
        
        for param in &filter_params {
            search_query = search_query.bind(param);
        }
        
        search_query = search_query.bind(limit as i64).bind(offset as i64);

        let rows = search_query.fetch_all(&self.pool).await.map_err(AppError::from)?;

        let mut count_query = sqlx::query(&count_sql);
        count_query = count_query.bind(&fts_query);
        
        for param in &filter_params {
            count_query = count_query.bind(param);
        }

        let count_row = count_query.fetch_one(&self.pool).await.map_err(AppError::from)?;
        let total_count: i64 = count_row.try_get("total").unwrap_or(0);

        let mut items = Vec::new();
        for row in rows {
            let db_item = DbItem {
                id: row.try_get("id").unwrap_or(0),
                name: row.try_get("name").unwrap_or_default(),
                description: row.try_get("description").ok(),
                created_at: row.try_get("created_at").unwrap_or_else(|_| chrono::Utc::now()),
                updated_at: row.try_get("updated_at").unwrap_or_else(|_| chrono::Utc::now()),
                tags: row.try_get("tags").unwrap_or_else(|_| "[]".to_string()),
                metadata: row.try_get("metadata").unwrap_or_else(|_| "{}".to_string()),
                created_by: row.try_get("created_by").ok(),
            };

            let item = db_item.to_api_item();
            let rank: f64 = row.try_get("rank").unwrap_or(0.0);
            
            let matched_fields = self.identify_matched_fields(&item, search_text);
            
            let result_item = SearchResultItem::new(item)
                .with_relevance(rank)
                .with_matched_fields(matched_fields);
            
            items.push(result_item);
        }

        Ok((items, total_count as u64))
    }

    async fn filter_search(&self, query: &SearchQuery) -> Result<(Vec<SearchResultItem>, u64)> {
        debug!("Filter-only search");

        let (filter_clause, filter_params) = self.build_filter_clause(query);
        
        let sort_clause = self.build_sort_clause(&query.sort_criteria);
        
        let limit = query.limit.unwrap_or(50);
        let offset = query.offset.unwrap_or(0);

        let search_sql = format!(
            r#"
            SELECT *
            FROM items
            {}
            {}
            LIMIT ? OFFSET ?
            "#,
            filter_clause,
            sort_clause
        );

        let count_sql = format!(
            r#"
            SELECT COUNT(*) as total
            FROM items
            {}
            "#,
            filter_clause
        );

        let mut search_query = sqlx::query(&search_sql);
        for param in &filter_params {
            search_query = search_query.bind(param);
        }
        search_query = search_query.bind(limit as i64).bind(offset as i64);

        let rows = search_query.fetch_all(&self.pool).await.map_err(AppError::from)?;

        let mut count_query = sqlx::query(&count_sql);
        for param in &filter_params {
            count_query = count_query.bind(param);
        }

        let count_row = count_query.fetch_one(&self.pool).await.map_err(AppError::from)?;
        let total_count: i64 = count_row.try_get("total").unwrap_or(0);

        let mut items = Vec::new();
        for row in rows {
            let db_item = DbItem {
                id: row.try_get("id").unwrap_or(0),
                name: row.try_get("name").unwrap_or_default(),
                description: row.try_get("description").ok(),
                created_at: row.try_get("created_at").unwrap_or_else(|_| chrono::Utc::now()),
                updated_at: row.try_get("updated_at").unwrap_or_else(|_| chrono::Utc::now()),
                tags: row.try_get("tags").unwrap_or_else(|_| "[]".to_string()),
                metadata: row.try_get("metadata").unwrap_or_else(|_| "{}".to_string()),
                created_by: row.try_get("created_by").ok(),
            };

            let item = db_item.to_api_item();
            let result_item = SearchResultItem::new(item);
            items.push(result_item);
        }

        Ok((items, total_count as u64))
    }

    fn build_fts_query(&self, text: &str) -> String {
        let cleaned = text.trim();
        if cleaned.is_empty() {
            return "*".to_string();
        }

        if cleaned.contains(' ') {
            format!("\"{}\"", cleaned.replace('"', ""))
        } else {
            cleaned.to_string()
        }
    }

    fn build_filter_clause(&self, query: &SearchQuery) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        if query.text.is_some() {
            conditions.push("fts.items_fts MATCH ?".to_string());
        }

        if !query.tags.is_empty() {
            let tag_conditions: Vec<String> = query.tags.iter()
                .map(|_| "i.tags LIKE ?".to_string())
                .collect();
            conditions.push(format!("({})", tag_conditions.join(" OR ")));
            
            for tag in &query.tags {
                params.push(format!("%\"{}\"%%", tag));
            }
        }

        if let Some(ref date_range) = query.created_date_range {
            if let Some(start) = date_range.start {
                conditions.push("i.created_at >= ?".to_string());
                params.push(start.to_rfc3339());
            }
            if let Some(end) = date_range.end {
                conditions.push("i.created_at <= ?".to_string());
                params.push(end.to_rfc3339());
            }
        }

        if let Some(ref date_range) = query.updated_date_range {
            if let Some(start) = date_range.start {
                conditions.push("i.updated_at >= ?".to_string());
                params.push(start.to_rfc3339());
            }
            if let Some(end) = date_range.end {
                conditions.push("i.updated_at <= ?".to_string());
                params.push(end.to_rfc3339());
            }
        }

        if let Some(created_by) = query.created_by {
            conditions.push("i.created_by = ?".to_string());
            params.push(created_by.to_string());
        }

        if query.text.is_some() && query.min_relevance.is_some() {
            conditions.push("fts.rank >= ?".to_string());
            params.push(query.min_relevance.unwrap().to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, params)
    }

    fn build_sort_clause(&self, sort_criteria: &[crate::search::query::SortCriterion]) -> String {
        if sort_criteria.is_empty() {
            return "ORDER BY i.created_at DESC".to_string();
        }

        let sort_parts: Vec<String> = sort_criteria.iter()
            .map(|criterion| {
                let field = match criterion.field {
                    SortField::Name => "i.name",
                    SortField::CreatedAt => "i.created_at",
                    SortField::UpdatedAt => "i.updated_at",
                    SortField::Relevance => "fts.rank",
                };
                format!("{} {}", field, criterion.order)
            })
            .collect();

        format!("ORDER BY {}", sort_parts.join(", "))
    }

    fn process_fuzzy_query(&self, text: &str) -> String {
        let mut fuzzy_text = text.to_lowercase();
        
        fuzzy_text = fuzzy_text.replace("teh", "the");
        fuzzy_text = fuzzy_text.replace("adn", "and");
        fuzzy_text = fuzzy_text.replace("recieve", "receive");
        
        fuzzy_text
    }

    fn identify_matched_fields(&self, item: &Item, search_text: &str) -> Vec<String> {
        let mut matched_fields = Vec::new();
        let search_lower = search_text.to_lowercase();

        if item.name.to_lowercase().contains(&search_lower) {
            matched_fields.push("name".to_string());
        }

        if let Some(ref description) = item.description {
            if description.to_lowercase().contains(&search_lower) {
                matched_fields.push("description".to_string());
            }
        }

        for tag in &item.tags {
            if tag.to_lowercase().contains(&search_lower) {
                matched_fields.push("tags".to_string());
                break;
            }
        }

        matched_fields
    }

    pub async fn health_check(&self) -> Result<bool> {
        let result = sqlx::query("SELECT COUNT(*) as count FROM items_fts")
            .fetch_one(&self.pool)
            .await;

        match result {
            Ok(_) => {
                debug!("Search engine health check passed");
                Ok(true)
            }
            Err(e) => {
                error!("Search engine health check failed: {}", e);
                Ok(false)
            }
        }
    }

    pub async fn rebuild_index(&self) -> Result<()> {
        debug!("Rebuilding FTS index");
        
        sqlx::query("INSERT INTO items_fts(items_fts) VALUES('rebuild')")
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;

        if let Some(ref cache) = self.cache {
            cache.invalidate_all();
        }

        debug!("FTS index rebuilt successfully");
        Ok(())
    }

    pub fn invalidate_cache(&self) {
        if let Some(ref cache) = self.cache {
            cache.invalidate_all();
        }
    }

    pub fn invalidate_cache_pattern(&self, pattern: &str) {
        if let Some(ref cache) = self.cache {
            cache.invalidate_by_pattern(pattern);
        }
    }

    pub fn get_cache_stats(&self) -> Option<crate::search::cache::SearchCacheStats> {
        self.cache.as_ref().map(|cache| cache.get_stats())
    }

    pub fn cleanup_cache(&self) {
        if let Some(ref cache) = self.cache {
            cache.cleanup_expired();
        }
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
    async fn test_search_engine_creation() {
        let pool = setup_test_db().await;
        let engine = SearchEngine::new(pool);
        
        let health = engine.health_check().await.unwrap();
        assert!(health);
    }

    #[tokio::test]
    async fn test_build_fts_query() {
        let pool = setup_test_db().await;
        let engine = SearchEngine::new(pool);
        
        assert_eq!(engine.build_fts_query("test"), "test");
        assert_eq!(engine.build_fts_query("test query"), "\"test query\"");
        assert_eq!(engine.build_fts_query(""), "*");
    }

    #[tokio::test]
    async fn test_process_fuzzy_query() {
        let pool = setup_test_db().await;
        let engine = SearchEngine::new(pool);
        
        assert_eq!(engine.process_fuzzy_query("teh test"), "the test");
        assert_eq!(engine.process_fuzzy_query("adn more"), "and more");
    }

    #[tokio::test]
    async fn test_identify_matched_fields() {
        let pool = setup_test_db().await;
        let engine = SearchEngine::new(pool);
        
        let item = Item {
            id: 1,
            name: "Test Item".to_string(),
            description: Some("This is a test description".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            tags: vec!["test".to_string(), "example".to_string()],
            metadata: None,
        };

        let matched = engine.identify_matched_fields(&item, "test");
        assert!(matched.contains(&"name".to_string()));
        assert!(matched.contains(&"description".to_string()));
        assert!(matched.contains(&"tags".to_string()));

        let matched = engine.identify_matched_fields(&item, "nonexistent");
        assert!(matched.is_empty());
    }
}