//! Pure HTML rendering for `kb graph`. No I/O here — [`render_graph_html`]
//! takes a fully hydrated [`GraphExport`] and returns a self-contained HTML
//! string. Writing it out / serving it to a browser is `cli::browser`'s job.

use crate::domain::GraphExport;
use crate::errors::Error;

/// Placeholder substituted with the `GraphExport` JSON in the template.
const DATA_PLACEHOLDER: &str = "__GRAPH_DATA__";

/// Template asset — colocated under `assets/` since `include_str!` paths
/// resolve relative to this file.
const TEMPLATE: &str = include_str!("assets/graph_view.html");

/// Renders a complete, self-contained HTML page for `export`: the graph
/// data is serialized to JSON and inlined into the template, which loads
/// vis-network from a CDN and wires up drag/click/highlight behaviour.
pub fn render_graph_html(export: &GraphExport) -> Result<String, Error> {
    let json = serde_json::to_string(export).map_err(|e| Error::GraphQueryError(e.to_string()))?;
    Ok(TEMPLATE.replacen(DATA_PLACEHOLDER, &json, 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{GraphExportEdge, GraphExportNode};

    fn sample_export() -> GraphExport {
        GraphExport {
            root_id: "car-id".to_string(),
            root_key: "car".to_string(),
            nodes: vec![
                GraphExportNode {
                    id: "car-id".to_string(),
                    key: "car".to_string(),
                    value: "a car".to_string(),
                    notes: String::new(),
                    category: "concept".to_string(),
                    namespace: "default".to_string(),
                    reference: String::new(),
                    tags: vec!["vehicle".to_string()],
                    path: None,
                    created_on: "2026-01-01T00:00:00+0000".to_string(),
                },
                GraphExportNode {
                    id: "engine-id".to_string(),
                    key: "engine".to_string(),
                    value: "an engine".to_string(),
                    notes: String::new(),
                    category: "concept".to_string(),
                    namespace: "default".to_string(),
                    reference: String::new(),
                    tags: vec![],
                    path: None,
                    created_on: "2026-01-01T00:00:00+0000".to_string(),
                },
            ],
            edges: vec![GraphExportEdge {
                id: "edge-id".to_string(),
                from_id: "car-id".to_string(),
                to_id: "engine-id".to_string(),
                note: "has an engine".to_string(),
                created_on: "2026-01-01T00:00:00+0000".to_string(),
            }],
        }
    }

    #[test]
    fn embeds_the_graph_json_in_place_of_the_placeholder() {
        let html = render_graph_html(&sample_export()).unwrap();

        assert!(!html.contains(DATA_PLACEHOLDER));
        assert!(html.contains("\"root_key\":\"car\""));
        assert!(html.contains("\"key\":\"engine\""));
        assert!(html.contains("\"note\":\"has an engine\""));
    }

    #[test]
    fn keeps_the_vis_network_script_tag() {
        let html = render_graph_html(&sample_export()).unwrap();

        assert!(html.contains("vis-network"));
    }
}
