use napi_derive::napi;

use std::collections::HashMap;

use memvid_core::{EntityKind, FollowResult, LinkType, MeshEdge, MeshNode};

use crate::memvid::{guard_memvid, lock_inner, JsMemvid};

// ---------------------------------------------------------------------------
// JsMeshNode
// ---------------------------------------------------------------------------

/// An entity node in the Logic-Mesh graph.
///
/// Required fields: `canonicalName`, `displayName`, `kind`.
/// Optional fields default to sensible values.
#[napi(object)]
pub struct JsMeshNode {
    /// Canonical entity name (lowercased, normalized).
    pub canonical_name: String,
    /// Display name (original casing).
    pub display_name: String,
    /// Entity kind: "person", "organization", "project", "email", "date",
    /// "location", "product", "event", "money", "url", or "other".
    pub kind: String,
    /// Confidence score from NER (0.0–1.0).
    pub confidence: Option<f64>,
    /// Frame IDs where this entity appears.
    pub frame_ids: Option<Vec<i64>>,
}

/// Convert a `JsMeshNode` into a Rust `MeshNode`.
fn to_mesh_node(js: &JsMeshNode) -> MeshNode {
    let kind = EntityKind::from_label(&js.kind);
    let confidence = js.confidence.unwrap_or(1.0) as f32;
    let frame_ids: Vec<u64> = js
        .frame_ids
        .as_ref()
        .map(|ids| ids.iter().map(|&id| id as u64).collect())
        .unwrap_or_default();
    let first_frame = frame_ids.first().copied().unwrap_or(0);

    let mut node = MeshNode::new(
        js.canonical_name.clone(),
        js.display_name.clone(),
        kind,
        confidence,
        first_frame,
        0, // byte_start
        0, // byte_len
    );

    // Add remaining frame_ids as additional mentions.
    for &fid in frame_ids.iter().skip(1) {
        if !node.frame_ids.contains(&fid) {
            node.frame_ids.push(fid);
            node.mentions.push((fid, 0, 0));
        }
    }

    node
}

/// Convert a Rust `MeshNode` reference into a `JsMeshNode`.
fn from_mesh_node(node: &MeshNode) -> JsMeshNode {
    JsMeshNode {
        canonical_name: node.canonical_name.clone(),
        display_name: node.display_name.clone(),
        kind: node.kind.as_str().to_string(),
        confidence: Some(node.confidence_f32() as f64),
        frame_ids: Some(node.frame_ids.iter().map(|&id| id as i64).collect()),
    }
}

// ---------------------------------------------------------------------------
// JsMeshEdge
// ---------------------------------------------------------------------------

/// A relationship edge in the Logic-Mesh graph.
///
/// Required fields: `fromNode`, `toNode`, `link`.
#[napi(object)]
pub struct JsMeshEdge {
    /// Source node ID (use the node's computed ID, or 0 if unknown).
    pub from_node: i64,
    /// Target node ID.
    pub to_node: i64,
    /// Relationship type: "manager", "member", "owner", "author", "email",
    /// "deadline", "location", "employer", "parent", "child", "related",
    /// or any custom string.
    pub link: String,
    /// Confidence score (0.0–1.0).
    pub confidence: Option<f64>,
    /// Frame ID where relationship was detected.
    pub frame_id: Option<i64>,
}

/// Convert a `JsMeshEdge` into a Rust `MeshEdge`.
fn to_mesh_edge(js: &JsMeshEdge) -> MeshEdge {
    let link = LinkType::from_str(&js.link);
    let confidence = js.confidence.unwrap_or(1.0) as f32;
    let frame_id = js.frame_id.unwrap_or(0) as u64;

    MeshEdge::new(js.from_node as u64, js.to_node as u64, link, confidence, frame_id)
}

/// Convert a Rust `MeshEdge` reference into a `JsMeshEdge`.
#[allow(dead_code)]
fn from_mesh_edge(edge: &MeshEdge) -> JsMeshEdge {
    JsMeshEdge {
        from_node: edge.from_node as i64,
        to_node: edge.to_node as i64,
        link: edge.link.as_str().to_string(),
        confidence: Some(edge.confidence_f32() as f64),
        frame_id: Some(edge.frame_id as i64),
    }
}

// ---------------------------------------------------------------------------
// JsFollowResult
// ---------------------------------------------------------------------------

/// Result from following relationships in the Logic-Mesh graph.
#[napi(object)]
pub struct JsFollowResult {
    /// Entity name found.
    pub node: String,
    /// Entity kind.
    pub kind: String,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Frame IDs where this entity appears.
    pub frame_ids: Vec<i64>,
    /// Path length from start node.
    pub path_length: u32,
}

