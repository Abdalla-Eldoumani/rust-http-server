use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::store::Item;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub tags: Vec<String>,
    pub created_date_range: Option<DateRange>,
    pub updated_date_range: Option<DateRange>,
    pub sort_criteria: Vec<SortCriterion>,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
    pub fuzzy: bool,
    pub created_by: Option<i64>,
    pub min_relevance: Option<f64>,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            tags: Vec::new(),
            created_date_range: None,
            updated_date_range: None,
            sort_criteria: vec![SortCriterion {
                field: SortField::CreatedAt,
                order: SortOrder::Desc,
            }],
            offset: None,
            limit: None,
            fuzzy: false,
            created_by: None,
            min_relevance: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortField {
    Name,
    CreatedAt,
    UpdatedAt,
    Relevance,
}

impl std::fmt::Display for SortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortField::Name => write!(f, "name"),
            SortField::CreatedAt => write!(f, "created_at"),
            SortField::UpdatedAt => write!(f, "updated_at"),
            SortField::Relevance => write!(f, "rank"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortCriterion {
    pub field: SortField,
    pub order: SortOrder,
}

impl std::fmt::Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortOrder::Asc => write!(f, "ASC"),
            SortOrder::Desc => write!(f, "DESC"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub items: Vec<SearchResultItem>,
    pub total_count: u64,
    pub offset: u64,
    pub limit: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub item: Item,
    pub relevance_score: Option<f64>,
    pub matched_fields: Vec<String>,
}

impl SearchQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_text(mut self, text: String) -> Self {
        self.text = Some(text);
        self
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

    pub fn with_sort(mut self, field: SortField, order: SortOrder) -> Self {
        self.sort_criteria = vec![SortCriterion { field, order }];
        self
    }

    pub fn with_multiple_sort(mut self, criteria: Vec<SortCriterion>) -> Self {
        self.sort_criteria = criteria;
        self
    }

    pub fn add_sort(mut self, field: SortField, order: SortOrder) -> Self {
        self.sort_criteria.push(SortCriterion { field, order });
        self
    }

    pub fn with_pagination(mut self, offset: u64, limit: u64) -> Self {
        self.offset = Some(offset);
        self.limit = Some(limit);
        self
    }

    pub fn with_fuzzy(mut self, fuzzy: bool) -> Self {
        self.fuzzy = fuzzy;
        self
    }

    pub fn with_created_by(mut self, user_id: i64) -> Self {
        self.created_by = Some(user_id);
        self
    }

    pub fn with_min_relevance(mut self, min_score: f64) -> Self {
        self.min_relevance = Some(min_score);
        self
    }
}

impl SearchResultItem {
    pub fn new(item: Item) -> Self {
        Self {
            item,
            relevance_score: None,
            matched_fields: Vec::new(),
        }
    }

    pub fn with_relevance(mut self, score: f64) -> Self {
        self.relevance_score = Some(score);
        self
    }

    pub fn with_matched_fields(mut self, fields: Vec<String>) -> Self {
        self.matched_fields = fields;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_query_default() {
        let query = SearchQuery::default();
        assert!(query.text.is_none());
        assert!(query.tags.is_empty());
        assert!(query.created_date_range.is_none());
        assert!(query.updated_date_range.is_none());
        assert_eq!(query.sort_criteria.len(), 1);
        assert!(matches!(query.sort_criteria[0].field, SortField::CreatedAt));
        assert!(matches!(query.sort_criteria[0].order, SortOrder::Desc));
        assert!(!query.fuzzy);
        assert!(query.created_by.is_none());
        assert!(query.min_relevance.is_none());
    }

    #[test]
    fn test_search_query_builder() {
        let now = Utc::now();
        let query = SearchQuery::new()
            .with_text("test query".to_string())
            .with_tags(vec!["tag1".to_string(), "tag2".to_string()])
            .with_created_date_range(Some(now), None)
            .with_sort(SortField::Name, SortOrder::Asc)
            .add_sort(SortField::CreatedAt, SortOrder::Desc)
            .with_pagination(10, 20)
            .with_fuzzy(true)
            .with_created_by(1)
            .with_min_relevance(0.5);

        assert_eq!(query.text, Some("test query".to_string()));
        assert_eq!(query.tags.len(), 2);
        assert!(query.created_date_range.is_some());
        assert_eq!(query.sort_criteria.len(), 2);
        assert_eq!(query.offset, Some(10));
        assert_eq!(query.limit, Some(20));
        assert!(query.fuzzy);
        assert_eq!(query.created_by, Some(1));
        assert_eq!(query.min_relevance, Some(0.5));
    }

    #[test]
    fn test_sort_field_display() {
        assert_eq!(SortField::Name.to_string(), "name");
        assert_eq!(SortField::CreatedAt.to_string(), "created_at");
        assert_eq!(SortField::UpdatedAt.to_string(), "updated_at");
        assert_eq!(SortField::Relevance.to_string(), "rank");
    }

    #[test]
    fn test_sort_order_display() {
        assert_eq!(SortOrder::Asc.to_string(), "ASC");
        assert_eq!(SortOrder::Desc.to_string(), "DESC");
    }
}