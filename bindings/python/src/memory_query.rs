use std::collections::HashMap;

use pyo3::prelude::*;

use memvid_core::types::memory_card::{MemoryCard, MemoryKind, Polarity, VersionRelation};

use crate::error;
use crate::lifecycle::{PyMemvid, guard_memvid};

// ---------------------------------------------------------------------------
// Helper conversions
// ---------------------------------------------------------------------------

fn kind_to_string(kind: MemoryKind) -> String {
    match kind {
        MemoryKind::Fact => "fact".to_string(),
        MemoryKind::Preference => "preference".to_string(),
        MemoryKind::Event => "event".to_string(),
        MemoryKind::Profile => "profile".to_string(),
        MemoryKind::Relationship => "relationship".to_string(),
        MemoryKind::Goal => "goal".to_string(),
        MemoryKind::Other => "other".to_string(),
    }
}

fn polarity_to_string(polarity: Polarity) -> String {
    match polarity {
        Polarity::Positive => "positive".to_string(),
        Polarity::Negative => "negative".to_string(),
        Polarity::Neutral => "neutral".to_string(),
    }
}

fn version_relation_to_string(vr: VersionRelation) -> String {
    match vr {
        VersionRelation::Sets => "sets".to_string(),
        VersionRelation::Updates => "updates".to_string(),
        VersionRelation::Extends => "extends".to_string(),
        VersionRelation::Retracts => "retracts".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Python wrapper types
// ---------------------------------------------------------------------------

/// A memory card returned from query operations.
#[pyclass(name = "MemoryCard", frozen)]
pub struct PyMemoryCard {
    #[pyo3(get)]
    pub id: u64,
    #[pyo3(get)]
    pub kind: String,
    #[pyo3(get)]
    pub entity: String,
    #[pyo3(get)]
    pub slot: String,
    #[pyo3(get)]
    pub value: String,
    #[pyo3(get)]
    pub polarity: Option<String>,
    #[pyo3(get)]
    pub event_date: Option<i64>,
    #[pyo3(get)]
    pub document_date: Option<i64>,
    #[pyo3(get)]
    pub version_key: Option<String>,
    #[pyo3(get)]
    pub version_relation: String,
    #[pyo3(get)]
    pub source_frame_id: u64,
    #[pyo3(get)]
    pub source_uri: Option<String>,
    #[pyo3(get)]
    pub engine: String,
    #[pyo3(get)]
    pub engine_version: String,
    #[pyo3(get)]
    pub confidence: Option<f32>,
    #[pyo3(get)]
    pub created_at: i64,
}

#[pymethods]
impl PyMemoryCard {
    fn __repr__(&self) -> String {
        format!(
            "MemoryCard(id={}, kind='{}', entity='{}', slot='{}', value='{}')",
            self.id, self.kind, self.entity, self.slot, self.value
        )
    }
}

impl From<&MemoryCard> for PyMemoryCard {
    fn from(c: &MemoryCard) -> Self {
        Self {
            id: c.id,
            kind: kind_to_string(c.kind),
            entity: c.entity.clone(),
            slot: c.slot.clone(),
            value: c.value.clone(),
            polarity: c.polarity.map(polarity_to_string),
            event_date: c.event_date,
            document_date: c.document_date,
            version_key: c.version_key.clone(),
            version_relation: version_relation_to_string(c.version_relation),
            source_frame_id: c.source_frame_id,
            source_uri: c.source_uri.clone(),
            engine: c.engine.clone(),
            engine_version: c.engine_version.clone(),
            confidence: c.confidence,
            created_at: c.created_at,
        }
    }
}

/// Statistics about the memories track.
#[pyclass(name = "MemoriesStats", frozen)]
pub struct PyMemoriesStats {
    #[pyo3(get)]
    pub card_count: usize,
    #[pyo3(get)]
    pub entity_count: usize,
    #[pyo3(get)]
    pub slot_count: usize,
    raw_cards_by_kind: HashMap<String, usize>,
    #[pyo3(get)]
    pub enriched_frames: usize,
    #[pyo3(get)]
    pub last_enrichment: Option<i64>,
}

#[pymethods]
impl PyMemoriesStats {
    #[getter]
    fn cards_by_kind(&self) -> HashMap<String, usize> {
        self.raw_cards_by_kind.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "MemoriesStats(card_count={}, entity_count={}, slot_count={})",
            self.card_count, self.entity_count, self.slot_count
        )
    }
}

// ---------------------------------------------------------------------------
// Query methods on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Get the current (most recent non-retracted) memory for an entity:slot pair.
    /// Returns None if no matching card exists.
    fn get_current_memory(
        &self,
        py: Python<'_>,
        entity: &str,
        slot: &str,
    ) -> PyResult<Option<PyMemoryCard>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv.get_current_memory(entity, slot).map(PyMemoryCard::from))
        })
    }

    /// Get all memory cards for an entity.
    fn get_entity_memories(&self, py: Python<'_>, entity: &str) -> PyResult<Vec<PyMemoryCard>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv
                .get_entity_memories(entity)
                .into_iter()
                .map(PyMemoryCard::from)
                .collect())
        })
    }

    /// Get event-type memory cards for an entity, sorted chronologically.
    fn get_memory_timeline(&self, py: Python<'_>, entity: &str) -> PyResult<Vec<PyMemoryCard>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv
                .get_memory_timeline(entity)
                .into_iter()
                .map(PyMemoryCard::from)
                .collect())
        })
    }

    /// Get all preference cards for an entity.
    fn get_preferences(&self, py: Python<'_>, entity: &str) -> PyResult<Vec<PyMemoryCard>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv
                .get_preferences(entity)
                .into_iter()
                .map(PyMemoryCard::from)
                .collect())
        })
    }

    /// Aggregate all unique values for an entity:slot pair across all occurrences.
    fn aggregate_memory_slot(
        &self,
        py: Python<'_>,
        entity: &str,
        slot: &str,
    ) -> PyResult<Vec<String>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv.aggregate_memory_slot(entity, slot))
        })
    }

    /// Get statistics about the memories track.
    fn memories_stats(&self, py: Python<'_>) -> PyResult<PyMemoriesStats> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            let stats = mv.memories_stats();
            Ok(PyMemoriesStats {
                card_count: stats.card_count,
                entity_count: stats.entity_count,
                slot_count: stats.slot_count,
                raw_cards_by_kind: stats.cards_by_kind,
                enriched_frames: stats.enriched_frames,
                last_enrichment: stats.last_enrichment,
            })
        })
    }
}

/// Register memory query classes on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMemoryCard>()?;
    m.add_class::<PyMemoriesStats>()?;
    Ok(())
}
