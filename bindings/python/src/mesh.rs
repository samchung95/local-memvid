use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use memvid_core::types::logic_mesh::{EntityKind, FollowResult, LinkType, MeshEdge, MeshNode};

use crate::error;
use crate::lifecycle::{PyMemvid, guard_memvid};

// ---------------------------------------------------------------------------
// Helper conversions (reuse EntityKind parsing from schema.rs pattern)
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

// ---------------------------------------------------------------------------
// Dict helpers
// ---------------------------------------------------------------------------

fn dict_get_str(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<String> {
    dict.get_item(key)?
        .ok_or_else(|| {
            pyo3::exceptions::PyKeyError::new_err(format!("missing required key '{}'", key))
        })?
        .extract::<String>()
}

fn dict_get_opt_f64(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<f64>> {
    match dict.get_item(key)? {
        Some(v) if !v.is_none() => Ok(Some(v.extract::<f64>()?)),
        _ => Ok(None),
    }
}

fn dict_get_opt_u64(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<u64>> {
    match dict.get_item(key)? {
        Some(v) if !v.is_none() => Ok(Some(v.extract::<u64>()?)),
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// PyMeshNode
// ---------------------------------------------------------------------------

/// A node in the entity-relationship graph.
#[pyclass(frozen, name = "MeshNode")]
pub struct PyMeshNode {
    #[pyo3(get)]
    pub id: u64,
    #[pyo3(get)]
    pub canonical_name: String,
    #[pyo3(get)]
    pub display_name: String,
    #[pyo3(get)]
    pub kind: String,
    #[pyo3(get)]
    pub confidence: f32,
    #[pyo3(get)]
    pub frame_ids: Vec<u64>,
}

impl From<&MeshNode> for PyMeshNode {
    fn from(node: &MeshNode) -> Self {
        Self {
            id: node.id,
            canonical_name: node.canonical_name.clone(),
            display_name: node.display_name.clone(),
            kind: entity_kind_to_string(node.kind),
            confidence: node.confidence_f32(),
            frame_ids: node.frame_ids.clone(),
        }
    }
}

#[pymethods]
impl PyMeshNode {
    fn __repr__(&self) -> String {
        format!(
            "MeshNode(id={}, canonical_name='{}', kind='{}', confidence={})",
            self.id, self.canonical_name, self.kind, self.confidence
        )
    }
}

// ---------------------------------------------------------------------------
// PyFollowResult
// ---------------------------------------------------------------------------

/// Result from following relationships in the graph.
#[pyclass(frozen, name = "FollowResult")]
pub struct PyFollowResult {
    #[pyo3(get)]
    pub node: String,
    #[pyo3(get)]
    pub kind: String,
    #[pyo3(get)]
    pub confidence: f32,
    #[pyo3(get)]
    pub frame_ids: Vec<u64>,
    #[pyo3(get)]
    pub path_length: usize,
}

impl From<FollowResult> for PyFollowResult {
    fn from(r: FollowResult) -> Self {
        Self {
            node: r.node,
            kind: entity_kind_to_string(r.kind),
            confidence: r.confidence,
            frame_ids: r.frame_ids,
            path_length: r.path_length,
        }
    }
}

#[pymethods]
impl PyFollowResult {
    fn __repr__(&self) -> String {
        format!(
            "FollowResult(node='{}', kind='{}', path_length={})",
            self.node, self.kind, self.path_length
        )
    }
}

// ---------------------------------------------------------------------------
// PyLogicMeshStats
// ---------------------------------------------------------------------------

/// Statistics about the logic mesh.
#[pyclass(frozen, name = "LogicMeshStats")]
pub struct PyLogicMeshStats {
    #[pyo3(get)]
    pub node_count: usize,
    #[pyo3(get)]
    pub edge_count: usize,
    raw_entity_kinds: HashMap<String, usize>,
    raw_link_types: HashMap<String, usize>,
}

#[pymethods]
impl PyLogicMeshStats {
    /// Count by entity kind.
    #[getter]
    fn entity_kinds(&self) -> HashMap<String, usize> {
        self.raw_entity_kinds.clone()
    }

    /// Count by link type.
    #[getter]
    fn link_types(&self) -> HashMap<String, usize> {
        self.raw_link_types.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "LogicMeshStats(node_count={}, edge_count={})",
            self.node_count, self.edge_count
        )
    }
}

// ---------------------------------------------------------------------------
// PyMemvid mesh methods
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Add a single node to the entity-relationship graph.
    #[pyo3(signature = (*, canonical_name, display_name, kind, confidence=0.9))]
    fn add_mesh_node(
        &self,
        py: Python<'_>,
        canonical_name: String,
        display_name: String,
        kind: &str,
        confidence: f64,
    ) -> PyResult<()> {
        error::catch_panic(py, || {
            let ek = parse_entity_kind(kind)?;
            let node = MeshNode::new(
                canonical_name,
                display_name,
                ek,
                confidence as f32,
                0, // no associated frame from Python API
                0,
                0,
            );
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.add_mesh_node(node);
            Ok(())
        })
    }

    /// Add multiple nodes from a list of dicts.
    ///
    /// Each dict must have keys: canonical_name, display_name, kind.
    /// Optional keys: confidence (default 0.9).
    fn add_mesh_nodes(&self, py: Python<'_>, nodes: Vec<Bound<'_, PyDict>>) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut rust_nodes = Vec::with_capacity(nodes.len());
            for (i, dict) in nodes.iter().enumerate() {
                let canonical_name = dict_get_str(dict, "canonical_name").map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!("node[{}]: {}", i, e))
                })?;
                let display_name = dict_get_str(dict, "display_name").map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!("node[{}]: {}", i, e))
                })?;
                let kind_str = dict_get_str(dict, "kind").map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!("node[{}]: {}", i, e))
                })?;
                let ek = parse_entity_kind(&kind_str).map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!("node[{}]: {}", i, e))
                })?;
                let confidence = dict_get_opt_f64(dict, "confidence")
                    .map_err(|e| {
                        pyo3::exceptions::PyValueError::new_err(format!("node[{}]: {}", i, e))
                    })?
                    .unwrap_or(0.9);
                rust_nodes.push(MeshNode::new(
                    canonical_name,
                    display_name,
                    ek,
                    confidence as f32,
                    0,
                    0,
                    0,
                ));
            }
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.add_mesh_nodes(rust_nodes);
            Ok(())
        })
    }

    /// Add a single edge to the entity-relationship graph.
    #[pyo3(signature = (*, from_node, to_node, link, confidence=0.9, frame_id=None))]
    fn add_mesh_edge(
        &self,
        py: Python<'_>,
        from_node: u64,
        to_node: u64,
        link: &str,
        confidence: f64,
        frame_id: Option<u64>,
    ) -> PyResult<()> {
        error::catch_panic(py, || {
            let lt = LinkType::from_str(link);
            let edge = MeshEdge::new(
                from_node,
                to_node,
                lt,
                confidence as f32,
                frame_id.unwrap_or(0),
            );
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.add_mesh_edge(edge);
            Ok(())
        })
    }

    /// Add multiple edges from a list of dicts.
    ///
    /// Each dict must have keys: from_node (int), to_node (int), link (str).
    /// Optional keys: confidence (default 0.9), frame_id.
    fn add_mesh_edges(&self, py: Python<'_>, edges: Vec<Bound<'_, PyDict>>) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut rust_edges = Vec::with_capacity(edges.len());
            for (i, dict) in edges.iter().enumerate() {
                let from_node = dict
                    .get_item("from_node")?
                    .ok_or_else(|| {
                        pyo3::exceptions::PyKeyError::new_err(format!(
                            "edge[{}]: missing required key 'from_node'",
                            i
                        ))
                    })?
                    .extract::<u64>()
                    .map_err(|e| {
                        pyo3::exceptions::PyValueError::new_err(format!("edge[{}]: {}", i, e))
                    })?;
                let to_node = dict
                    .get_item("to_node")?
                    .ok_or_else(|| {
                        pyo3::exceptions::PyKeyError::new_err(format!(
                            "edge[{}]: missing required key 'to_node'",
                            i
                        ))
                    })?
                    .extract::<u64>()
                    .map_err(|e| {
                        pyo3::exceptions::PyValueError::new_err(format!("edge[{}]: {}", i, e))
                    })?;
                let link_str = dict_get_str(dict, "link").map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!("edge[{}]: {}", i, e))
                })?;
                let lt = LinkType::from_str(&link_str);
                let confidence = dict_get_opt_f64(dict, "confidence")
                    .map_err(|e| {
                        pyo3::exceptions::PyValueError::new_err(format!("edge[{}]: {}", i, e))
                    })?
                    .unwrap_or(0.9);
                let frame_id = dict_get_opt_u64(dict, "frame_id")
                    .map_err(|e| {
                        pyo3::exceptions::PyValueError::new_err(format!("edge[{}]: {}", i, e))
                    })?
                    .unwrap_or(0);
                rust_edges.push(MeshEdge::new(
                    from_node,
                    to_node,
                    lt,
                    confidence as f32,
                    frame_id,
                ));
            }
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.add_mesh_edges(rust_edges);
            Ok(())
        })
    }

    /// Follow relationships from an entity in the graph.
    fn follow(
        &self,
        py: Python<'_>,
        start: &str,
        link: &str,
        hops: usize,
    ) -> PyResult<Vec<PyFollowResult>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            let results = mv.follow(start, link, hops);
            Ok(results.into_iter().map(PyFollowResult::from).collect())
        })
    }

    /// Find an entity node by name (case-insensitive).
    fn find_entity(&self, py: Python<'_>, name: &str) -> PyResult<Option<PyMeshNode>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv.find_entity(name).map(PyMeshNode::from))
        })
    }

    /// Get all entities mentioned in a specific frame.
    fn frame_entities(&self, py: Python<'_>, frame_id: u64) -> PyResult<Vec<PyMeshNode>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv
                .frame_entities(frame_id)
                .into_iter()
                .map(PyMeshNode::from)
                .collect())
        })
    }

    /// Get all entities of a specific kind.
    fn entities_by_kind(&self, py: Python<'_>, kind: &str) -> PyResult<Vec<PyMeshNode>> {
        error::catch_panic(py, || {
            let ek = parse_entity_kind(kind)?;
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv
                .entities_by_kind(ek)
                .into_iter()
                .map(PyMeshNode::from)
                .collect())
        })
    }

    /// Get statistics about the logic mesh.
    fn logic_mesh_stats(&self, py: Python<'_>) -> PyResult<PyLogicMeshStats> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            let stats = mv.logic_mesh_stats();
            Ok(PyLogicMeshStats {
                node_count: stats.node_count,
                edge_count: stats.edge_count,
                raw_entity_kinds: stats.entity_kinds,
                raw_link_types: stats.link_types,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMeshNode>()?;
    m.add_class::<PyFollowResult>()?;
    m.add_class::<PyLogicMeshStats>()?;
    Ok(())
}
