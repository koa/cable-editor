use crate::components::cabinet::edit::IdOrNew;
use crate::error::FrontendError;
use crate::graphql::authenticated::cable_details::CableDetails;
use crate::graphql::authenticated::schema;
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
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Panel")]
struct ChildPanelEntry {
    id: i32,
    name: Option<String>,
    parent_id: Option<i32>,
    parent_order: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelTreeEntry {
    pub id: i32,
    pub name: Option<Box<str>>,
    pub children: Box<[PanelTreeEntry]>,
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
            let mut names = HashMap::<i32, Option<Box<str>>>::new();
            let mut children = HashMap::<i32, BTreeMap<i32, i32>>::new();
            let mut is_child = HashSet::new();
            for root_entry in response
                .data
                .and_then(|d| d.schacht)
                .map(|s| s.root_panels)
                .unwrap_or_default()
            {
                names.insert(root_entry.id, root_entry.name.map(String::into_boxed_str));
                for child in root_entry.all_children_recursive {
                    names.insert(child.id, child.name.map(String::into_boxed_str));
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
                }
            }
            let roots = names
                .keys()
                .copied()
                .filter(|id| !is_child.contains(id))
                .collect::<Vec<_>>();
            info!("Roots: {roots:?}");
            let data = roots
                .into_iter()
                .map(|root_id| collect_children(root_id, &mut children, &mut names))
                .collect();
            info!("Data: {data:?}");
            info!("Children: {children:?}");
            assert!(children.is_empty());
            assert!(names.is_empty());
            Ok(data)
        }
    }
}

fn collect_children(
    entry_id: i32,
    children: &mut HashMap<i32, BTreeMap<i32, i32>>,
    names: &mut HashMap<i32, Option<Box<str>>>,
) -> PanelTreeEntry {
    let child_map = children.remove(&entry_id).unwrap_or_default();
    let name = names.remove(&entry_id).flatten();
    let mut child_results = Vec::with_capacity(child_map.len());
    for child_id in child_map.into_values() {
        child_results.push(collect_children(child_id, children, names));
    }
    PanelTreeEntry {
        id: entry_id,
        name,
        children: child_results.into_boxed_slice(),
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

#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "IdOrNewInput")]
pub struct IdOrNewInput {
    pub id: Option<i32>,
    pub temporary: Option<String>,
}

// Praktischer Helfer für die Konvertierung
impl From<IdOrNew> for IdOrNewInput {
    fn from(val: IdOrNew) -> Self {
        match val {
            IdOrNew::Id(id) => IdOrNewInput {
                id: Some(id),
                temporary: None,
            },
            IdOrNew::Temporary(uuid) => IdOrNewInput {
                id: None,
                temporary: Some(uuid.to_string()),
            },
        }
    }
}

// Ein einziges Input-Struct für Create UND Update
#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "FlatPanelInput")]
pub struct FlatPanelInput {
    pub id: IdOrNewInput,
    pub name: Option<String>,
    pub parent_id: Option<IdOrNewInput>,
    pub order: i32,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct SyncCabinetPanelsVariables {
    pub cabinet_id: i32,
    pub changes: Vec<FlatPanelInput>,
    pub deletes: Vec<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SyncCabinetPanelsVariables")]
struct SyncCabinetPanelsMutation {
    #[arguments(cabinetId: $cabinet_id, changes: $changes, deletes: $deletes)]
    update_cabinet_panels: bool,
}

pub async fn update_panels_in_cabinet(
    deletes: Vec<i32>,
    changes: Vec<FlatPanelInput>,
    cabinet_id: i32,
    credentials: Option<OAuth2Context>,
) -> Result<(), FrontendError> {
    mutate::<SyncCabinetPanelsMutation, _>(
        SyncCabinetPanelsVariables {
            cabinet_id,
            changes,
            deletes,
        },
        credentials.as_ref(),
    )
    .await
    .map(|_| ())
}
