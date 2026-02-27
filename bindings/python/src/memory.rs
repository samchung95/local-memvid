use pyo3::prelude::*;
use pyo3::types::PyDict;

use memvid_core::types::memory_card::{MemoryCardBuilder, MemoryKind, Polarity};

use crate::error;
use crate::lifecycle::{guard_memvid, PyMemvid};

/// Parse a Python string into a `MemoryKind`.
fn parse_kind(kind: &str) -> PyResult<MemoryKind> {
    match kind.to_lowercase().as_str() {
        "fact" => Ok(MemoryKind::Fact),
        "preference" => Ok(MemoryKind::Preference),
        "event" => Ok(MemoryKind::Event),
        "profile" => Ok(MemoryKind::Profile),
        "relationship" => Ok(MemoryKind::Relationship),
        "goal" => Ok(MemoryKind::Goal),
        "other" => Ok(MemoryKind::Other),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "invalid memory kind '{}': expected one of fact, preference, event, profile, relationship, goal, other",
            kind
        ))),
    }
}

/// Parse an optional Python string into a `Polarity`.
fn parse_polarity(polarity: &str) -> PyResult<Polarity> {
    Polarity::from_str(polarity).ok_or_else(|| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "invalid polarity '{}': expected one of positive, negative, neutral",
            polarity
        ))
    })
}

/// Build a `MemoryCard` from the given Python kwargs.
pub(crate) fn build_card(
    kind: &str,
    entity: &str,
    slot: &str,
    value: &str,
    polarity: Option<&str>,
    event_date: Option<i64>,
) -> PyResult<memvid_core::types::MemoryCard> {
    let mk = parse_kind(kind)?;
    let mut builder = MemoryCardBuilder::new()
        .kind(mk)
        .entity(entity)
        .slot(slot)
        .value(value)
        .source(0, None)
        .engine("python-api", "1.0.0");

    if let Some(p) = polarity {
        builder = builder.polarity(parse_polarity(p)?);
    }
    if let Some(ed) = event_date {
        builder = builder.event_date(ed);
    }

    builder.build(0).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!("failed to build memory card: {}", e))
    })
}

/// Extract a required string field from a Python dict.
fn dict_get_str<'py>(dict: &Bound<'py, PyDict>, key: &str) -> PyResult<String> {
    dict.get_item(key)?
        .ok_or_else(|| {
            pyo3::exceptions::PyKeyError::new_err(format!("missing required key '{}'", key))
        })?
        .extract::<String>()
}

/// Extract an optional string field from a Python dict.
fn dict_get_opt_str(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    match dict.get_item(key)? {
        Some(v) if !v.is_none() => Ok(Some(v.extract::<String>()?)),
        _ => Ok(None),
    }
}

/// Extract an optional i64 field from a Python dict.
fn dict_get_opt_i64(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<i64>> {
    match dict.get_item(key)? {
        Some(v) if !v.is_none() => Ok(Some(v.extract::<i64>()?)),
        _ => Ok(None),
    }
}

#[pymethods]
impl PyMemvid {
    /// Insert a single memory card and return its ID.
    ///
    /// `kind` must be one of: fact, preference, event, profile, relationship, goal, other.
    /// `polarity` (optional) must be one of: positive, negative, neutral.
    #[pyo3(signature = (*, kind, entity, slot, value, polarity=None, event_date=None))]
    fn put_memory_card(
        &self,
        py: Python<'_>,
        kind: &str,
        entity: &str,
        slot: &str,
        value: &str,
        polarity: Option<&str>,
        event_date: Option<i64>,
    ) -> PyResult<u64> {
        error::catch_panic(py, || {
            let card = build_card(kind, entity, slot, value, polarity, event_date)?;
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.put_memory_card(card)
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Insert multiple memory cards from a list of dicts and return their IDs.
    ///
    /// Each dict must have keys: kind, entity, slot, value.
    /// Optional keys: polarity, event_date.
    fn put_memory_cards(
        &self,
        py: Python<'_>,
        cards: Vec<Bound<'_, PyDict>>,
    ) -> PyResult<Vec<u64>> {
        error::catch_panic(py, || {
            let mut rust_cards = Vec::with_capacity(cards.len());
            for (i, dict) in cards.iter().enumerate() {
                let kind = dict_get_str(dict, "kind")
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(
                        format!("card[{}]: {}", i, e),
                    ))?;
                let entity = dict_get_str(dict, "entity")
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(
                        format!("card[{}]: {}", i, e),
                    ))?;
                let slot = dict_get_str(dict, "slot")
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(
                        format!("card[{}]: {}", i, e),
                    ))?;
                let value = dict_get_str(dict, "value")
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(
                        format!("card[{}]: {}", i, e),
                    ))?;
                let polarity = dict_get_opt_str(dict, "polarity")
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(
                        format!("card[{}]: {}", i, e),
                    ))?;
                let event_date = dict_get_opt_i64(dict, "event_date")
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(
                        format!("card[{}]: {}", i, e),
                    ))?;

                let card = build_card(
                    &kind,
                    &entity,
                    &slot,
                    &value,
                    polarity.as_deref(),
                    event_date,
                )?;
                rust_cards.push(card);
            }

            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.put_memory_cards(rust_cards)
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Get the total number of memory cards.
    #[getter]
    fn memory_card_count(&self, py: Python<'_>) -> PyResult<usize> {
        let lock = guard_memvid!(self, py);
        Ok(lock.as_ref().unwrap().memory_card_count())
    }

    /// Clear all memory cards and enrichment records.
    fn clear_memories(&self, py: Python<'_>) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.clear_memories();
            Ok(())
        })
    }
}
