use std::str::FromStr;

use super::*;

fn make_kb() -> Kb {
    Kb {
        id: "kb-id-1".to_string(),
        key: "car".to_string(),
        value: "a car".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "vehicles".to_string(),
        tags: vec![],
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
    assert_eq!(node.key, "car");
    assert_eq!(node.category, "concept");
    assert_eq!(node.namespace, "vehicles");
}

fn make_export_kb_item(key: &str) -> ExportKbItem {
    ExportKbItem {
        key: key.to_string(),
        value: "a value".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "vehicles".to_string(),
        tags: vec![],
        parent_key: None,
        path: None,
        media_extension: None,
    }
}

#[test]
fn export_edge_item_serializes_with_capitalized_keys() {
    let edge = ExportEdgeItem {
        from_key: "motogp-twitter".to_string(),
        to_key: "motogp-quote".to_string(),
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
            from_key: "a".to_string(),
            to_key: "b".to_string(),
            note: String::new(),
        }],
    };
    let yaml = serde_yaml::to_string(&doc).expect("serialize");
    assert!(yaml.starts_with("kbs:"));
    assert!(yaml.contains("graph:"));
    assert!(yaml.contains("From: a"));
    assert!(yaml.contains("Key: motogp-twitter"));
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
    assert_eq!(item.from_key, "a");
    assert_eq!(item.to_key, "b");
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
        from_key: "motogp-twitter".to_string(),
        to_key: "motogp-quote".to_string(),
        note: "source account".to_string(),
    };
    let yaml = serde_yaml::to_string(&export_edge).expect("serialize");
    let import_edge: ImportEdgeItem = serde_yaml::from_str(&yaml).expect("deserialize");
    assert_eq!(import_edge.from_key, export_edge.from_key);
    assert_eq!(import_edge.to_key, export_edge.to_key);
    assert_eq!(import_edge.note, export_edge.note);
}

#[test]
fn import_document_deserializes_kbs_and_graph_sections() {
    let yaml = "kbs:\n  - Key: car\n    Value: a value\ngraph:\n  - From: car\n    To: engine\n    Note: has an engine\n";
    let doc: ImportDocument = serde_yaml::from_str(yaml).expect("deserialize");
    assert_eq!(doc.kbs.len(), 1);
    assert_eq!(doc.kbs[0].key, "car");
    assert_eq!(doc.graph.len(), 1);
    assert_eq!(doc.graph[0].from_key, "car");
    assert_eq!(doc.graph[0].to_key, "engine");
}

#[test]
fn import_document_defaults_missing_graph_section_to_empty() {
    let yaml = "kbs:\n  - Key: car\n    Value: a value\n";
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
            from_key: "car".to_string(),
            to_key: "engine".to_string(),
            note: "has an engine".to_string(),
        }],
    };
    let yaml = serde_yaml::to_string(&export_doc).expect("serialize");
    let import_doc: ImportDocument = serde_yaml::from_str(&yaml).expect("deserialize");
    assert_eq!(import_doc.kbs.len(), export_doc.kbs.len());
    assert_eq!(import_doc.kbs[0].key, export_doc.kbs[0].key);
    assert_eq!(import_doc.graph.len(), export_doc.graph.len());
    assert_eq!(import_doc.graph[0].from_key, export_doc.graph[0].from_key);
}

#[test]
fn failed_import_edge_item_carries_item_and_reason() {
    let failed = FailedImportEdgeItem {
        item: make_import_kb_edge_item("car", "engine"),
        reason: "to key/id not found: engine".to_string(),
    };
    assert_eq!(failed.item.from_key, "car");
    assert_eq!(failed.reason, "to key/id not found: engine");
}

fn make_import_kb_edge_item(from_key: &str, to_key: &str) -> ImportEdgeItem {
    ImportEdgeItem {
        from_key: from_key.to_string(),
        to_key: to_key.to_string(),
        note: String::new(),
    }
}
