use pyo3::prelude::*;
use pyo3::types::PyDict;

use memvid_core::types::logic_mesh::EntityKind;
use memvid_core::types::schema::{Cardinality, PredicateSchema, ValueType};

use crate::error;
use crate::lifecycle::{guard_memvid, PyMemvid};
use crate::memory::build_card;

// ---------------------------------------------------------------------------
// Helper conversions
// ---------------------------------------------------------------------------

fn parse_entity_kind(s: &str) -> PyResult<EntityKind> {
    match s.to_lowercase().as_str() {
        "person" => Ok(EntityKind::Person),
        "organization" => Ok(EntityKind::Organization),
        "project" => Ok(EntityKind::Project),
        "email" => Ok(EntityKind::Email),
        "date" => Ok(EntityKind::Date),
        "location" => Ok(EntityKind::Location),
        "product" => Ok(EntityKind::Product),
        "event" => Ok(EntityKind::Event),
        "money" => Ok(EntityKind::Money),
        "url" => Ok(EntityKind::Url),
        "other" => Ok(EntityKind::Other),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "invalid entity kind '{}': expected one of person, organization, project, email, date, location, product, event, money, url, other",
            s
        ))),
    }
}

fn entity_kind_to_string(kind: EntityKind) -> String {
    match kind {
        EntityKind::Person => "person".to_string(),
        EntityKind::Organization => "organization".to_string(),
        EntityKind::Project => "project".to_string(),
        EntityKind::Email => "email".to_string(),
        EntityKind::Date => "date".to_string(),
        EntityKind::Location => "location".to_string(),
        EntityKind::Product => "product".to_string(),
        EntityKind::Event => "event".to_string(),
        EntityKind::Money => "money".to_string(),
        EntityKind::Url => "url".to_string(),
        EntityKind::Other => "other".to_string(),
    }
}

fn parse_value_type(
    range: &str,
    range_entity_kind: Option<&str>,
    range_enum_values: Option<Vec<String>>,
) -> PyResult<ValueType> {
    match range.to_lowercase().as_str() {
        "string" => Ok(ValueType::String),
        "number" => Ok(ValueType::Number),
        "datetime" => Ok(ValueType::DateTime),
        "boolean" => Ok(ValueType::Boolean),
        "entity_ref" => {
            let kind_str = range_entity_kind.ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(
                    "range_entity_kind is required when range='entity_ref'",
                )
            })?;
            let kind = parse_entity_kind(kind_str)?;
            Ok(ValueType::EntityRef { kind })
        }
        "enum" => {
            let values = range_enum_values.ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(
                    "range_enum_values is required when range='enum'",
                )
            })?;
            Ok(ValueType::Enum { values })
        }
        "any" => Ok(ValueType::Any),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "invalid range '{}': expected one of string, number, datetime, boolean, entity_ref, enum, any",
            range
        ))),
    }
}

fn parse_cardinality(s: &str) -> PyResult<Cardinality> {
    match s.to_lowercase().as_str() {
        "single" => Ok(Cardinality::Single),
        "multiple" => Ok(Cardinality::Multiple),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "invalid cardinality '{}': expected 'single' or 'multiple'",
            s
        ))),
    }
}

fn cardinality_to_string(c: Cardinality) -> String {
    match c {
        Cardinality::Single => "single".to_string(),
        Cardinality::Multiple => "multiple".to_string(),
    }
}

