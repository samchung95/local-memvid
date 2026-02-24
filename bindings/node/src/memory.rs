use napi_derive::napi;

use std::collections::HashMap;

use memvid_core::{MemoryCard, MemoryCardBuilder, MemoryKind, Polarity};

use crate::error::from_memvid_error;
use crate::memvid::{guard_memvid, lock_inner, JsMemvid};

// ---------------------------------------------------------------------------
// JsMemoryCard
// ---------------------------------------------------------------------------

/// A structured memory card representing an atomic fact, preference, event,
/// or other information extracted from content.
///
/// Required fields: `kind`, `entity`, `slot`, `value`.
/// All other fields are optional and default to sensible values.
#[napi(object)]
pub struct JsMemoryCard {
    /// Memory kind: "fact", "preference", "event", "profile", "relationship",
    /// "goal", or "other".
    pub kind: String,
    /// The entity this memory is about (e.g. "user", "project.memvid").
    pub entity: String,
    /// The attribute/slot (e.g. "employer", "favorite_food").
    pub slot: String,
    /// The actual value (stored as string; use JSON for complex values).
    pub value: String,
    /// Sentiment: "positive", "negative", or "neutral".
    pub polarity: Option<String>,
    /// When the event/fact occurred (Unix timestamp), not when it was recorded.
    pub event_date: Option<i64>,
}

/// Convert a `JsMemoryCard` into a Rust `MemoryCard` via the builder.
pub(crate) fn to_memory_card(js: JsMemoryCard) -> napi::Result<MemoryCard> {
    let kind = MemoryKind::from_str(&js.kind);

    let mut builder = MemoryCardBuilder::new()
        .kind(kind)
        .entity(js.entity)
        .slot(js.slot)
        .value(js.value)
        // source_frame_id and engine are required by the builder but not
        // exposed in the JS API — use placeholder values. The Rust
        // `put_memory_card` will assign a proper ID.
        .source(0, None)
        .engine("node-api", "1.0.0");

    if let Some(p) = js.polarity {
        if let Some(pol) = Polarity::from_str(&p) {
            builder = builder.polarity(pol);
        }
    }

    if let Some(d) = js.event_date {
        builder = builder.event_date(d);
    }

    builder.build(0).map_err(|e| {
        napi::Error::new(
            napi::Status::GenericFailure,
            format!("[INTERNAL] Failed to build MemoryCard: {e}"),
        )
    })
}

/// Convert a Rust `MemoryCard` reference into a `JsMemoryCard`.
pub(crate) fn from_memory_card(card: &MemoryCard) -> JsMemoryCard {
    JsMemoryCard {
        kind: card.kind.as_str().to_string(),
        entity: card.entity.clone(),
        slot: card.slot.clone(),
        value: card.value.clone(),
        polarity: card.polarity.map(|p| p.as_str().to_string()),
        event_date: card.event_date,
    }
}

