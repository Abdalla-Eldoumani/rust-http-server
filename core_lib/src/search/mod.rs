pub mod engine;
pub mod filters;
pub mod query;
pub mod advanced_filters;
pub mod cache;

pub use engine::SearchEngine;
pub use filters::{SearchFilters, DateRange};
pub use query::{SearchQuery, SearchResult, SearchResultItem, SortField, SortOrder, SortCriterion};
pub use advanced_filters::{AdvancedFilterBuilder, SearchPatterns};
pub use cache::{SearchCache, SearchCacheStats};