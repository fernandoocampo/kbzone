use std::str::FromStr;

use super::*;

fn make_kb() -> Kb {
    Kb {
        id: "kb-id-1".to_string(),
        value: "a car".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "vehicles".to_string(),
        tags: vec![],
        metadata: std::collections::BTreeMap::new(),
        created_on: "2026-01-01T00:00:00+0000".to_string(),
        parent: None,
        path: None,
        media_extension: None,
    }
}

#[test]
fn new_kb_edge_generates_id_and_timestamp() {
    let new_edge = NewKbEdge {
        from_id: "from-id".to_string(),
        to_id: "to-id".to_string(),
        note: "car has an engine".to_string(),
    };
    let edge = KbEdge::from(new_edge);
    assert!(!edge.id.is_empty());
    assert!(!edge.created_on.is_empty());
    assert_eq!(edge.from_id, "from-id");
    assert_eq!(edge.to_id, "to-id");
    assert_eq!(edge.note, "car has an engine");
}

#[test]
fn new_kb_edge_ids_are_unique_per_call() {
    let make = || {
        KbEdge::from(NewKbEdge {
            from_id: "a".to_string(),
            to_id: "b".to_string(),
            note: String::new(),
        })
    };
    assert_ne!(make().id, make().id);
}

#[test]
fn edge_direction_from_str_parses_valid_values() {
    assert_eq!(EdgeDirection::from_str("out"), Ok(EdgeDirection::Out));
    assert_eq!(EdgeDirection::from_str("in"), Ok(EdgeDirection::In));
    assert_eq!(EdgeDirection::from_str("both"), Ok(EdgeDirection::Both));
}

#[test]
fn edge_direction_from_str_rejects_invalid_value() {
    assert!(EdgeDirection::from_str("sideways").is_err());
}

#[test]
fn edge_direction_as_str_round_trips() {
    assert_eq!(EdgeDirection::Out.as_str(), "out");
    assert_eq!(EdgeDirection::In.as_str(), "in");
    assert_eq!(EdgeDirection::Both.as_str(), "both");
}

#[test]
fn graph_node_from_kb_copies_the_relevant_fields() {
    let kb = make_kb();
    let node = GraphNode::from(&kb);
    assert_eq!(node.id, "kb-id-1");
    assert_eq!(node.category, "concept");
    assert_eq!(node.namespace, "vehicles");
}

#[test]
fn graph_export_node_from_kb_sets_label_to_untruncated_value_when_short() {
    let kb = make_kb(); // value = "a car"
    let node = GraphExportNode::from(&kb);
    assert_eq!(node.label, "a car");
    assert_eq!(node.value, "a car");
}

#[test]
fn graph_export_node_from_kb_truncates_label_for_long_value() {
    let mut kb = make_kb();
    kb.value = "x".repeat(50);
    let node = GraphExportNode::from(&kb);
    assert_eq!(node.label.chars().count(), 41); // 40 chars + ellipsis
    assert!(node.label.ends_with('…'));
    // The full, untruncated value is retained separately for the detail panel.
    assert_eq!(node.value.chars().count(), 50);
}

fn make_related_result() -> RelatedResult {
    RelatedResult {
        node: GraphNode::from(&make_kb()),
        outgoing: vec![OutgoingEdge {
            edge_id: "e1".to_string(),
            note: "has an engine".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
            to: GraphNode {
                id: "engine-id".to_string(),
                category: "concept".to_string(),
                namespace: "vehicles".to_string(),
            },
        }],
        incoming: vec![],
    }
}

#[test]
fn kb_relationships_from_related_result_drops_node() {
    let related = KbRelationships::from(make_related_result());
    assert_eq!(related.outgoing.len(), 1);
    assert_eq!(related.outgoing[0].to.id, "engine-id");
    assert!(related.incoming.is_empty());
    // `KbRelationships` has no `node` field at all — this is a compile-time
    // guarantee, not something a runtime assertion can check further.
}

#[test]
fn kb_with_relationships_json_omits_relationships_key_when_none() {
    let dto = KbWithRelationships {
        kb: make_kb(),
        relationships: None,
    };
    let json = serde_json::to_string(&dto).expect("serialize should succeed");
    assert!(!json.contains("relationships"));
    assert!(json.contains("\"id\":\"kb-id-1\""));
    assert!(!json.contains("\"kb\":"));
}

#[test]
fn kb_with_relationships_json_includes_relationships_key_when_some() {
    let dto = KbWithRelationships {
        kb: make_kb(),
        relationships: Some(KbRelationships::from(make_related_result())),
    };
    let json = serde_json::to_string(&dto).expect("serialize should succeed");
    assert!(json.contains("\"relationships\""));
    assert!(json.contains("\"outgoing\""));
    assert!(json.contains("\"incoming\""));
    assert!(json.contains("\"id\":\"kb-id-1\""));
    // The root entry is already flattened at the top level — `node` inside
    // `relationships` would just repeat it, so it must not be there.
    assert!(!json.contains("\"node\""));
}

#[test]
fn kb_with_relationships_yaml_flattens_kb_fields() {
    let dto = KbWithRelationships {
        kb: make_kb(),
        relationships: Some(KbRelationships::from(make_related_result())),
    };
    let yaml = serde_yaml::to_string(&dto).expect("serialize should succeed");
    assert!(!yaml.contains("kb:"));
    assert!(yaml.contains("id: kb-id-1"));
    assert!(yaml.contains("relationships:"));
    assert!(!yaml.contains("node:"));
}

