use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{cable_details::CableSegmentEndSchacht, schema},
        query,
    },
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables)]
struct Variables {
    id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
struct FetchSchachtCablesQuery {
    #[arguments(schachtId: $id)]
    pub schacht: Option<SchachtCables>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct SchachtCables {
    pub id: i32,
    pub name: String,
    pub root_panels: Vec<SchachtRootPanel>,
    pub cables: Vec<SchachtCableEnd>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Panel")]
pub struct SchachtRootPanel {
    pub id: i32,
    pub name: Option<String>,
    pub count_ports: i32,
    pub all_children_recursive: Vec<SchachtChildPanel>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Panel")]
pub struct SchachtChildPanel {
    pub id: i32,
    pub name: Option<String>,
    pub parent_id: Option<i32>,
    pub parent_order: Option<i32>,
    pub count_ports: i32,
}

/// A panel of the Schacht in tree order.
#[derive(Debug, Clone, PartialEq)]
pub struct SchachtPanelEntry {
    pub id: i32,
    pub name: Option<String>,
    pub port_count: i32,
    /// Names of the parent panels, root first.
    pub parents: Vec<String>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "CableEnd")]
pub struct SchachtCableEnd {
    pub cable: SchachtCable,
    pub path: SchachtCablePath,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Cable")]
pub struct SchachtCable {
    pub id: i32,
    pub name: String,
}

/// Oriented from the requested Schacht, so far_schacht is the other end.
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "CablePath")]
pub struct SchachtCablePath {
    pub far_schacht: CableSegmentEndSchacht,
}

impl SchachtCableEnd {
    /// Cable label text: "<cable>-<destination>".
    pub fn label_text(&self) -> String {
        format!("{} - {}", self.cable.name, self.path.far_schacht.name)
    }
}

impl SchachtCables {
    /// All panels, each followed by its children (depth first, in panel order).
    pub fn panels(&self) -> Vec<SchachtPanelEntry> {
        fn push_children(
            root: &SchachtRootPanel,
            parent: i32,
            parents: &[String],
            out: &mut Vec<SchachtPanelEntry>,
        ) {
            let mut children: Vec<_> = root
                .all_children_recursive
                .iter()
                .filter(|c| c.parent_id == Some(parent))
                .collect();
            children.sort_by_key(|c| c.parent_order);
            for child in children {
                out.push(SchachtPanelEntry {
                    id: child.id,
                    name: child.name.clone(),
                    port_count: child.count_ports,
                    parents: parents.to_vec(),
                });
                let mut child_parents = parents.to_vec();
                child_parents.extend(child.name.clone());
                push_children(root, child.id, &child_parents, out);
            }
        }
        let mut out = Vec::new();
        for root in &self.root_panels {
            out.push(SchachtPanelEntry {
                id: root.id,
                name: root.name.clone(),
                port_count: root.count_ports,
                parents: Vec::new(),
            });
            push_children(
                root,
                root.id,
                &root.name.iter().cloned().collect::<Vec<_>>(),
                &mut out,
            );
        }
        out
    }

    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        id: i32,
    ) -> Result<SchachtCables, FrontendError> {
        query::<FetchSchachtCablesQuery, _>(Variables { id }, credentials)
            .await?
            .schacht
            .ok_or(FrontendError::NotFound)
    }
}