fn value_type_to_range_string(vt: &ValueType) -> String {
    match vt {
        ValueType::String => "string".to_string(),
        ValueType::Number => "number".to_string(),
        ValueType::DateTime => "datetime".to_string(),
        ValueType::Boolean => "boolean".to_string(),
        ValueType::EntityRef { .. } => "entity_ref".to_string(),
        ValueType::Enum { .. } => "enum".to_string(),
        ValueType::Any => "any".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Python wrapper types
// ---------------------------------------------------------------------------

/// A predicate schema describing validation rules for a memory card slot.
#[pyclass(name = "PredicateSchema", frozen)]
pub struct PyPredicateSchema {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub description: Option<String>,
    raw_domain: Vec<String>,
    #[pyo3(get)]
    pub range: String,
    #[pyo3(get)]
    pub range_entity_kind: Option<String>,
    raw_range_enum_values: Option<Vec<String>>,
    #[pyo3(get)]
    pub cardinality: String,
    #[pyo3(get)]
    pub inverse: Option<String>,
    #[pyo3(get)]
    pub builtin: bool,
}

#[pymethods]
impl PyPredicateSchema {
    #[getter]
    fn domain(&self) -> Vec<String> {
        self.raw_domain.clone()
    }

    #[getter]
    fn range_enum_values(&self) -> Option<Vec<String>> {
        self.raw_range_enum_values.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "PredicateSchema(id='{}', name='{}', range='{}', cardinality='{}')",
            self.id, self.name, self.range, self.cardinality
        )
    }
}

impl From<&PredicateSchema> for PyPredicateSchema {
    fn from(s: &PredicateSchema) -> Self {
        let (range_entity_kind, range_enum_values) = match &s.range {
            ValueType::EntityRef { kind } => (Some(entity_kind_to_string(*kind)), None),
            ValueType::Enum { values } => (None, Some(values.clone())),
            _ => (None, None),
        };
        Self {
            id: s.id.clone(),
            name: s.name.clone(),
            description: s.description.clone(),
            raw_domain: s.domain.iter().map(|k| entity_kind_to_string(*k)).collect(),
            range: value_type_to_range_string(&s.range),
            range_entity_kind,
            raw_range_enum_values: range_enum_values,
            cardinality: cardinality_to_string(s.cardinality),
            inverse: s.inverse.clone(),
            builtin: s.builtin,
        }
    }
}

/// Result of validating a memory card against the schema.
#[pyclass(name = "ValidationResult", frozen)]
pub struct PyValidationResult {
    #[pyo3(get)]
    pub valid: bool,
    #[pyo3(get)]
    pub error: Option<String>,
}

#[pymethods]
impl PyValidationResult {
    fn __repr__(&self) -> String {
        if self.valid {
            "ValidationResult(valid=True)".to_string()
        } else {
            format!(
                "ValidationResult(valid=False, error='{}')",
                self.error.as_deref().unwrap_or("")
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Schema methods on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Register a custom predicate schema.
    #[pyo3(signature = (*, name, description=None, domain=None, range="string", range_entity_kind=None, range_enum_values=None, cardinality="single", inverse=None, builtin=false))]
    #[allow(clippy::too_many_arguments)]
    fn register_schema(
        &self,
        py: Python<'_>,
        name: &str,
        description: Option<&str>,
        domain: Option<Vec<String>>,
        range: &str,
        range_entity_kind: Option<&str>,
        range_enum_values: Option<Vec<String>>,
        cardinality: &str,
        inverse: Option<&str>,
        builtin: bool,
    ) -> PyResult<()> {
        error::catch_panic(py, || {
            let vt = parse_value_type(range, range_entity_kind, range_enum_values)?;
            let card = parse_cardinality(cardinality)?;

            let mut schema = PredicateSchema::new(name, name);
            schema.description = description.map(String::from);
            schema.range = vt;
            schema.cardinality = card;
            schema.inverse = inverse.map(String::from);
            schema.builtin = builtin;

            if let Some(domain_strs) = domain {
                let mut kinds = Vec::with_capacity(domain_strs.len());
                for s in &domain_strs {
                    kinds.push(parse_entity_kind(s)?);
                }
                schema.domain = kinds;
            }

            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.register_schema(schema);
            Ok(())
        })
    }

    /// Validate a memory card dict against the schema.
    ///
    /// The card dict must have keys: kind, entity, slot, value.
    /// Optional keys: polarity, event_date.
    ///
    /// Returns a ValidationResult with valid=True if valid, or valid=False with an error message.
    fn validate_card(
        &self,
        py: Python<'_>,
        card: &Bound<'_, PyDict>,
    ) -> PyResult<PyValidationResult> {
        error::catch_panic(py, || {
            let kind = dict_get_str(card, "kind")?;
            let entity = dict_get_str(card, "entity")?;
            let slot = dict_get_str(card, "slot")?;
            let value = dict_get_str(card, "value")?;
            let polarity = dict_get_opt_str(card, "polarity")?;
            let event_date = dict_get_opt_i64(card, "event_date")?;

            let rust_card = build_card(
                &kind,
                &entity,
                &slot,
                &value,
                polarity.as_deref(),
                event_date,
            )?;

            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            match mv.validate_card(&rust_card) {
                Ok(()) => Ok(PyValidationResult {
                    valid: true,
                    error: None,
                }),
                Err(e) => Ok(PyValidationResult {
                    valid: false,
                    error: Some(e.to_string()),
                }),
            }
        })
    }

    /// Enable or disable strict schema validation.
    fn set_schema_strict(&self, py: Python<'_>, strict: bool) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.set_schema_strict(strict);
            Ok(())
        })
    }

    /// Infer schemas from existing memory cards.
    ///
    /// Analyzes all predicates in the memories track and infers type and
    /// cardinality from the actual values.
    fn infer_schemas(&self, py: Python<'_>) -> PyResult<Vec<PyPredicateSchema>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv
                .infer_schemas()
                .iter()
                .map(PyPredicateSchema::from)
                .collect())
        })
    }
}

// ---------------------------------------------------------------------------
// Dict helpers (local copies to avoid visibility issues)
// ---------------------------------------------------------------------------

fn dict_get_str(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<String> {
    dict.get_item(key)?
        .ok_or_else(|| {
            pyo3::exceptions::PyKeyError::new_err(format!("missing required key '{}'", key))
        })?
        .extract::<String>()
}

fn dict_get_opt_str(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    match dict.get_item(key)? {
        Some(v) if !v.is_none() => Ok(Some(v.extract::<String>()?)),
        _ => Ok(None),
    }
}

fn dict_get_opt_i64(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<i64>> {
    match dict.get_item(key)? {
        Some(v) if !v.is_none() => Ok(Some(v.extract::<i64>()?)),
        _ => Ok(None),
    }
}

/// Register schema classes on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPredicateSchema>()?;
    m.add_class::<PyValidationResult>()?;
    Ok(())
}