/// Convert a Rust `FollowResult` into a `JsFollowResult`.
fn from_follow_result(r: &FollowResult) -> JsFollowResult {
    JsFollowResult {
        node: r.node.clone(),
        kind: r.kind.as_str().to_string(),
        confidence: r.confidence as f64,
        frame_ids: r.frame_ids.iter().map(|&id| id as i64).collect(),
        path_length: r.path_length as u32,
    }
}

// ---------------------------------------------------------------------------
// JsLogicMeshStats
// ---------------------------------------------------------------------------

/// Statistics about the Logic-Mesh.
#[napi(object)]
pub struct JsLogicMeshStats {
    /// Total node count.
    pub node_count: u32,
    /// Total edge count.
    pub edge_count: u32,
    /// Count by entity kind (e.g. `{ "person": 3, "organization": 1 }`).
    pub entity_kinds: HashMap<String, u32>,
    /// Count by link type (e.g. `{ "manager": 2, "employer": 1 }`).
    pub link_types: HashMap<String, u32>,
}

// ---------------------------------------------------------------------------
// JsMemvid impl — logic mesh operations
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    // -- addMeshNode ---------------------------------------------------------

    /// Add a single mesh node (entity) to the Logic-Mesh (synchronous).
    /// The node is merged with existing nodes by canonical name and kind.
    #[napi(js_name = "addMeshNodeSync")]
    pub fn add_mesh_node_sync(&self, node: JsMeshNode) -> napi::Result<()> {
        let rust_node = to_mesh_node(&node);
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.add_mesh_node(rust_node);
        Ok(())
    }

    /// Add a single mesh node (entity) to the Logic-Mesh (async).
    #[napi(js_name = "addMeshNode")]
    pub async fn add_mesh_node(&self, node: JsMeshNode) -> napi::Result<()> {
        let rust_node = to_mesh_node(&node);
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.add_mesh_node(rust_node);
            Ok(())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- addMeshNodes --------------------------------------------------------

    /// Add multiple mesh nodes at once (synchronous).
    #[napi(js_name = "addMeshNodesSync")]
    pub fn add_mesh_nodes_sync(&self, nodes: Vec<JsMeshNode>) -> napi::Result<()> {
        let rust_nodes: Vec<MeshNode> = nodes.iter().map(to_mesh_node).collect();
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.add_mesh_nodes(rust_nodes);
        Ok(())
    }

    /// Add multiple mesh nodes at once (async).
    #[napi(js_name = "addMeshNodes")]
    pub async fn add_mesh_nodes(&self, nodes: Vec<JsMeshNode>) -> napi::Result<()> {
        let rust_nodes: Vec<MeshNode> = nodes.iter().map(to_mesh_node).collect();
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.add_mesh_nodes(rust_nodes);
            Ok(())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- addMeshEdge ---------------------------------------------------------

    /// Add a single mesh edge (relationship) to the Logic-Mesh (synchronous).
    /// The edge is deduplicated by (from, to, link_type).
    #[napi(js_name = "addMeshEdgeSync")]
    pub fn add_mesh_edge_sync(&self, edge: JsMeshEdge) -> napi::Result<()> {
        let rust_edge = to_mesh_edge(&edge);
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.add_mesh_edge(rust_edge);
        Ok(())
    }

    /// Add a single mesh edge (relationship) to the Logic-Mesh (async).
    #[napi(js_name = "addMeshEdge")]
    pub async fn add_mesh_edge(&self, edge: JsMeshEdge) -> napi::Result<()> {
        let rust_edge = to_mesh_edge(&edge);
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.add_mesh_edge(rust_edge);
            Ok(())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- addMeshEdges --------------------------------------------------------

    /// Add multiple mesh edges at once (synchronous).
    #[napi(js_name = "addMeshEdgesSync")]
    pub fn add_mesh_edges_sync(&self, edges: Vec<JsMeshEdge>) -> napi::Result<()> {
        let rust_edges: Vec<MeshEdge> = edges.iter().map(to_mesh_edge).collect();
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.add_mesh_edges(rust_edges);
        Ok(())
    }

    /// Add multiple mesh edges at once (async).
    #[napi(js_name = "addMeshEdges")]
    pub async fn add_mesh_edges(&self, edges: Vec<JsMeshEdge>) -> napi::Result<()> {
        let rust_edges: Vec<MeshEdge> = edges.iter().map(to_mesh_edge).collect();
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.add_mesh_edges(rust_edges);
            Ok(())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- follow --------------------------------------------------------------

    /// Follow relationships from an entity in the graph (synchronous).
    /// Traverses the Logic-Mesh starting from the named entity,
    /// following edges of the specified type up to `hops` hops.
    #[napi(js_name = "followSync")]
    pub fn follow_sync(
        &self,
        start: String,
        link: String,
        hops: u32,
    ) -> napi::Result<Vec<JsFollowResult>> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        let results = mv.follow(&start, &link, hops as usize);
        Ok(results.iter().map(from_follow_result).collect())
    }

    /// Follow relationships from an entity in the graph (async).
    #[napi(js_name = "follow")]
    pub async fn follow(
        &self,
        start: String,
        link: String,
        hops: u32,
    ) -> napi::Result<Vec<JsFollowResult>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            let results = mv.follow(&start, &link, hops as usize);
            Ok(results.iter().map(from_follow_result).collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- findEntity ----------------------------------------------------------

    /// Find an entity node by name (case-insensitive, synchronous).
    /// Returns null if not found.
    #[napi(js_name = "findEntitySync")]
    pub fn find_entity_sync(&self, name: String) -> napi::Result<Option<JsMeshNode>> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(mv.find_entity(&name).map(from_mesh_node))
    }

    /// Find an entity node by name (case-insensitive, async).
    /// Returns null if not found.
    #[napi(js_name = "findEntity")]
    pub async fn find_entity(&self, name: String) -> napi::Result<Option<JsMeshNode>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            Ok(mv.find_entity(&name).map(from_mesh_node))
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- frameEntities -------------------------------------------------------

    /// Get all entities mentioned in a specific frame (synchronous).
    #[napi(js_name = "frameEntitiesSync")]
    pub fn frame_entities_sync(&self, frame_id: i64) -> napi::Result<Vec<JsMeshNode>> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        let nodes = mv.frame_entities(frame_id as u64);
        Ok(nodes.iter().map(|n| from_mesh_node(n)).collect())
    }

    /// Get all entities mentioned in a specific frame (async).
    #[napi(js_name = "frameEntities")]
    pub async fn frame_entities(&self, frame_id: i64) -> napi::Result<Vec<JsMeshNode>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            let nodes = mv.frame_entities(frame_id as u64);
            Ok(nodes.iter().map(|n| from_mesh_node(n)).collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- entitiesByKind ------------------------------------------------------

    /// Get all entities of a specific kind (synchronous).
    /// Kind is a string: "person", "organization", "project", etc.
    #[napi(js_name = "entitiesByKindSync")]
    pub fn entities_by_kind_sync(&self, kind: String) -> napi::Result<Vec<JsMeshNode>> {
        let entity_kind = EntityKind::from_label(&kind);
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        let nodes = mv.entities_by_kind(entity_kind);
        Ok(nodes.iter().map(|n| from_mesh_node(n)).collect())
    }

    /// Get all entities of a specific kind (async).
    #[napi(js_name = "entitiesByKind")]
    pub async fn entities_by_kind(&self, kind: String) -> napi::Result<Vec<JsMeshNode>> {
        let entity_kind = EntityKind::from_label(&kind);
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            let nodes = mv.entities_by_kind(entity_kind);
            Ok(nodes.iter().map(|n| from_mesh_node(n)).collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- logicMeshStats ------------------------------------------------------

    /// Get statistics about the Logic-Mesh.
    #[napi(js_name = "logicMeshStats")]
    pub fn logic_mesh_stats(&self) -> napi::Result<JsLogicMeshStats> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        let stats = mv.logic_mesh_stats();
        Ok(JsLogicMeshStats {
            node_count: stats.node_count as u32,
            edge_count: stats.edge_count as u32,
            entity_kinds: stats
                .entity_kinds
                .into_iter()
                .map(|(k, v)| (k, v as u32))
                .collect(),
            link_types: stats
                .link_types
                .into_iter()
                .map(|(k, v)| (k, v as u32))
                .collect(),
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
    fn to_mesh_node_basic() {
        let js = JsMeshNode {
            canonical_name: "anthropic".to_string(),
            display_name: "Anthropic".to_string(),
            kind: "organization".to_string(),
            confidence: Some(0.95),
            frame_ids: Some(vec![1, 2, 3]),
        };
        let node = to_mesh_node(&js);
        assert_eq!(node.canonical_name, "anthropic");
        assert_eq!(node.display_name, "Anthropic");
        assert_eq!(node.kind, EntityKind::Organization);
        assert!((node.confidence_f32() - 0.95).abs() < 0.01);
        assert_eq!(node.frame_ids, vec![1, 2, 3]);
    }

    #[test]
    fn to_mesh_node_defaults() {
        let js = JsMeshNode {
            canonical_name: "alice".to_string(),
            display_name: "Alice".to_string(),
            kind: "person".to_string(),
            confidence: None,
            frame_ids: None,
        };
        let node = to_mesh_node(&js);
        assert_eq!(node.kind, EntityKind::Person);
        assert!((node.confidence_f32() - 1.0).abs() < 0.01);
        assert_eq!(node.frame_ids, vec![0]); // default frame_id
    }

    #[test]
    fn from_mesh_node_roundtrip() {
        let rust_node = MeshNode::new(
            "anthropic".to_string(),
            "Anthropic".to_string(),
            EntityKind::Organization,
            0.9,
            42,
            0,
            0,
        );
        let js = from_mesh_node(&rust_node);
        assert_eq!(js.canonical_name, "anthropic");
        assert_eq!(js.display_name, "Anthropic");
        assert_eq!(js.kind, "organization");
        assert!(js.confidence.unwrap() > 0.89);
        assert_eq!(js.frame_ids.unwrap(), vec![42]);
    }

    #[test]
    fn to_mesh_edge_basic() {
        let js = JsMeshEdge {
            from_node: 100,
            to_node: 200,
            link: "manager".to_string(),
            confidence: Some(0.8),
            frame_id: Some(5),
        };
        let edge = to_mesh_edge(&js);
        assert_eq!(edge.from_node, 100);
        assert_eq!(edge.to_node, 200);
        assert_eq!(edge.link.as_str(), "manager");
        assert!((edge.confidence_f32() - 0.8).abs() < 0.01);
        assert_eq!(edge.frame_id, 5);
    }

    #[test]
    fn to_mesh_edge_defaults() {
        let js = JsMeshEdge {
            from_node: 10,
            to_node: 20,
            link: "related".to_string(),
            confidence: None,
            frame_id: None,
        };
        let edge = to_mesh_edge(&js);
        assert!((edge.confidence_f32() - 1.0).abs() < 0.01);
        assert_eq!(edge.frame_id, 0);
    }

    #[test]
    fn from_follow_result_conversion() {
        let rust = FollowResult {
            node: "Bob".to_string(),
            kind: EntityKind::Person,
            confidence: 0.85,
            frame_ids: vec![1, 2],
            path_length: 2,
        };
        let js = from_follow_result(&rust);
        assert_eq!(js.node, "Bob");
        assert_eq!(js.kind, "person");
        assert!((js.confidence - 0.85).abs() < 0.01);
        assert_eq!(js.frame_ids, vec![1, 2]);
        assert_eq!(js.path_length, 2);
    }

    #[test]
    fn entity_kind_parsing() {
        for (label, expected) in [
            ("person", "person"),
            ("organization", "organization"),
            ("project", "project"),
            ("email", "email"),
            ("date", "date"),
            ("location", "location"),
            // Note: "product" is an alias for Project in from_label()
            ("product", "project"),
            ("event", "event"),
            ("money", "money"),
            ("url", "url"),
            ("unknown_kind", "other"),
        ] {
            let kind = EntityKind::from_label(label);
            assert_eq!(kind.as_str(), expected, "Mismatch for label '{label}'");
        }
    }

    #[test]
    fn link_type_parsing() {
        for (s, expected) in [
            ("manager", "manager"),
            ("member", "member"),
            ("owner", "owner"),
            ("employer", "employer"),
            ("related", "related"),
            ("custom_link", "custom_link"),
        ] {
            let link = LinkType::from_str(s);
            assert_eq!(link.as_str(), expected, "Mismatch for link string '{s}'");
        }
    }

    #[test]
    fn closed_add_mesh_node_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let node = JsMeshNode {
            canonical_name: "test".to_string(),
            display_name: "Test".to_string(),
            kind: "person".to_string(),
            confidence: None,
            frame_ids: None,
        };
        let result = mv.add_mesh_node_sync(node);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("CLOSED"), "Expected CLOSED error, got: {err_msg}");
    }

    #[test]
    fn closed_follow_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.follow_sync("start".to_string(), "link".to_string(), 1) {
            Err(e) => assert!(e.to_string().contains("CLOSED"), "Expected CLOSED, got: {e}"),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }

    #[test]
    fn closed_find_entity_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.find_entity_sync("test".to_string()) {
            Err(e) => assert!(e.to_string().contains("CLOSED"), "Expected CLOSED, got: {e}"),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }

    #[test]
    fn closed_logic_mesh_stats_returns_error() {
        let mv = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        match mv.logic_mesh_stats() {
            Err(e) => assert!(e.to_string().contains("CLOSED"), "Expected CLOSED, got: {e}"),
            Ok(_) => panic!("Expected error for closed instance"),
        }
    }
}
