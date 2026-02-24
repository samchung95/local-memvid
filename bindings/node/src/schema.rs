use napi_derive::napi;

use memvid_core::{Cardinality, EntityKind, PredicateSchema, ValueType};

use crate::memory::{JsMemoryCard, to_memory_card};
use crate::memvid::{JsMemvid, guard_memvid, lock_inner};

// ---------------------------------------------------------------------------
// JsPredicateSchema
// ---------------------------------------------------------------------------

/// Schema definition for a predicate (memory card slot).
///
/// Defines validation rules including value type, cardinality, and domain
/// constraints for a given predicate/slot.
#[napi(object)]
pub struct JsPredicateSchema {
    /// Unique identifier for this predicate (matches the slot name).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Entity kinds that can have this predicate (empty = any).
    /// Values: "person", "organization", "project", "email", "date",
    /// "location", "product", "event", "money", "url", "other".
    pub domain: Vec<String>,
    /// Expected value type: "string", "number", "datetime", "boolean",
    /// "entity_ref", "enum", or "any".
    pub range: String,
    /// For "entity_ref" range: the kind of entity referenced.
    pub range_entity_kind: Option<String>,
    /// For "enum" range: the allowed values.
    pub range_enum_values: Option<Vec<String>>,
    /// "single" or "multiple".
    pub cardinality: String,
    /// Inverse predicate ID (e.g., "employer" <-> "employee").
    pub inverse: Option<String>,
    /// Whether this is a built-in schema.
    pub builtin: bool,
}

