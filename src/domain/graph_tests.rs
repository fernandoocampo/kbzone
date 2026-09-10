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
