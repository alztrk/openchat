//! In-memory hot cache for active conversations (Zed-inspired memory architecture).
//!
//! Avoids redundant SQLite roundtrips for active conversations during chatting,
//! context token estimation, and rapid prompt composition.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Instant,
};

use crate::chatgpt_store::StoredMessage;

/// Default maximum number of conversations retained in RAM.
pub const DEFAULT_MAX_HOT_CONVERSATIONS: usize = 16;

#[derive(Clone, Debug)]
struct CachedConversationEntry {
    messages: Arc<Vec<StoredMessage>>,
    active_leaf_message_id: Option<String>,
    last_accessed: Instant,
}

/// Thread-safe in-memory hot cache for active conversation messages and active leaf state.
#[derive(Debug)]
pub struct ConversationHotCache {
    entries: RwLock<HashMap<String, CachedConversationEntry>>,
    max_conversations: usize,
}

impl Default for ConversationHotCache {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_HOT_CONVERSATIONS)
    }
}

impl ConversationHotCache {
    /// Creates a new hot cache with the specified capacity.
    pub fn new(max_conversations: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::with_capacity(max_conversations.min(64))),
            max_conversations: max_conversations.max(1),
        }
    }

    /// Looks up cached messages for a conversation, updating last accessed timestamp on hit.
    pub fn get(&self, conversation_id: &str) -> Option<(Arc<Vec<StoredMessage>>, Option<String>)> {
        let mut entries = self.entries.write().ok()?;
        if let Some(entry) = entries.get_mut(conversation_id) {
            entry.last_accessed = Instant::now();
            Some((
                Arc::clone(&entry.messages),
                entry.active_leaf_message_id.clone(),
            ))
        } else {
            None
        }
    }

    /// Stores or replaces the cached messages for a conversation.
    /// If capacity is exceeded, evicts the least recently accessed conversation.
    pub fn insert(
        &self,
        conversation_id: String,
        messages: Vec<StoredMessage>,
        active_leaf_message_id: Option<String>,
    ) {
        if let Ok(mut entries) = self.entries.write() {
            if entries.len() >= self.max_conversations && !entries.contains_key(&conversation_id) {
                let oldest_id = entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_accessed)
                    .map(|(id, _)| id.clone());

                if let Some(id) = oldest_id {
                    entries.remove(&id);
                }
            }

            entries.insert(
                conversation_id,
                CachedConversationEntry {
                    messages: Arc::new(messages),
                    active_leaf_message_id,
                    last_accessed: Instant::now(),
                },
            );
        }
    }

    /// Invalidates a specific conversation when its messages or state are mutated.
    pub fn invalidate(&self, conversation_id: &str) {
        if let Ok(mut entries) = self.entries.write() {
            entries.remove(conversation_id);
        }
    }

    /// Clears the entire hot cache.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }

    /// Returns the number of conversations currently cached in memory.
    pub fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }

    /// Returns true if the cache contains no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_message(id: &str, role: &str, content: &str) -> StoredMessage {
        StoredMessage {
            id: id.to_owned(),
            role: role.to_owned(),
            content: content.to_owned(),
            status: "completed".to_owned(),
            output_tokens: Some(10),
            tool_activities: Vec::new(),
            attachments: Vec::new(),
        }
    }

    #[test]
    fn hot_cache_hit_and_miss_lifecycle() {
        let cache = ConversationHotCache::new(4);
        assert!(cache.is_empty());

        let msgs = vec![
            sample_message("m1", "user", "hello"),
            sample_message("m2", "assistant", "world"),
        ];

        cache.insert("conv_1".to_owned(), msgs.clone(), Some("m2".to_owned()));
        assert_eq!(cache.len(), 1);

        let hit = cache.get("conv_1");
        assert!(hit.is_some());
        let (cached_msgs, leaf) = hit.unwrap();
        assert_eq!(cached_msgs.len(), 2);
        assert_eq!(cached_msgs[0].content, "hello");
        assert_eq!(leaf.as_deref(), Some("m2"));

        assert!(cache.get("conv_unknown").is_none());
    }

    #[test]
    fn hot_cache_invalidation() {
        let cache = ConversationHotCache::new(4);
        cache.insert(
            "conv_1".to_owned(),
            vec![sample_message("m1", "user", "test")],
            None,
        );
        assert_eq!(cache.len(), 1);

        cache.invalidate("conv_1");
        assert!(cache.get("conv_1").is_none());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn hot_cache_lru_eviction() {
        let cache = ConversationHotCache::new(2);

        cache.insert(
            "conv_1".to_owned(),
            vec![sample_message("m1", "user", "1")],
            None,
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
        cache.insert(
            "conv_2".to_owned(),
            vec![sample_message("m2", "user", "2")],
            None,
        );
        assert_eq!(cache.len(), 2);

        // Access conv_1 to make conv_2 the least recently accessed
        assert!(cache.get("conv_1").is_some());
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Insert conv_3, which should evict conv_2
        cache.insert(
            "conv_3".to_owned(),
            vec![sample_message("m3", "user", "3")],
            None,
        );
        assert_eq!(cache.len(), 2);

        assert!(
            cache.get("conv_1").is_some(),
            "conv_1 was recently accessed, should remain"
        );
        assert!(
            cache.get("conv_2").is_none(),
            "conv_2 should have been evicted"
        );
        assert!(cache.get("conv_3").is_some(), "conv_3 should be present");
    }

    #[test]
    fn hot_cache_clear() {
        let cache = ConversationHotCache::new(4);
        cache.insert("c1".to_owned(), vec![], None);
        cache.insert("c2".to_owned(), vec![], None);
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }
}