/// Validation result for a memory card against the schema.
#[napi(object)]
pub struct JsValidationResult {
    /// Whether the card is valid.
    pub valid: bool,
    /// Error message if invalid, null if valid.
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Converters
// ---------------------------------------------------------------------------

fn parse_entity_kind(s: &str) -> EntityKind {
    EntityKind::from_label(s)
}

fn value_type_to_string(vt: &ValueType) -> String {
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

fn from_predicate_schema(schema: &PredicateSchema) -> JsPredicateSchema {
    let range_entity_kind = match &schema.range {
        ValueType::EntityRef { kind } => Some(kind.as_str().to_string()),
        _ => None,
    };
    let range_enum_values = match &schema.range {
        ValueType::Enum { values } => Some(values.clone()),
        _ => None,
    };

    JsPredicateSchema {
        id: schema.id.clone(),
        name: schema.name.clone(),
        description: schema.description.clone(),
        domain: schema
            .domain
            .iter()
            .map(|k| k.as_str().to_string())
            .collect(),
        range: value_type_to_string(&schema.range),
        range_entity_kind,
        range_enum_values,
        cardinality: match schema.cardinality {
            Cardinality::Single => "single".to_string(),
            Cardinality::Multiple => "multiple".to_string(),
        },
        inverse: schema.inverse.clone(),
        builtin: schema.builtin,
    }
}

fn to_predicate_schema(js: &JsPredicateSchema) -> PredicateSchema {
    let domain: Vec<EntityKind> = js.domain.iter().map(|s| parse_entity_kind(s)).collect();

    let range = match js.range.as_str() {
        "number" => ValueType::Number,
        "datetime" => ValueType::DateTime,
        "boolean" => ValueType::Boolean,
        "entity_ref" => {
            let kind = js
                .range_entity_kind
                .as_deref()
                .map(parse_entity_kind)
                .unwrap_or(EntityKind::Other);
            ValueType::EntityRef { kind }
        }
        "enum" => {
            let values = js.range_enum_values.clone().unwrap_or_default();
            ValueType::Enum { values }
        }
        "any" => ValueType::Any,
        _ => ValueType::String,
    };

    let cardinality = match js.cardinality.as_str() {
        "multiple" => Cardinality::Multiple,
        _ => Cardinality::Single,
    };

    let mut schema = PredicateSchema::new(js.id.clone(), js.name.clone());
    schema.description = js.description.clone();
    schema.domain = domain;
    schema.range = range;
    schema.cardinality = cardinality;
    schema.inverse = js.inverse.clone();
    schema.builtin = js.builtin;
    schema
}

// ---------------------------------------------------------------------------
// JsMemvid impl — schema operations
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    // -- registerSchema ------------------------------------------------------

    /// Register a predicate schema (synchronous).
    #[napi(js_name = "registerSchemaSync")]
    pub fn register_schema_sync(&self, schema: JsPredicateSchema) -> napi::Result<()> {
        let rust_schema = to_predicate_schema(&schema);
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.register_schema(rust_schema);
        Ok(())
    }

    /// Register a predicate schema (async).
    #[napi(js_name = "registerSchema")]
    pub async fn register_schema(&self, schema: JsPredicateSchema) -> napi::Result<()> {
        let rust_schema = to_predicate_schema(&schema);
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.register_schema(rust_schema);
            Ok(())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- validateCard --------------------------------------------------------

    /// Validate a memory card against the schema (synchronous).
    /// Returns `{ valid: true }` or `{ valid: false, error: "..." }`.
    #[napi(js_name = "validateCardSync")]
    pub fn validate_card_sync(&self, card: JsMemoryCard) -> napi::Result<JsValidationResult> {
        let rust_card = to_memory_card(card)?;
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        match mv.validate_card(&rust_card) {
            Ok(()) => Ok(JsValidationResult {
                valid: true,
                error: None,
            }),
            Err(e) => Ok(JsValidationResult {
                valid: false,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Validate a memory card against the schema (async).
    /// Returns `{ valid: true }` or `{ valid: false, error: "..." }`.
    #[napi(js_name = "validateCard")]
    pub async fn validate_card(&self, card: JsMemoryCard) -> napi::Result<JsValidationResult> {
        let rust_card = to_memory_card(card)?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            match mv.validate_card(&rust_card) {
                Ok(()) => Ok(JsValidationResult {
                    valid: true,
                    error: None,
                }),
                Err(e) => Ok(JsValidationResult {
                    valid: false,
                    error: Some(e.to_string()),
                }),
            }
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- setSchemaStrict -----------------------------------------------------

    /// Enable or disable strict schema validation (synchronous).
    /// In strict mode, unknown predicates are rejected during validation.
    #[napi(js_name = "setSchemaStrictSync")]
    pub fn set_schema_strict_sync(&self, strict: bool) -> napi::Result<()> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.set_schema_strict(strict);
        Ok(())
    }

    // -- inferSchemas --------------------------------------------------------

    /// Infer schemas from existing memory cards (synchronous).
    /// Analyzes all predicates (slots) and infers type information
    /// and cardinality from actual values.
    #[napi(js_name = "inferSchemasSync")]
    pub fn infer_schemas_sync(&self) -> napi::Result<Vec<JsPredicateSchema>> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        let schemas = mv.infer_schemas();
        Ok(schemas.iter().map(from_predicate_schema).collect())
    }

    /// Infer schemas from existing memory cards (async).
    /// Analyzes all predicates (slots) and infers type information
    /// and cardinality from actual values.
    #[napi(js_name = "inferSchemas")]
    pub async fn infer_schemas(&self) -> napi::Result<Vec<JsPredicateSchema>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            let schemas = mv.infer_schemas();
            Ok(schemas.iter().map(from_predicate_schema).collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_predicate_schema_basic() {
        let schema = PredicateSchema::new("employer", "Employer");
        let js = from_predicate_schema(&schema);
        assert_eq!(js.id, "employer");
        assert_eq!(js.name, "Employer");
        assert!(js.description.is_none());
        assert!(js.domain.is_empty());
        assert_eq!(js.range, "string");
        assert!(js.range_entity_kind.is_none());
        assert!(js.range_enum_values.is_none());
        assert_eq!(js.cardinality, "single");
        assert!(js.inverse.is_none());
        assert!(!js.builtin);
    }

    #[test]
    fn from_predicate_schema_with_domain_and_range() {
        let schema = PredicateSchema::new("age", "Age")
            .with_domain(vec![EntityKind::Person])
            .with_range(ValueType::Number)
            .builtin();
        let js = from_predicate_schema(&schema);
        assert_eq!(js.domain, vec!["person"]);
        assert_eq!(js.range, "number");
        assert!(js.builtin);
    }

    #[test]
    fn from_predicate_schema_entity_ref() {
        let schema = PredicateSchema::new("spouse", "Spouse")
            .with_range(ValueType::EntityRef {
                kind: EntityKind::Person,
            })
            .with_inverse("spouse");
        let js = from_predicate_schema(&schema);
        assert_eq!(js.range, "entity_ref");
        assert_eq!(js.range_entity_kind.as_deref(), Some("person"));
        assert_eq!(js.inverse.as_deref(), Some("spouse"));
    }

    #[test]
    fn from_predicate_schema_enum() {
        let schema = PredicateSchema::new("status", "Status").with_range(ValueType::Enum {
            values: vec!["active".to_string(), "inactive".to_string()],
        });
        let js = from_predicate_schema(&schema);
        assert_eq!(js.range, "enum");
        assert_eq!(
            js.range_enum_values.as_deref(),
            Some(&["active".to_string(), "inactive".to_string()][..])
        );
    }

    #[test]
    fn to_predicate_schema_roundtrip() {
        let js = JsPredicateSchema {
            id: "hobby".to_string(),
            name: "Hobby".to_string(),
            description: Some("A hobby".to_string()),
            domain: vec!["person".to_string()],
            range: "string".to_string(),
            range_entity_kind: None,
            range_enum_values: None,
            cardinality: "multiple".to_string(),
            inverse: None,
            builtin: false,
        };
        let rust = to_predicate_schema(&js);
        assert_eq!(rust.id, "hobby");
        assert_eq!(rust.name, "Hobby");
        assert_eq!(rust.description.as_deref(), Some("A hobby"));
        assert_eq!(rust.domain, vec![EntityKind::Person]);
        assert_eq!(rust.range, ValueType::String);
        assert_eq!(rust.cardinality, Cardinality::Multiple);
    }

    #[test]
    fn closed_register_schema_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let schema = JsPredicateSchema {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            domain: vec![],
            range: "string".to_string(),
            range_entity_kind: None,
            range_enum_values: None,
            cardinality: "single".to_string(),
            inverse: None,
            builtin: false,
        };
        match mv.register_schema_sync(schema) {
            Err(e) => assert!(
                e.to_string().contains("CLOSED"),
                "Expected CLOSED, got: {e}"
            ),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }

    #[test]
    fn closed_validate_card_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let card = JsMemoryCard {
            kind: "fact".to_string(),
            entity: "user".to_string(),
            slot: "employer".to_string(),
            value: "Anthropic".to_string(),
            polarity: None,
            event_date: None,
        };
        match mv.validate_card_sync(card) {
            Err(e) => assert!(
                e.to_string().contains("CLOSED"),
                "Expected CLOSED, got: {e}"
            ),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }

    #[test]
    fn closed_set_schema_strict_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.set_schema_strict_sync(true) {
            Err(e) => assert!(
                e.to_string().contains("CLOSED"),
                "Expected CLOSED, got: {e}"
            ),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }

    #[test]
    fn closed_infer_schemas_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.infer_schemas_sync() {
            Err(e) => assert!(
                e.to_string().contains("CLOSED"),
                "Expected CLOSED, got: {e}"
            ),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }
}
