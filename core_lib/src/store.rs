//! In-memory data store for the application

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: u64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub tags: Vec<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct DataStore {
    items: Arc<RwLock<HashMap<u64, Item>>>,
    next_id: Arc<RwLock<u64>>,
}

impl DataStore {
    pub fn new() -> Self {
        let mut initial_items = HashMap::new();
        
        initial_items.insert(1, Item {
            id: 1,
            name: "Sample Item 1".to_string(),
            description: Some("This is a sample item".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            tags: vec!["sample".to_string(), "demo".to_string()],
            metadata: Some(serde_json::json!({"category": "electronics", "price": 99.99})),
        });
        
        initial_items.insert(2, Item {
            id: 2,
            name: "Sample Item 2".to_string(),
            description: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            tags: vec!["demo".to_string()],
            metadata: None,
        });

        Self {
            items: Arc::new(RwLock::new(initial_items)),
            next_id: Arc::new(RwLock::new(3)),
        }
    }

    pub fn get_items(&self, limit: Option<usize>, offset: Option<usize>) -> Result<Vec<Item>> {
        let items = self.items.read()
            .map_err(|_| AppError::InternalServerError)?;
        
        let mut all_items: Vec<Item> = items.values().cloned().collect();
        all_items.sort_by(|a, b| a.id.cmp(&b.id));
        
        let offset = offset.unwrap_or(0);
        let limit = limit.unwrap_or(all_items.len());
        
        Ok(all_items
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect())
    }

    pub fn get_item(&self, id: u64) -> Result<Item> {
        let items = self.items.read()
            .map_err(|_| AppError::InternalServerError)?;
        
        items.get(&id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Item with id {} not found", id)))
    }

    pub fn create_item(&self, name: String, description: Option<String>, tags: Vec<String>, metadata: Option<serde_json::Value>) -> Result<Item> {
        let mut items = self.items.write()
            .map_err(|_| AppError::InternalServerError)?;
        
        let mut next_id = self.next_id.write()
            .map_err(|_| AppError::InternalServerError)?;
        
        let id = *next_id;
        *next_id += 1;
        
        let now = chrono::Utc::now();
        let item = Item {
            id,
            name,
            description,
            created_at: now,
            updated_at: now,
            tags,
            metadata,
        };
        
        items.insert(id, item.clone());
        Ok(item)
    }

    pub fn update_item(&self, id: u64, name: String, description: Option<String>, tags: Vec<String>, metadata: Option<serde_json::Value>) -> Result<Item> {
        let mut items = self.items.write()
            .map_err(|_| AppError::InternalServerError)?;
        
        let item = items.get_mut(&id)
            .ok_or_else(|| AppError::NotFound(format!("Item with id {} not found", id)))?;
        
        item.name = name;
        item.description = description;
        item.tags = tags;
        item.metadata = metadata;
        item.updated_at = chrono::Utc::now();
        
        Ok(item.clone())
    }

    pub fn patch_item(&self, id: u64, updates: HashMap<String, serde_json::Value>) -> Result<Item> {
        let mut items = self.items.write()
            .map_err(|_| AppError::InternalServerError)?;
        
        let item = items.get_mut(&id)
            .ok_or_else(|| AppError::NotFound(format!("Item with id {} not found", id)))?;
        
        if let Some(name) = updates.get("name").and_then(|v| v.as_str()) {
            item.name = name.to_string();
        }
        
        if let Some(desc) = updates.get("description") {
            if desc.is_null() {
                item.description = None;
            } else if let Some(desc_str) = desc.as_str() {
                item.description = Some(desc_str.to_string());
            }
        }
        
        if let Some(tags) = updates.get("tags").and_then(|v| v.as_array()) {
            item.tags = tags.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        
        if let Some(metadata) = updates.get("metadata") {
            if metadata.is_null() {
                item.metadata = None;
            } else {
                item.metadata = Some(metadata.clone());
            }
        }
        
        item.updated_at = chrono::Utc::now();
        
        Ok(item.clone())
    }

    pub fn delete_item(&self, id: u64) -> Result<()> {
        let mut items = self.items.write()
            .map_err(|_| AppError::InternalServerError)?;
        
        items.remove(&id)
            .ok_or_else(|| AppError::NotFound(format!("Item with id {} not found", id)))?;
        
        Ok(())
    }

    pub fn get_stats(&self) -> Result<serde_json::Value> {
        let items = self.items.read()
            .map_err(|_| AppError::InternalServerError)?;
        
        let total_items = items.len();
        let tags: std::collections::HashSet<String> = items.values()
            .flat_map(|item| item.tags.iter().cloned())
            .collect();
        
        Ok(serde_json::json!({
            "total_items": total_items,
            "unique_tags": tags.len(),
            "tags": tags,
        }))
    }
}

impl Default for DataStore {
    fn default() -> Self {
        Self::new()
    }
}