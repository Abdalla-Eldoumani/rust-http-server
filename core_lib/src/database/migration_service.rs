use crate::{
    database::{ItemRepository, CreateItemInput, Repository},
    store::{DataStore, Item},
    error::Result,
};
use tracing::{info, warn, error};

pub struct MigrationService {
    item_repository: ItemRepository,
}

impl MigrationService {
    pub fn new(item_repository: ItemRepository) -> Self {
        Self { item_repository }
    }

    pub async fn migrate_from_memory_store(&self, store: &DataStore) -> Result<MigrationResult> {
        info!("Starting migration from in-memory store to database");

        let mut result = MigrationResult::default();

        let items = store.get_items(None, None)?;
        result.total_items = items.len();

        if items.is_empty() {
            info!("No items to migrate");
            return Ok(result);
        }

        info!("Found {} items to migrate", items.len());

        let existing_count = self.item_repository.count().await?;
        if existing_count > 0 {
            warn!("Database already contains {} items. Migration will add new items with new IDs.", existing_count);
        }

        for item in items {
            info!("Migrating item: {} ({})", item.id, item.name);
            match self.migrate_single_item(&item).await {
                Ok(migrated_item) => {
                    info!("Successfully migrated item {} -> {} ({})", item.id, migrated_item.id, migrated_item.name);
                    result.migrated_items.push(ItemMigrationInfo {
                        original_id: item.id,
                        new_id: migrated_item.id,
                        name: item.name.clone(),
                    });
                    result.successful_migrations += 1;
                }
                Err(e) => {
                    error!("Failed to migrate item {}: {}", item.id, e);
                    result.failed_migrations.push(ItemMigrationError {
                        original_id: item.id,
                        name: item.name.clone(),
                        error: e.to_string(),
                    });
                    result.failed_count += 1;
                }
            }
        }

        info!(
            "Migration completed: {} successful, {} failed out of {} total",
            result.successful_migrations,
            result.failed_count,
            result.total_items
        );

        Ok(result)
    }

    async fn migrate_single_item(&self, item: &Item) -> Result<Item> {
        let create_input = CreateItemInput {
            name: item.name.clone(),
            description: item.description.clone(),
            tags: item.tags.clone(),
            metadata: item.metadata.clone(),
            created_by: None,
        };

        let migrated_item = self.item_repository.create(create_input).await?;
        
        Ok(migrated_item)
    }

    pub async fn is_migration_needed(&self, store: &DataStore) -> Result<bool> {
        let memory_items = store.get_items(None, None)?;
        let db_count = self.item_repository.count().await?;

        if db_count == 0 && !memory_items.is_empty() {
            return Ok(true);
        }

        Ok(false)
    }

    pub async fn verify_migration(&self, store: &DataStore) -> Result<MigrationVerification> {
        let memory_items = store.get_items(None, None)?;
        let db_count = self.item_repository.count().await?;
        
        let verification = MigrationVerification {
            memory_store_count: memory_items.len(),
            database_count: db_count as usize,
            counts_match: memory_items.len() <= db_count as usize,
        };

        Ok(verification)
    }
}

#[derive(Debug, Clone)]
pub struct MigrationResult {
    pub total_items: usize,
    pub successful_migrations: usize,
    pub failed_count: usize,
    pub migrated_items: Vec<ItemMigrationInfo>,
    pub failed_migrations: Vec<ItemMigrationError>,
}

impl Default for MigrationResult {
    fn default() -> Self {
        Self {
            total_items: 0,
            successful_migrations: 0,
            failed_count: 0,
            migrated_items: Vec::new(),
            failed_migrations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ItemMigrationInfo {
    pub original_id: u64,
    pub new_id: u64,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ItemMigrationError {
    pub original_id: u64,
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct MigrationVerification {
    pub memory_store_count: usize,
    pub database_count: usize,
    pub counts_match: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use crate::database::{get_database_pool, run_migrations};

    async fn setup_test_db() -> ItemRepository {
        let temp_file = NamedTempFile::new().unwrap();
        let database_url = format!("sqlite:{}", temp_file.path().display());
        
        let pool = get_database_pool(&database_url).await.unwrap();
        run_migrations(pool.clone()).await.unwrap();
        
        ItemRepository::new(pool)
    }

    #[tokio::test]
    async fn test_migration_from_empty_store() {
        let item_repository = setup_test_db().await;
        let migration_service = MigrationService::new(item_repository);
        let empty_store = DataStore::empty();
        
        let result = migration_service.migrate_from_memory_store(&empty_store).await.unwrap();
        
        assert_eq!(result.total_items, 0);
        assert_eq!(result.successful_migrations, 0);
        assert_eq!(result.failed_count, 0);
    }

    #[tokio::test]
    async fn test_migration_with_items() {
        let item_repository = setup_test_db().await;
        let migration_service = MigrationService::new(item_repository);
        let store = DataStore::new();

        let result = migration_service.migrate_from_memory_store(&store).await.unwrap();
        
        assert_eq!(result.total_items, 2);
        assert_eq!(result.successful_migrations, 2);
        assert_eq!(result.failed_count, 0);
        assert_eq!(result.migrated_items.len(), 2);
    }

    #[tokio::test]
    async fn test_migration_needed_check() {
        let item_repository = setup_test_db().await;
        let migration_service = MigrationService::new(item_repository);
        let store = DataStore::new();

        let needed = migration_service.is_migration_needed(&store).await.unwrap();
        assert!(needed);

        migration_service.migrate_from_memory_store(&store).await.unwrap();
        let needed_after = migration_service.is_migration_needed(&store).await.unwrap();
        assert!(!needed_after);
    }

    #[tokio::test]
    async fn test_migration_verification() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let database_url = format!("sqlite:{}", temp_file.path().display());
        
        let pool = crate::database::get_database_pool(&database_url).await.unwrap();
        crate::database::run_migrations(pool.clone()).await.unwrap();
        
        let item_repository = ItemRepository::new(pool);
        let migration_service = MigrationService::new(item_repository);
        let store = DataStore::new();

        let verification_before = migration_service.verify_migration(&store).await.unwrap();
        assert_eq!(verification_before.memory_store_count, 2);
        assert_eq!(verification_before.database_count, 0);
        assert!(!verification_before.counts_match);

        let result = migration_service.migrate_from_memory_store(&store).await.unwrap();
        assert_eq!(result.successful_migrations, 2);
        assert_eq!(result.failed_count, 0);
        
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let verification_after = migration_service.verify_migration(&store).await.unwrap();
        assert_eq!(verification_after.memory_store_count, 2);
        assert_eq!(verification_after.database_count, 2);
        assert!(verification_after.counts_match);
    }
}