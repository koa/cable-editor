use crate::error::FrontendError;
use crate::graphql::authenticated::IdOrNewInput;
use crate::graphql::authenticated::schema;
use crate::graphql::{mutate, query};
use crate::util::get_credentials;
use std::str::FromStr;
use yew_oauth2::context::OAuth2Context;
// Import für mutate nicht vergessen

#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "FlatPortInput")]
pub struct FlatPortInput {
    pub id: IdOrNewInput, // Das Struct aus der Panel-Logik
    pub order: i32,
    pub label: String,
    pub port_type: PortType,
}

#[derive(cynic::QueryVariables, Debug)]
struct SyncPanelPortsVariables {
    panel_id: i32,
    changes: Vec<FlatPortInput>,
    deletes: Vec<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SyncPanelPortsVariables")]
struct SyncPanelPortsMutation {
    #[arguments(panelId: $panel_id, changes: $changes, deletes: $deletes)]
    pub update_panel_ports: bool,
}

// Das Enum für den Typ
#[derive(Clone, Copy, PartialEq, Eq, Debug, strum::Display, cynic::Enum)]
#[cynic(graphql_type = "PanelPortType")]
pub enum PortType {
    Splice,
    Connector,
    Loop,
}

/*impl PortType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PortType::Splice => "Splice",
            PortType::Connector => "Connector",
            PortType::Loop => "Loop",
        }
    }
}

impl FromStr for PortType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Splice" => Ok(PortType::Splice),
            "Connector" => Ok(PortType::Connector),
            "Loop" => Ok(PortType::Loop),
            _ => Err(()),
        }
    }
}*/

#[derive(cynic::QueryVariables, Debug)]
struct FetchPanelPortsVariables {
    panel_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "FetchPanelPortsVariables")]
struct FetchPanelPortsQuery {
    #[arguments(panelId: $panel_id)]
    panel: Option<PanelWithPorts>,
}

// Der Wrapper für das Panel
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Panel")]
struct PanelWithPorts {
    name: Option<String>,
    ports: Vec<PanelPortEntry>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "PanelPort")]
pub struct PanelPortEntry {
    pub id: i32,
    pub order_number: i32,
    pub label: Option<String>,
    pub port_type: PortType,
}

#[derive(Debug, Clone, Default)]
pub struct FetchedPanelWithPorts {
    pub ports: Vec<PanelPortEntry>,
    pub panel_name: Option<String>,
    pub schacht_name: Option<String>,
}

impl FetchedPanelWithPorts {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        panel_id: i32,
    ) -> Result<FetchedPanelWithPorts, FrontendError> {
        let variables = FetchPanelPortsVariables { panel_id };
        Ok(query::<FetchPanelPortsQuery, _>(variables, credentials)
            .await?
            .data
            .and_then(|d| d.panel)
            .map(|p| FetchedPanelWithPorts {
                ports: p.ports,
                panel_name: p.name,
                schacht_name: None,
            })
            .unwrap_or_default())
    }
}

pub async fn update_panel_ports(
    credentials: Option<&OAuth2Context>,
    panel_id: i32,
    changes: Vec<FlatPortInput>,
    deletes: Vec<i32>,
) -> Result<(), FrontendError> {
    let variables = SyncPanelPortsVariables {
        panel_id,
        changes,
        deletes,
    };

    mutate::<SyncPanelPortsMutation, _>(variables, credentials).await?;
    Ok(())
}
