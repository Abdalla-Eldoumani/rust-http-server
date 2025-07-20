use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use crate::search::query::{DateRange, SortField, SortOrder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilters {
    pub tags: Vec<String>,
    pub created_date_range: Option<DateRange>,
    pub updated_date_range: Option<DateRange>,
    pub created_by: Option<i64>,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            tags: Vec::new(),
            created_date_range: None,
            updated_date_range: None,
            created_by: None,
        }
    }
}

impl SearchFilters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_created_date_range(mut self, start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> Self {
        self.created_date_range = Some(DateRange { start, end });
        self
    }

    pub fn with_updated_date_range(mut self, start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> Self {
        self.updated_date_range = Some(DateRange { start, end });
        self
    }

    pub fn with_created_by(mut self, user_id: i64) -> Self {
        self.created_by = Some(user_id);
        self
    }

    pub fn has_filters(&self) -> bool {
        !self.tags.is_empty() || self.created_date_range.is_some() || self.updated_date_range.is_some() || self.created_by.is_some()
    }

    pub fn build_where_clause(&self) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        if !self.tags.is_empty() {
            let tag_conditions: Vec<String> = self.tags.iter()
                .map(|_| "tags LIKE ?".to_string())
                .collect();
            conditions.push(format!("({})", tag_conditions.join(" OR ")));
            
            for tag in &self.tags {
                params.push(format!("%\"{}\"%%", tag));
            }
        }

        if let Some(ref date_range) = self.created_date_range {
            if let Some(start) = date_range.start {
                conditions.push("created_at >= ?".to_string());
                params.push(start.to_rfc3339());
            }
            if let Some(end) = date_range.end {
                conditions.push("created_at <= ?".to_string());
                params.push(end.to_rfc3339());
            }
        }

        if let Some(ref date_range) = self.updated_date_range {
            if let Some(start) = date_range.start {
                conditions.push("updated_at >= ?".to_string());
                params.push(start.to_rfc3339());
            }
            if let Some(end) = date_range.end {
                conditions.push("updated_at <= ?".to_string());
                params.push(end.to_rfc3339());
            }
        }

        if let Some(user_id) = self.created_by {
            conditions.push("created_by = ?".to_string());
            params.push(user_id.to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_search_filters_default() {
        let filters = SearchFilters::default();
        assert!(filters.tags.is_empty());
        assert!(filters.created_date_range.is_none());
        assert!(filters.updated_date_range.is_none());
        assert!(filters.created_by.is_none());
        assert!(!filters.has_filters());
    }

    #[test]
    fn test_search_filters_builder() {
        let now = Utc::now();
        let filters = SearchFilters::new()
            .with_tags(vec!["tag1".to_string(), "tag2".to_string()])
            .with_created_date_range(Some(now), None)
            .with_created_by(1);

        assert_eq!(filters.tags.len(), 2);
        assert!(filters.created_date_range.is_some());
        assert_eq!(filters.created_by, Some(1));
        assert!(filters.has_filters());
    }

    #[test]
    fn test_build_where_clause_empty() {
        let filters = SearchFilters::new();
        let (where_clause, params) = filters.build_where_clause();
        assert!(where_clause.is_empty());
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_where_clause_with_tags() {
        let filters = SearchFilters::new()
            .with_tags(vec!["tag1".to_string(), "tag2".to_string()]);
        let (where_clause, params) = filters.build_where_clause();
        
        assert!(where_clause.contains("WHERE"));
        assert!(where_clause.contains("tags LIKE ?"));
        assert_eq!(params.len(), 2);
        assert!(params[0].contains("tag1"));
        assert!(params[1].contains("tag2"));
    }

    #[test]
    fn test_build_where_clause_with_date_range() {
        let now = Utc::now();
        let filters = SearchFilters::new()
            .with_created_date_range(Some(now), None);
        let (where_clause, params) = filters.build_where_clause();
        
        assert!(where_clause.contains("WHERE"));
        assert!(where_clause.contains("created_at >= ?"));
        assert_eq!(params.len(), 1);
    }
}