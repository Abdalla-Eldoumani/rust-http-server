use chrono::{DateTime, Utc};
use crate::search::{SearchQuery, SortField, SortOrder};

#[derive(Debug, Clone)]
pub struct AdvancedFilterBuilder {
    query: SearchQuery,
}

impl AdvancedFilterBuilder {
    pub fn new() -> Self {
        Self {
            query: SearchQuery::new(),
        }
    }

    pub fn search_text(mut self, text: &str, fuzzy: bool) -> Self {
        self.query = self.query.with_text(text.to_string()).with_fuzzy(fuzzy);
        self
    }

    pub fn filter_by_tags(mut self, tags: Vec<&str>) -> Self {
        let tag_strings: Vec<String> = tags.into_iter().map(|s| s.to_string()).collect();
        self.query = self.query.with_tags(tag_strings);
        self
    }

    pub fn created_between(mut self, start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> Self {
        self.query = self.query.with_created_date_range(start, end);
        self
    }

    pub fn updated_between(mut self, start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> Self {
        self.query = self.query.with_updated_date_range(start, end);
        self
    }

    pub fn created_by_user(mut self, user_id: i64) -> Self {
        self.query = self.query.with_created_by(user_id);
        self
    }

    pub fn min_relevance(mut self, score: f64) -> Self {
        self.query = self.query.with_min_relevance(score);
        self
    }

    pub fn sort_by(mut self, field: SortField, order: SortOrder) -> Self {
        self.query = self.query.with_sort(field, order);
        self
    }

    pub fn then_sort_by(mut self, field: SortField, order: SortOrder) -> Self {
        self.query = self.query.add_sort(field, order);
        self
    }

    pub fn paginate(mut self, offset: u64, limit: u64) -> Self {
        self.query = self.query.with_pagination(offset, limit);
        self
    }

    pub fn build(self) -> SearchQuery {
        self.query
    }
}

impl Default for AdvancedFilterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SearchPatterns;

impl SearchPatterns {
    pub fn recent_items(days: i64, limit: u64) -> SearchQuery {
        let since = Utc::now() - chrono::Duration::days(days);
        AdvancedFilterBuilder::new()
            .created_between(Some(since), None)
            .sort_by(SortField::CreatedAt, SortOrder::Desc)
            .paginate(0, limit)
            .build()
    }

    pub fn recently_updated(days: i64, limit: u64) -> SearchQuery {
        let since = Utc::now() - chrono::Duration::days(days);
        AdvancedFilterBuilder::new()
            .updated_between(Some(since), None)
            .sort_by(SortField::UpdatedAt, SortOrder::Desc)
            .paginate(0, limit)
            .build()
    }

    pub fn by_tags_with_relevance(tags: Vec<&str>, text: Option<&str>) -> SearchQuery {
        let mut builder = AdvancedFilterBuilder::new()
            .filter_by_tags(tags);

        if let Some(search_text) = text {
            builder = builder
                .search_text(search_text, true)
                .sort_by(SortField::Relevance, SortOrder::Desc)
                .then_sort_by(SortField::CreatedAt, SortOrder::Desc);
        } else {
            builder = builder.sort_by(SortField::CreatedAt, SortOrder::Desc);
        }

        builder.build()
    }

    pub fn by_user(user_id: i64, limit: u64) -> SearchQuery {
        AdvancedFilterBuilder::new()
            .created_by_user(user_id)
            .sort_by(SortField::CreatedAt, SortOrder::Desc)
            .paginate(0, limit)
            .build()
    }

    pub fn high_relevance_search(text: &str, min_score: f64) -> SearchQuery {
        AdvancedFilterBuilder::new()
            .search_text(text, true)
            .min_relevance(min_score)
            .sort_by(SortField::Relevance, SortOrder::Desc)
            .then_sort_by(SortField::CreatedAt, SortOrder::Desc)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advanced_filter_builder() {
        let now = Utc::now();
        let query = AdvancedFilterBuilder::new()
            .search_text("test", true)
            .filter_by_tags(vec!["tag1", "tag2"])
            .created_between(Some(now), None)
            .sort_by(SortField::Relevance, SortOrder::Desc)
            .then_sort_by(SortField::Name, SortOrder::Asc)
            .paginate(10, 20)
            .build();

        assert_eq!(query.text, Some("test".to_string()));
        assert_eq!(query.tags.len(), 2);
        assert!(query.created_date_range.is_some());
        assert_eq!(query.sort_criteria.len(), 2);
        assert!(query.fuzzy);
        assert_eq!(query.offset, Some(10));
        assert_eq!(query.limit, Some(20));
    }

    #[test]
    fn test_search_patterns_recent_items() {
        let query = SearchPatterns::recent_items(7, 50);
        assert!(query.created_date_range.is_some());
        assert_eq!(query.limit, Some(50));
        assert_eq!(query.offset, Some(0));
    }

    #[test]
    fn test_search_patterns_by_tags() {
        let query = SearchPatterns::by_tags_with_relevance(vec!["rust", "web"], Some("server"));
        assert_eq!(query.tags.len(), 2);
        assert_eq!(query.text, Some("server".to_string()));
        assert!(query.fuzzy);
        assert_eq!(query.sort_criteria.len(), 2);
    }

    #[test]
    fn test_search_patterns_high_relevance() {
        let query = SearchPatterns::high_relevance_search("important query", 0.8);
        assert_eq!(query.text, Some("important query".to_string()));
        assert_eq!(query.min_relevance, Some(0.8));
        assert!(query.fuzzy);
    }
}