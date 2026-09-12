use crate::config::NETBOX_CONFIG;
use crate::error::BackendError;
use crate::netbox::fetch::{
    DeviceFilterVariables, DeviceIdVariables, PortTypeEnum, QueryDeviceWithPort,
};
use crate::netbox::fetch::{DeviceWithRearPorts, QueryDevicesAndPorts};
use crate::netbox::id::NumberId;
use cynic::http::ReqwestExt;
use cynic::{QueryBuilder as CQB, QueryFragment};
use log::info;
use reqwest::header::{AUTHORIZATION, HeaderMap};
use std::collections::BTreeMap;

pub mod fetch;
#[cynic::schema("netbox")]
mod schema {}

pub mod id;

pub async fn fetch_devices_and_ports() -> Result<Vec<DeviceWithRearPorts>, BackendError> {
    let netbox_data = query::<QueryDevicesAndPorts, _>(DeviceFilterVariables {
        types: Some(vec![PortTypeEnum::TypeLc, PortTypeEnum::TypeLcUpc]),
    })
    .await?;
    //let config = ClientConfig::new(NETBOX_CONFIG.url(), NETBOX_CONFIG.token());

    Ok(netbox_data
        .device_list
        .into_iter()
        .map(|e| (e.id, e))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect())
}
pub async fn fetch_device_with_ports(
    device_id: NumberId,
) -> Result<Option<DeviceWithRearPorts>, BackendError> {
    Ok(query::<QueryDeviceWithPort, _>(DeviceIdVariables {
        device_id,
        types: Some(vec![PortTypeEnum::TypeLc, PortTypeEnum::TypeLcUpc]),
    })
    .await?
    .device)
}

async fn reqwest_client() -> Result<reqwest::Client, BackendError> {
    let access_token = NETBOX_CONFIG.token();
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        format!("Bearer {access_token}")
            .parse()
            .map_err(BackendError::CreateNetboxCynicClientHeaderError)?,
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(BackendError::CreateNetboxCynicClientError)
}

pub async fn query<Q, V>(request: V) -> Result<Q, BackendError>
where
    Q: QueryFragment + serde::de::DeserializeOwned + 'static,
    Q::SchemaType: cynic::schema::QueryRoot,
    V: cynic::QueryVariables<Fields = Q::VariablesFields> + serde::Serialize,
{
    let response = reqwest_client()
        .await?
        .post(NETBOX_CONFIG.url())
        .run_graphql(Q::build(request))
        .await
        .map_err(|error| BackendError::ErrorCallingNetboxBackend {
            query: Q::name(),
            error,
        })?;
    if let Some(errors) = response.errors {
        Err(BackendError::NetboxGraphqlError {
            query: Q::name(),
            errors,
        })
    } else if let Some(data) = response.data {
        Ok(data)
    } else {
        Err(BackendError::NetboxCynicEmptyResponseError { query: Q::name() })
    }
}
