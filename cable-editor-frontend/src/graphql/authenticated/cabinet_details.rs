use crate::error::FrontendError;
use crate::graphql::authenticated::{IdOrNew, IdOrNewInput, schema};
use crate::graphql::{mutate, query};
use cynic::GraphQlResponse;
use log::info;
use std::collections::{BTreeMap, HashMap, HashSet};
use yew_oauth2::prelude::OAuth2Context;

#[derive(cynic::QueryVariables)]
struct Variables {
    id: i32,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
struct FetchDuctDetailsQuery {
    #[arguments(schachtId: $id)]
    pub schacht: Option<SchachtDetails>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Schacht")]
struct SchachtDetails {
    root_panels: Vec<RootPanelEntry>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Panel")]
struct RootPanelEntry {
    id: i32,
    name: Option<String>,
    all_children_recursive: Vec<ChildPanelEntry>,
    count_ports: i32,
    #[arguments(portType: "LOOP")]
    #[cynic(rename = "countPorts", alias)]
    count_loop_ports: i32,
    netbox_device: Option<NetboxDeviceId>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Panel")]
struct ChildPanelEntry {
    id: i32,
    name: Option<String>,
    parent_id: Option<i32>,
    parent_order: Option<i32>,
    count_ports: i32,
    #[arguments(portType: "LOOP")]
    #[cynic(rename = "countPorts", alias)]
    count_loop_ports: i32,
    netbox_device: Option<NetboxDeviceId>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "DeviceWithRearPorts")]
struct NetboxDeviceId {
    id: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelTreeEntry {
    pub id: i32,
    pub name: Option<Box<str>>,
    pub children: Box<[PanelTreeEntry]>,
    pub has_loop: bool,
    pub port_count: usize,
    pub netbox_device_id: Option<i32>,
}
struct PanelEntryData {
    name: Option<Box<str>>,
    has_loop: bool,
    port_count: usize,
    netbox_device_id: Option<i32>,
}

impl PanelTreeEntry {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        id: i32,
    ) -> Result<Box<[PanelTreeEntry]>, FrontendError> {
        let response = query::<FetchDuctDetailsQuery, _>(Variables { id }, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            let mut panel_data = HashMap::new();
            let mut children = HashMap::<i32, BTreeMap<i32, i32>>::new();
            let mut is_child = HashSet::new();
            for root_entry in response
                .data
                .and_then(|d| d.schacht)
                .map(|s| s.root_panels)
                .unwrap_or_default()
            {
                for child in root_entry.all_children_recursive {
                    if let ChildPanelEntry {
                        id,
                        parent_id: Some(parent_id),
                        parent_order: Some(parent_order),
                        ..
                    } = child
                    {
                        is_child.insert(id);
                        children
                            .entry(parent_id)
                            .or_default()
                            .insert(parent_order, id);
                    }
                    panel_data.insert(
                        child.id,
                        PanelEntryData {
                            name: child.name.map(|v| v.into_boxed_str()),
                            has_loop: child.count_loop_ports > 0,
                            port_count: child.count_ports as usize,
                            netbox_device_id: child.netbox_device.map(|d| d.id),
                        },
                    );
                }
                panel_data.insert(
                    root_entry.id,
                    PanelEntryData {
                        name: root_entry.name.map(|v| v.into_boxed_str()),
                        has_loop: root_entry.count_loop_ports > 0,
                        port_count: root_entry.count_ports as usize,
                        netbox_device_id: root_entry.netbox_device.map(|d| d.id),
                    },
                );
            }
            let roots = panel_data
                .keys()
                .copied()
                .filter(|id| !is_child.contains(id))
                .collect::<Vec<_>>();
            info!("Roots: {roots:?}");
            let data = roots
                .into_iter()
                .map(|root_id| collect_children(root_id, &mut children, &mut panel_data))
                .collect();
            info!("Data: {data:?}");
            info!("Children: {children:?}");
            assert!(children.is_empty());
            assert!(panel_data.is_empty());
            Ok(data)
        }
    }
}

fn collect_children(
    entry_id: i32,
    children: &mut HashMap<i32, BTreeMap<i32, i32>>,
    panel_data: &mut HashMap<i32, PanelEntryData>,
) -> PanelTreeEntry {
    let child_map = children.remove(&entry_id).unwrap_or_default();
    let mut child_results = Vec::with_capacity(child_map.len());
    for child_id in child_map.into_values() {
        child_results.push(collect_children(child_id, children, panel_data));
    }
    if let Some(entry_data) = panel_data.remove(&entry_id) {
        PanelTreeEntry {
            id: entry_id,
            name: entry_data.name,
            children: child_results.into_boxed_slice(),
            has_loop: entry_data.has_loop,
            port_count: entry_data.port_count,
            netbox_device_id: entry_data.netbox_device_id,
        }
    } else {
        PanelTreeEntry {
            id: entry_id,
            name: None,
            children: child_results.into_boxed_slice(),
            has_loop: false,
            port_count: 0,
            netbox_device_id: None,
        }
    }
}
#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "CreatePanel")]
pub struct CreatePanelInput {
    pub name: String,
    pub schacht_id: i32,
    pub children: Vec<CreatePanelInput>,
}
#[derive(cynic::QueryVariables)]
struct CreatePanelVariables {
    panel: CreatePanelInput,
    parent_panel: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CreatePanelVariables")]
struct CreatePanelQuery {
    #[arguments(panel: $panel, parentPanel: $parent_panel)]
    #[allow(unused)]
    create_panel: bool,
}
pub async fn create_panel(
    credentials: Option<&OAuth2Context>,
    panel: CreatePanelInput,
    parent_panel: Option<i32>,
) -> Result<(), FrontendError> {
    let response = mutate::<CreatePanelQuery, _>(
        CreatePanelVariables {
            panel,
            parent_panel,
        },
        credentials,
    )
    .await?;
    if let Some(errors) = response.errors {
        Err(FrontendError::Graphql(errors))
    } else {
        Ok(())
    }
}
