use crate::error::FrontendError;
use crate::graphql::authenticated::{IdOrNewInput, schema};
use crate::graphql::{mutate, query, query_simple};
use std::fmt::{Display, Formatter};
use std::fs::write;
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "FlatPanelInput")]
pub struct FlatPanelInput {
    pub id: IdOrNewInput,
    pub name: Option<String>,
    pub parent_id: Option<IdOrNewInput>,
    pub order: i32,
    pub netbox_device_id: Option<i32>,
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
    #[allow(unused)]
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

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct QueryAvailableNetboxPanels {
    netbox_devices: Vec<OverviewNetboxDevice>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[cynic(graphql_type = "DeviceWithRearPorts")]
pub struct OverviewNetboxDevice {
    pub id: i32,
    pub name: Option<String>,
    pub device_type: String,
    pub location_name: Option<String>,
}
impl Display for OverviewNetboxDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(loc) = self.location_name.as_deref() {
            write!(f, "{loc} ")?;
        }
        if let Some(name) = self.name.as_deref() {
            write!(f, "{name} ")?;
        } else {
            write!(f, "{} ", self.id)?;
        }
        write!(f, "({})", self.device_type)
    }
}
impl OverviewNetboxDevice {
    pub async fn list_devices(
        credentials: Option<OAuth2Context>,
    ) -> Result<Box<[OverviewNetboxDevice]>, FrontendError> {
        Ok(
            query_simple::<QueryAvailableNetboxPanels, _>((), credentials.as_ref())
                .await?
                .netbox_devices
                .into_boxed_slice(),
        )
    }
}