fn make_export_kb_item(id: &str) -> ExportKbItem {
    ExportKbItem {
        id: id.to_string(),
        value: "a value".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "vehicles".to_string(),
        tags: vec![],
        parent_id: None,
        path: None,
        media_extension: None,
    }
}

#[test]
fn export_edge_item_serializes_with_capitalized_keys() {
    let edge = ExportEdgeItem {
        from_id: "motogp-twitter".to_string(),
        to_id: "motogp-quote".to_string(),
        note: "source account".to_string(),
    };
    let yaml = serde_yaml::to_string(&edge).expect("serialize");
    assert!(yaml.contains("From: motogp-twitter"));
    assert!(yaml.contains("To: motogp-quote"));
    assert!(yaml.contains("Note: source account"));
}

#[test]
fn export_document_serializes_kbs_and_graph_sections() {
    let doc = ExportDocument {
        kbs: vec![make_export_kb_item("motogp-twitter")],
        graph: vec![ExportEdgeItem {
            from_id: "a".to_string(),
            to_id: "b".to_string(),
            note: String::new(),
        }],
    };
    let yaml = serde_yaml::to_string(&doc).expect("serialize");
    assert!(yaml.starts_with("kbs:"));
    assert!(yaml.contains("graph:"));
    assert!(yaml.contains("From: a"));
    assert!(yaml.contains("Id: motogp-twitter"));
}

#[test]
fn export_document_serializes_empty_graph_as_empty_sequence() {
    let doc = ExportDocument {
        kbs: vec![],
        graph: vec![],
    };
    let yaml = serde_yaml::to_string(&doc).expect("serialize");
    assert!(yaml.contains("graph: []"));
}

#[test]
fn import_edge_item_deserializes_from_pascal_case_yaml() {
    let yaml = "From: a\nTo: b\nNote: n\n";
    let item: ImportEdgeItem = serde_yaml::from_str(yaml).expect("deserialize");
    assert_eq!(item.from_id, "a");
    assert_eq!(item.to_id, "b");
    assert_eq!(item.note, "n");
}

#[test]
fn import_edge_item_note_defaults_when_absent() {
    let yaml = "From: a\nTo: b\n";
    let item: ImportEdgeItem = serde_yaml::from_str(yaml).expect("deserialize");
    assert!(item.note.is_empty());
}

#[test]
fn import_edge_item_round_trips_from_export_edge_item_output() {
    let export_edge = ExportEdgeItem {
        from_id: "motogp-twitter".to_string(),
        to_id: "motogp-quote".to_string(),
        note: "source account".to_string(),
    };
    let yaml = serde_yaml::to_string(&export_edge).expect("serialize");
    let import_edge: ImportEdgeItem = serde_yaml::from_str(&yaml).expect("deserialize");
    assert_eq!(import_edge.from_id, export_edge.from_id);
    assert_eq!(import_edge.to_id, export_edge.to_id);
    assert_eq!(import_edge.note, export_edge.note);
}

#[test]
fn import_document_deserializes_kbs_and_graph_sections() {
    let yaml = "kbs:\n  - Id: car\n    Value: a value\ngraph:\n  - From: car\n    To: engine\n    Note: has an engine\n";
    let doc: ImportDocument = serde_yaml::from_str(yaml).expect("deserialize");
    assert_eq!(doc.kbs.len(), 1);
    assert_eq!(doc.kbs[0].id, "car");
    assert_eq!(doc.graph.len(), 1);
    assert_eq!(doc.graph[0].from_id, "car");
    assert_eq!(doc.graph[0].to_id, "engine");
}

#[test]
fn import_document_defaults_missing_graph_section_to_empty() {
    let yaml = "kbs:\n  - Id: car\n    Value: a value\n";
    let doc: ImportDocument = serde_yaml::from_str(yaml).expect("deserialize");
    assert!(doc.graph.is_empty());
}

#[test]
fn import_document_defaults_missing_kbs_section_to_empty() {
    let yaml = "graph:\n  - From: car\n    To: engine\n";
    let doc: ImportDocument = serde_yaml::from_str(yaml).expect("deserialize");
    assert!(doc.kbs.is_empty());
}

#[test]
fn import_document_round_trips_from_export_document_output() {
    let export_doc = ExportDocument {
        kbs: vec![make_export_kb_item("car")],
        graph: vec![ExportEdgeItem {
            from_id: "car".to_string(),
            to_id: "engine".to_string(),
            note: "has an engine".to_string(),
        }],
    };
    let yaml = serde_yaml::to_string(&export_doc).expect("serialize");
    let import_doc: ImportDocument = serde_yaml::from_str(&yaml).expect("deserialize");
    assert_eq!(import_doc.kbs.len(), export_doc.kbs.len());
    assert_eq!(import_doc.kbs[0].id, export_doc.kbs[0].id);
    assert_eq!(import_doc.graph.len(), export_doc.graph.len());
    assert_eq!(import_doc.graph[0].from_id, export_doc.graph[0].from_id);
}

#[test]
fn failed_import_edge_item_carries_item_and_reason() {
    let failed = FailedImportEdgeItem {
        item: make_import_kb_edge_item("car", "engine"),
        reason: "to id not found: engine".to_string(),
    };
    assert_eq!(failed.item.from_id, "car");
    assert_eq!(failed.reason, "to id not found: engine");
}

fn make_import_kb_edge_item(from_id: &str, to_id: &str) -> ImportEdgeItem {
    ImportEdgeItem {
        from_id: from_id.to_string(),
        to_id: to_id.to_string(),
        note: String::new(),
    }
}
