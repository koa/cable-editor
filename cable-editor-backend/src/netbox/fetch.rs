use crate::db::entity::panel::PanelPort;
use crate::error::BackendError;
use crate::netbox::id::NumberId;
use crate::netbox::{fetch_device_with_ports, query, schema};
use async_graphql::{Context, Object};
use cynic::queries::VariableMatch;

#[derive(cynic::QueryVariables, Debug)]
pub struct DeviceFilterVariables {
    pub types: Option<Vec<PortTypeEnum>>,
}
#[derive(cynic::QueryVariables, Debug)]
pub struct DeviceIdVariables {
    pub device_id: NumberId,
    pub types: Option<Vec<PortTypeEnum>>,
}
#[derive(cynic::QueryVariables, Debug)]
pub struct RearPortIdVariables {
    pub rear_port_id: NumberId,
}

impl VariableMatch<DeviceFilterVariablesFields> for DeviceIdVariablesFields {}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = DeviceFilterVariables)]
pub struct QueryDevicesAndPorts {
    #[arguments(filters: { rear_ports: { type: { in_list: $types } } })]
    #[cynic(rename = "device_list")]
    pub device_list: Vec<DeviceWithRearPorts>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = DeviceIdVariables)]
pub struct QueryDeviceWithPort {
    #[arguments(id: $device_id)]
    #[cynic(rename = "device")]
    pub device: Option<DeviceWithRearPorts>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables=RearPortIdVariables)]
pub struct QueryRearPort {
    #[arguments(id: $rear_port_id)]
    #[cynic(rename = "rear_port")]
    pub rear_port: Option<RearPort>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = DeviceFilterVariables)]
pub struct CurrentCircuitData {
    #[cynic(rename = "circuit_list")]
    pub circuit_list: Vec<CircuitType>,
    #[arguments(filters: { rear_ports: { type: { in_list: $types } } })]
    #[cynic(rename = "device_list")]
    pub device_list: Vec<DeviceWithRearPorts>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type="DeviceType", variables = DeviceFilterVariables)]
pub struct DeviceWithRearPorts {
    pub id: NumberId,
    pub location: Option<LocationType>,
    pub name: Option<String>,
    #[cynic(rename = "device_type")]
    pub device_type: DeviceTypeType,
    #[arguments(filters: { type: { in_list: $types } })]
    pub rearports: Vec<RearPort>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "DeviceType")]
pub struct DeviceWithId {
    pub id: NumberId,
}
#[Object]
impl DeviceWithRearPorts {
    async fn id(&self) -> u32 {
        self.id.into()
    }
    async fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    async fn device_type(&self) -> &str {
        self.device_type.display.as_str()
    }
    async fn location_name(&self) -> Option<&str> {
        self.location.as_ref().map(|l| l.name.as_str())
    }
    async fn rear_ports(&self) -> &[RearPort] {
        self.rearports.as_slice()
    }
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "RearPortType")]
pub struct RearPort {
    pub id: NumberId,
    pub name: String,
    #[cynic(rename = "cable_connector")]
    pub cable_connector: Option<i32>,
    pub device: DeviceWithId,
}

impl RearPort {
    pub async fn fetch_by_id(rear_port_id: NumberId) -> Result<Option<RearPort>, BackendError> {
        Ok(
            query::<QueryRearPort, _>(RearPortIdVariables { rear_port_id })
                .await?
                .rear_port,
        )
    }
}

#[Object]
impl RearPort {
    async fn id(&self) -> u32 {
        self.id.into()
    }
    async fn name(&self) -> &str {
        self.name.as_str()
    }
    async fn fiber_ports(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PanelPort>> {
        PanelPort::rear_port_from_netbox(self.id.into(), ctx).await
    }
    async fn device(&self) -> async_graphql::Result<DeviceWithRearPorts> {
        let id = self.device.id;
        Ok(fetch_device_with_ports(id)
            .await?
            .ok_or_else(|| async_graphql::Error::new(format!("Device for id {id} not found")))?)
    }
}

#[derive(cynic::QueryFragment, Debug)]
pub struct DeviceTypeType {
    pub display: String,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct LocationType {
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct CircuitType {
    pub id: NumberId,
    pub display: String,
    pub terminations: Vec<CircuitTerminationType>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct CircuitTerminationType {
    pub __typename: String,
    #[cynic(rename = "link_peers")]
    pub link_peers: Vec<LinkPeerType>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "RearPortType")]
pub struct RearPortType2 {
    pub id: NumberId,
}

#[derive(cynic::InlineFragments, Debug)]
pub enum LinkPeerType {
    RearPortType2(RearPortType2),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
pub enum PortTypeEnum {
    #[cynic(rename = "TYPE_110_PUNCH")]
    Type110Punch,
    #[cynic(rename = "TYPE_4P2C")]
    Type4P2C,
    #[cynic(rename = "TYPE_4P4C")]
    Type4P4C,
    #[cynic(rename = "TYPE_6P2C")]
    Type6P2C,
    #[cynic(rename = "TYPE_6P4C")]
    Type6P4C,
    #[cynic(rename = "TYPE_6P6C")]
    Type6P6C,
    #[cynic(rename = "TYPE_8P2C")]
    Type8P2C,
    #[cynic(rename = "TYPE_8P4C")]
    Type8P4C,
    #[cynic(rename = "TYPE_8P6C")]
    Type8P6C,
    #[cynic(rename = "TYPE_8P8C")]
    Type8P8C,
    TypeBnc,
    TypeCs,
    TypeF,
    TypeFc,
    TypeFcApc,
    TypeFcPc,
    TypeFcUpc,
    TypeGg45,
    TypeLc,
    TypeLcApc,
    TypeLcPc,
    TypeLcUpc,
    TypeLsh,
    TypeLshApc,
    TypeLshPc,
    TypeLshUpc,
    TypeLx5,
    TypeLx5Apc,
    TypeLx5Pc,
    TypeLx5Upc,
    TypeMdc,
    TypeMpo,
    TypeMrj21,
    TypeMtrj,
    TypeMu,
    TypeMuApc,
    TypeMuPc,
    TypeMuUpc,
    TypeN,
    TypeOther,
    TypeSc,
    TypeScApc,
    TypeScPc,
    TypeScUpc,
    #[cynic(rename = "TYPE_SMA_905")]
    TypeSma905,
    #[cynic(rename = "TYPE_SMA_906")]
    TypeSma906,
    TypeSn,
    TypeSplice,
    TypeSt,
    #[cynic(rename = "TYPE_TERA_1P")]
    TypeTera1P,
    #[cynic(rename = "TYPE_TERA_2P")]
    TypeTera2P,
    #[cynic(rename = "TYPE_TERA_4P")]
    TypeTera4P,
    TypeUrmP2,
    TypeUrmP4,
    TypeUrmP8,
    TypeUsbA,
    TypeUsbB,
    TypeUsbC,
    TypeUsbMicroA,
    TypeUsbMicroAb,
    TypeUsbMicroB,
    TypeUsbMiniA,
    TypeUsbMiniB,
}
