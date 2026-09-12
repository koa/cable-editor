use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{IdOrNewInput, PortType, schema},
        mutate, query, query_simple,
    },
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "FlatPortInput")]
pub struct FlatPortInput {
    pub id: IdOrNewInput,
    pub order: i32,
    pub label: String,
    pub port_type: PortType,
    pub netbox_port_id: Option<i32>,
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
    #[allow(unused)]
    pub update_panel_ports: bool,
}

#[derive(cynic::QueryVariables, Debug)]
struct FetchPanelPortsVariables {
    panel_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = FetchPanelPortsVariables)]
struct FetchPanelPortsQuery {
    #[arguments(panelId: $panel_id)]
    panel: Option<PanelWithPorts>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Panel")]
struct PanelWithPorts {
    name: Option<String>,
    ports: Vec<PanelPortEntry>,
    parent_chain: Vec<ParentPanel>,
    netbox_device: Option<NetboxDeviceId>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Panel")]
struct ParentPanel {
    netbox_device: Option<NetboxDeviceId>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "DeviceWithRearPorts")]
struct NetboxDeviceId {
    id: i32,
}
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "PanelPort")]
pub struct PanelPortEntry {
    pub id: i32,
    pub order_number: i32,
    pub label: Option<String>,
    pub port_type: PortType,
    pub netbox_port: Option<NetboxPortId>,
}
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "RearPort")]
pub struct NetboxPortId {
    pub id: i32,
}

#[derive(Debug, Clone, Default)]
pub struct FetchedPanelWithPorts {
    pub ports: Vec<PanelPortEntry>,
    pub panel_name: Option<String>,
    pub schacht_name: Option<String>,
    pub netbox_device_id: Option<i32>,
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
                netbox_device_id: p
                    .netbox_device
                    .or(p.parent_chain.into_iter().find_map(|p| p.netbox_device))
                    .map(|p| p.id),
            })
            .unwrap_or_default())
    }
}

#[derive(cynic::QueryVariables, Debug)]
struct FetchNetboxDevicePortsVariables {
    netbox_device_id: i32,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = FetchNetboxDevicePortsVariables)]
struct FetchNetboxDevicePorts {
    #[arguments(netboxDeviceId: $netbox_device_id)]
    netbox_device: Option<NetboxDeviceWithPorts>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "DeviceWithRearPorts")]
struct NetboxDeviceWithPorts {
    rear_ports: Vec<NetboxDevicePort>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "RearPort")]
pub struct NetboxDevicePort {
    pub id: i32,
    pub name: String,
}
impl NetboxDevicePort {
    pub async fn fetch_ports(
        credentials: Option<&OAuth2Context>,
        netbox_device_id: i32,
    ) -> Result<Box<[NetboxDevicePort]>, FrontendError> {
        Ok(query_simple::<FetchNetboxDevicePorts, _>(
            FetchNetboxDevicePortsVariables { netbox_device_id },
            credentials,
        )
        .await?
        .netbox_device
        .ok_or(FrontendError::NotFound)?
        .rear_ports
        .into_boxed_slice())
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