// ---------------------------------------------------------------------------
// JsMemvid impl — memory card CRUD
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    // -- putMemoryCard -------------------------------------------------------

    /// Insert a single memory card (synchronous). Returns the assigned card ID.
    #[napi(js_name = "putMemoryCardSync")]
    pub fn put_memory_card_sync(&self, card: JsMemoryCard) -> napi::Result<i64> {
        let rust_card = to_memory_card(card)?;
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let id = mv.put_memory_card(rust_card).map_err(from_memvid_error)?;
        Ok(id as i64)
    }

    /// Insert a single memory card (async). Returns the assigned card ID.
    #[napi(js_name = "putMemoryCard")]
    pub async fn put_memory_card(&self, card: JsMemoryCard) -> napi::Result<i64> {
        let rust_card = to_memory_card(card)?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let id = mv.put_memory_card(rust_card).map_err(from_memvid_error)?;
            Ok(id as i64)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- putMemoryCards ------------------------------------------------------

    /// Insert multiple memory cards (synchronous). Returns the assigned card IDs.
    #[napi(js_name = "putMemoryCardsSync")]
    pub fn put_memory_cards_sync(&self, cards: Vec<JsMemoryCard>) -> napi::Result<Vec<i64>> {
        let rust_cards: Vec<MemoryCard> = cards
            .into_iter()
            .map(to_memory_card)
            .collect::<napi::Result<Vec<_>>>()?;
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let ids = mv
            .put_memory_cards(rust_cards)
            .map_err(from_memvid_error)?;
        Ok(ids.into_iter().map(|id| id as i64).collect())
    }

    /// Insert multiple memory cards (async). Returns the assigned card IDs.
    #[napi(js_name = "putMemoryCards")]
    pub async fn put_memory_cards(&self, cards: Vec<JsMemoryCard>) -> napi::Result<Vec<i64>> {
        let rust_cards: Vec<MemoryCard> = cards
            .into_iter()
            .map(to_memory_card)
            .collect::<napi::Result<Vec<_>>>()?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let ids = mv
                .put_memory_cards(rust_cards)
                .map_err(from_memvid_error)?;
            Ok(ids.into_iter().map(|id| id as i64).collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- memoryCardCount -----------------------------------------------------

    /// Total number of memory cards stored.
    #[napi(getter, js_name = "memoryCardCount")]
    pub fn memory_card_count(&self) -> napi::Result<u32> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(mv.memory_card_count() as u32)
    }

    // -- clearMemories -------------------------------------------------------

    /// Wipe all memory cards and enrichment records (synchronous).
    #[napi(js_name = "clearMemoriesSync")]
    pub fn clear_memories_sync(&self) -> napi::Result<()> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.clear_memories();
        Ok(())
    }

    /// Wipe all memory cards and enrichment records (async).
    #[napi(js_name = "clearMemories")]
    pub async fn clear_memories(&self) -> napi::Result<()> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.clear_memories();
            Ok(())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }
}

// ---------------------------------------------------------------------------
// JsMemoriesStats
// ---------------------------------------------------------------------------

/// Statistics about the memories track.
#[napi(object)]
pub struct JsMemoriesStats {
    /// Total number of cards.
    pub card_count: u32,
    /// Number of unique entities.
    pub entity_count: u32,
    /// Number of unique (entity, slot) pairs.
    pub slot_count: u32,
    /// Cards grouped by kind (e.g. `{ "fact": 3, "preference": 2 }`).
    pub cards_by_kind: HashMap<String, u32>,
    /// Number of enriched frames.
    pub enriched_frames: u32,
    /// Last enrichment timestamp (Unix seconds), or null if never enriched.
    pub last_enrichment: Option<i64>,
}

