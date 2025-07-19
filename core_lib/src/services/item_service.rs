use crate::{
    database::{ItemRepository, Repository, CreateItemInput, UpdateItemInput, ListParams},
    store::{DataStore, Item},
    error::{AppError, Result},
};
use std::collections::HashMap;

#[derive(Clone)]
pub struct ItemService {
    item_repository: Option<ItemRepository>,
    data_store: DataStore,
    use_database: bool,
}

impl ItemService {
    pub fn with_database(item_repository: ItemRepository, data_store: DataStore) -> Self {
        Self {
            item_repository: Some(item_repository),
            data_store,
            use_database: true,
        }
    }

    pub fn with_memory_store(data_store: DataStore) -> Self {
        Self {
            item_repository: None,
            data_store,
            use_database: false,
        }
    }

    pub async fn get_items(&self, limit: Option<usize>, offset: Option<usize>) -> Result<Vec<Item>> {
        if self.use_database {
            if let Some(repo) = &self.item_repository {
                let params = ListParams {
                    limit: limit.map(|l| l as i64),
                    offset: offset.map(|o| o as i64),
                    sort_by: Some("created_at".to_string()),
                    sort_order: Some(crate::database::SortOrder::Desc),
                };
                return repo.list(params).await;
            }
        }

        self.data_store.get_items(limit, offset)
    }

    pub async fn get_item(&self, id: u64) -> Result<Item> {
        if self.use_database {
            if let Some(repo) = &self.item_repository {
                return match repo.get_by_id(id as i64).await? {
                    Some(item) => Ok(item),
                    None => Err(AppError::NotFound(format!("Item with id {} not found", id))),
                };
            }
        }

        self.data_store.get_item(id)
    }

    pub async fn create_item(
        &self,
        name: String,
        description: Option<String>,
        tags: Vec<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<Item> {
        if self.use_database {
            if let Some(repo) = &self.item_repository {
                let input = CreateItemInput {
                    name,
                    description,
                    tags,
                    metadata,
                    created_by: None,
                };
                return repo.create(input).await;
            }
        }

        self.data_store.create_item(name, description, tags, metadata)
    }

    pub async fn update_item(
        &self,
        id: u64,
        name: String,
        description: Option<String>,
        tags: Vec<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<Item> {
        if self.use_database {
            if let Some(repo) = &self.item_repository {
                let input = UpdateItemInput {
                    name,
                    description,
                    tags,
                    metadata,
                };
                return repo.update(id as i64, input).await;
            }
        }

        self.data_store.update_item(id, name, description, tags, metadata)
    }

    pub async fn patch_item(&self, id: u64, updates: HashMap<String, serde_json::Value>) -> Result<Item> {
        if self.use_database {
            if let Some(repo) = &self.item_repository {
                let current_item = match repo.get_by_id(id as i64).await? {
                    Some(item) => item,
                    None => return Err(AppError::NotFound(format!("Item with id {} not found", id))),
                };

                let mut name = current_item.name;
                let mut description = current_item.description;
                let mut tags = current_item.tags;
                let mut metadata = current_item.metadata;

                if let Some(new_name) = updates.get("name").and_then(|v| v.as_str()) {
                    name = new_name.to_string();
                }

                if let Some(desc) = updates.get("description") {
                    if desc.is_null() {
                        description = None;
                    } else if let Some(desc_str) = desc.as_str() {
                        description = Some(desc_str.to_string());
                    }
                }

                if let Some(new_tags) = updates.get("tags").and_then(|v| v.as_array()) {
                    tags = new_tags.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                }

                if let Some(new_metadata) = updates.get("metadata") {
                    if new_metadata.is_null() {
                        metadata = None;
                    } else {
                        metadata = Some(new_metadata.clone());
                    }
                }

                let input = UpdateItemInput {
                    name,
                    description,
                    tags,
                    metadata,
                };

                return repo.update(id as i64, input).await;
            }
        }

        self.data_store.patch_item(id, updates)
    }

    pub async fn delete_item(&self, id: u64) -> Result<()> {
        if self.use_database {
            if let Some(repo) = &self.item_repository {
                return repo.delete(id as i64).await;
            }
        }

        self.data_store.delete_item(id)
    }

    pub async fn get_stats(&self) -> Result<serde_json::Value> {
        if self.use_database {
            if let Some(repo) = &self.item_repository {
                let count = repo.count().await?;
                
                return Ok(serde_json::json!({
                    "total_items": count,
                    "source": "database"
                }));
            }
        }

        let mut stats = self.data_store.get_stats()?;
        if let Some(obj) = stats.as_object_mut() {
            obj.insert("source".to_string(), serde_json::Value::String("memory".to_string()));
        }
        Ok(stats)
    }

    pub fn is_using_database(&self) -> bool {
        self.use_database && self.item_repository.is_some()
    }

    pub fn repository(&self) -> Option<&ItemRepository> {
        self.item_repository.as_ref()
    }

    pub fn data_store(&self) -> &DataStore {
        &self.data_store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use crate::database::{get_database_pool, run_migrations};

    async fn setup_test_repository() -> ItemRepository {
        let temp_file = NamedTempFile::new().unwrap();
        let database_url = format!("sqlite:{}", temp_file.path().display());
        
        let pool = get_database_pool(&database_url).await.unwrap();
        run_migrations(pool.clone()).await.unwrap();
        
        ItemRepository::new(pool)
    }

    #[tokio::test]
    async fn test_item_service_with_database() {
        let repo = setup_test_repository().await;
        let store = DataStore::new();
        let service = ItemService::with_database(repo, store);

        assert!(service.is_using_database());

        let item = service.create_item(
            "Test Item".to_string(),
            Some("Test Description".to_string()),
            vec!["test".to_string()],
            None,
        ).await.unwrap();

        assert_eq!(item.name, "Test Item");

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let retrieved = service.get_item(item.id).await.unwrap();
        assert_eq!(retrieved.name, "Test Item");

        let updated = service.update_item(
            item.id,
            "Updated Item".to_string(),
            Some("Updated Description".to_string()),
            vec!["updated".to_string()],
            None,
        ).await.unwrap();

        assert_eq!(updated.name, "Updated Item");

        let mut patch = HashMap::new();
        patch.insert("name".to_string(), serde_json::Value::String("Patched Item".to_string()));
        
        let patched = service.patch_item(item.id, patch).await.unwrap();
        assert_eq!(patched.name, "Patched Item");

        let items = service.get_items(Some(10), Some(0)).await.unwrap();
        assert_eq!(items.len(), 1);

        let stats = service.get_stats().await.unwrap();
        assert_eq!(stats["total_items"], 1);
        assert_eq!(stats["source"], "database");

        service.delete_item(item.id).await.unwrap();
        let result = service.get_item(item.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_item_service_with_memory_store() {
        let store = DataStore::new();
        let service = ItemService::with_memory_store(store);

        assert!(!service.is_using_database());

        let items = service.get_items(None, None).await.unwrap();
        assert_eq!(items.len(), 2);

        let stats = service.get_stats().await.unwrap();
        assert_eq!(stats["source"], "memory");
    }

    #[tokio::test]
    async fn test_fallback_behavior() {
        let store = DataStore::new();
        let service = ItemService {
            item_repository: None,
            data_store: store,
            use_database: true,
        };

        let items = service.get_items(None, None).await.unwrap();
        assert_eq!(items.len(), 2);
    }
}