// ---------------------------------------------------------------------------
// JsMemvid impl — memory card query methods
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    // -- getCurrentMemory ----------------------------------------------------

    /// Get the current (most recent, non-retracted) memory for an entity:slot
    /// (synchronous). Returns null if not found.
    #[napi(js_name = "getCurrentMemorySync")]
    pub fn get_current_memory_sync(
        &self,
        entity: String,
        slot: String,
    ) -> napi::Result<Option<JsMemoryCard>> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(mv.get_current_memory(&entity, &slot).map(from_memory_card))
    }

    /// Get the current (most recent, non-retracted) memory for an entity:slot
    /// (async). Returns null if not found.
    #[napi(js_name = "getCurrentMemory")]
    pub async fn get_current_memory(
        &self,
        entity: String,
        slot: String,
    ) -> napi::Result<Option<JsMemoryCard>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            Ok(mv.get_current_memory(&entity, &slot).map(from_memory_card))
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- getEntityMemories ---------------------------------------------------

    /// Get all memory cards for an entity (synchronous).
    #[napi(js_name = "getEntityMemoriesSync")]
    pub fn get_entity_memories_sync(&self, entity: String) -> napi::Result<Vec<JsMemoryCard>> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(mv
            .get_entity_memories(&entity)
            .into_iter()
            .map(from_memory_card)
            .collect())
    }

    /// Get all memory cards for an entity (async).
    #[napi(js_name = "getEntityMemories")]
    pub async fn get_entity_memories(&self, entity: String) -> napi::Result<Vec<JsMemoryCard>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            Ok(mv
                .get_entity_memories(&entity)
                .into_iter()
                .map(from_memory_card)
                .collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- getMemoryTimeline ---------------------------------------------------

    /// Get the event timeline for an entity, sorted chronologically
    /// (synchronous).
    #[napi(js_name = "getMemoryTimelineSync")]
    pub fn get_memory_timeline_sync(&self, entity: String) -> napi::Result<Vec<JsMemoryCard>> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(mv
            .get_memory_timeline(&entity)
            .into_iter()
            .map(from_memory_card)
            .collect())
    }

    /// Get the event timeline for an entity, sorted chronologically (async).
    #[napi(js_name = "getMemoryTimeline")]
    pub async fn get_memory_timeline(&self, entity: String) -> napi::Result<Vec<JsMemoryCard>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            Ok(mv
                .get_memory_timeline(&entity)
                .into_iter()
                .map(from_memory_card)
                .collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- getPreferences ------------------------------------------------------

    /// Get all preference-type cards for an entity (synchronous).
    #[napi(js_name = "getPreferencesSync")]
    pub fn get_preferences_sync(&self, entity: String) -> napi::Result<Vec<JsMemoryCard>> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(mv
            .get_preferences(&entity)
            .into_iter()
            .map(from_memory_card)
            .collect())
    }

    /// Get all preference-type cards for an entity (async).
    #[napi(js_name = "getPreferences")]
    pub async fn get_preferences(&self, entity: String) -> napi::Result<Vec<JsMemoryCard>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            Ok(mv
                .get_preferences(&entity)
                .into_iter()
                .map(from_memory_card)
                .collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- aggregateMemorySlot -------------------------------------------------

    /// Aggregate all unique values for a slot across all occurrences
    /// (synchronous).
    #[napi(js_name = "aggregateMemorySlotSync")]
    pub fn aggregate_memory_slot_sync(
        &self,
        entity: String,
        slot: String,
    ) -> napi::Result<Vec<String>> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(mv.aggregate_memory_slot(&entity, &slot))
    }

    /// Aggregate all unique values for a slot across all occurrences (async).
    #[napi(js_name = "aggregateMemorySlot")]
    pub async fn aggregate_memory_slot(
        &self,
        entity: String,
        slot: String,
    ) -> napi::Result<Vec<String>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            Ok(mv.aggregate_memory_slot(&entity, &slot))
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- memoriesStats -------------------------------------------------------

    /// Get statistics about the memories track.
    #[napi(js_name = "memoriesStats")]
    pub fn memories_stats(&self) -> napi::Result<JsMemoriesStats> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        let stats = mv.memories_stats();
        Ok(JsMemoriesStats {
            card_count: stats.card_count as u32,
            entity_count: stats.entity_count as u32,
            slot_count: stats.slot_count as u32,
            cards_by_kind: stats
                .cards_by_kind
                .into_iter()
                .map(|(k, v)| (k, v as u32))
                .collect(),
            enriched_frames: stats.enriched_frames as u32,
            last_enrichment: stats.last_enrichment,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_memory_card_fact() {
        let js = JsMemoryCard {
            kind: "fact".to_string(),
            entity: "user".to_string(),
            slot: "employer".to_string(),
            value: "Anthropic".to_string(),
            polarity: None,
            event_date: None,
        };
        let card = to_memory_card(js).unwrap();
        assert_eq!(card.kind, MemoryKind::Fact);
        assert_eq!(card.entity, "user");
        assert_eq!(card.slot, "employer");
        assert_eq!(card.value, "Anthropic");
    }

    #[test]
    fn to_memory_card_preference_with_polarity() {
        let js = JsMemoryCard {
            kind: "preference".to_string(),
            entity: "user".to_string(),
            slot: "beverage".to_string(),
            value: "coffee".to_string(),
            polarity: Some("positive".to_string()),
            event_date: None,
        };
        let card = to_memory_card(js).unwrap();
        assert_eq!(card.kind, MemoryKind::Preference);
        assert_eq!(card.polarity, Some(Polarity::Positive));
    }

    #[test]
    fn to_memory_card_event_with_date() {
        let js = JsMemoryCard {
            kind: "event".to_string(),
            entity: "user".to_string(),
            slot: "life_event".to_string(),
            value: "moved to SF".to_string(),
            polarity: None,
            event_date: Some(1700000000),
        };
        let card = to_memory_card(js).unwrap();
        assert_eq!(card.kind, MemoryKind::Event);
        assert_eq!(card.event_date, Some(1700000000));
    }

    #[test]
    fn from_memory_card_roundtrip() {
        let rust_card = MemoryCardBuilder::new()
            .fact()
            .entity("user")
            .slot("employer")
            .value("Anthropic")
            .source(0, None)
            .engine("test", "1.0.0")
            .build(42)
            .unwrap();

        let js = from_memory_card(&rust_card);
        assert_eq!(js.kind, "fact");
        assert_eq!(js.entity, "user");
        assert_eq!(js.slot, "employer");
        assert_eq!(js.value, "Anthropic");
        assert!(js.polarity.is_none());
        assert!(js.event_date.is_none());
    }

    #[test]
    fn all_memory_kinds_parsed() {
        for (s, expected) in [
            ("fact", MemoryKind::Fact),
            ("preference", MemoryKind::Preference),
            ("event", MemoryKind::Event),
            ("profile", MemoryKind::Profile),
            ("relationship", MemoryKind::Relationship),
            ("goal", MemoryKind::Goal),
            ("other", MemoryKind::Other),
            ("unknown_kind", MemoryKind::Other),
        ] {
            let js = JsMemoryCard {
                kind: s.to_string(),
                entity: "e".to_string(),
                slot: "s".to_string(),
                value: "v".to_string(),
                polarity: None,
                event_date: None,
            };
            let card = to_memory_card(js).unwrap();
            assert_eq!(card.kind, expected, "Mismatch for kind string '{s}'");
        }
    }

    #[test]
    fn closed_instance_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let js_card = JsMemoryCard {
            kind: "fact".to_string(),
            entity: "user".to_string(),
            slot: "test".to_string(),
            value: "val".to_string(),
            polarity: None,
            event_date: None,
        };
        let result = mv.put_memory_card_sync(js_card);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("CLOSED"), "Expected CLOSED error, got: {err_msg}");
    }

    // -- US-012 query method tests -------------------------------------------

    #[test]
    fn closed_get_current_memory_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.get_current_memory_sync("user".to_string(), "slot".to_string()) {
            Err(e) => assert!(e.to_string().contains("CLOSED"), "Expected CLOSED, got: {e}"),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }

    #[test]
    fn closed_get_entity_memories_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.get_entity_memories_sync("user".to_string()) {
            Err(e) => assert!(e.to_string().contains("CLOSED"), "Expected CLOSED, got: {e}"),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }

    #[test]
    fn closed_get_memory_timeline_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.get_memory_timeline_sync("user".to_string()) {
            Err(e) => assert!(e.to_string().contains("CLOSED"), "Expected CLOSED, got: {e}"),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }

    #[test]
    fn closed_get_preferences_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.get_preferences_sync("user".to_string()) {
            Err(e) => assert!(e.to_string().contains("CLOSED"), "Expected CLOSED, got: {e}"),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }

    #[test]
    fn closed_aggregate_memory_slot_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.aggregate_memory_slot_sync("user".to_string(), "slot".to_string()) {
            Err(e) => assert!(e.to_string().contains("CLOSED"), "Expected CLOSED, got: {e}"),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }

    #[test]
    fn closed_memories_stats_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.memories_stats() {
            Err(e) => assert!(e.to_string().contains("CLOSED"), "Expected CLOSED, got: {e}"),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }
}
