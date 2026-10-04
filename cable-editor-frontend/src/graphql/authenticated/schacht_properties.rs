use crate::graphql::authenticated::{OwnerId, SchachtId, SchachtTypRef};
use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{Genauigkeit, GeoPoint, Lv95Point, duct_properties::OwnerChoice, schema},
        mutate, query,
    },
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables)]
struct SchachtPropertiesVariables {
    schacht_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "SchachtPropertiesVariables")]
struct SchachtPropertiesQuery {
    #[arguments(schachtId: $schacht_id)]
    schacht: Option<SchachtProperties>,
    list_schacht_typ: Vec<SchachtTypRef>,
    list_owner: Vec<OwnerChoice>,
}

/// Name, type, position, owner and Lagebestimmung of a Schacht, as the properties page edits
/// them.
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct SchachtProperties {
    pub id: i32,
    pub name: String,
    pub typ: Option<SchachtTypeRef>,
    /// LV95, as stored
    pub position: Option<Lv95Point>,
    pub location: Option<GeoPoint>,
    pub owner: OwnerId,
    pub lagebestimmung: Genauigkeit,
}

/// What a Schacht's properties choose from.
#[derive(Debug, Clone, PartialEq)]
pub struct SchachtChoices {
    pub types: Box<[SchachtTypRef]>,
    pub owners: Box<[OwnerChoice]>,
}

#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq)]
#[cynic(graphql_type = "SchachtTyp")]
pub struct SchachtTypeRef {
    pub id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct SchachtChoicesQuery {
    list_schacht_typ: Vec<SchachtTypRef>,
    list_owner: Vec<OwnerChoice>,
}

/// The types and owners to choose from, for a new Schacht.
pub async fn fetch_schacht_choices(
    credentials: Option<&OAuth2Context>,
) -> Result<SchachtChoices, FrontendError> {
    let result = query::<SchachtChoicesQuery, _>((), credentials).await?;
    Ok(SchachtChoices {
        types: result.list_schacht_typ.into(),
        owners: result.list_owner.into(),
    })
}

/// The Schacht (missing if it doesn't exist) and the types and owners to choose from.
pub async fn fetch_schacht_properties(
    credentials: Option<&OAuth2Context>,
    schacht_id: i32,
) -> Result<(Option<SchachtProperties>, SchachtChoices), FrontendError> {
    let result =
        query::<SchachtPropertiesQuery, _>(SchachtPropertiesVariables { schacht_id }, credentials)
            .await?;
    let choices = SchachtChoices {
        types: result.list_schacht_typ.into(),
        owners: result.list_owner.into(),
    };
    Ok((result.schacht, choices))
}

#[derive(cynic::InputObject, Debug, Clone, Copy, PartialEq)]
pub struct Lv95Input {
    pub e: f64,
    pub n: f64,
}

#[derive(cynic::InputObject, Debug, Clone, Copy, PartialEq)]
pub struct GeoPointInput {
    pub lat: f64,
    pub lng: f64,
}

#[derive(cynic::InputObject, Debug, Clone, Copy, PartialEq)]
pub enum PositionInput {
    Lv95(Lv95Input),
    Wgs84(GeoPointInput),
}

#[derive(cynic::InputObject, Debug, Clone, PartialEq)]
pub struct SchachtInput {
    pub name: String,
    pub type_id: Option<i32>,
    pub position: Option<PositionInput>,
    pub owner_id: i32,
    pub lagebestimmung: Genauigkeit,
}

/// A position in both systems.
#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq)]
pub struct ConvertedPoint {
    pub lv95: Lv95Point,
    pub wgs84: GeoPoint,
}

#[derive(cynic::QueryVariables)]
struct ConvertPointVariables {
    position: PositionInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ConvertPointVariables")]
struct ConvertPointQuery {
    #[arguments(position: $position)]
    convert_point: ConvertedPoint,
}

/// The position in LV95 and WGS84 (PostGIS converts); refused outside of Switzerland.
pub async fn convert_point(
    credentials: Option<&OAuth2Context>,
    position: PositionInput,
) -> Result<ConvertedPoint, FrontendError> {
    Ok(
        query::<ConvertPointQuery, _>(ConvertPointVariables { position }, credentials)
            .await?
            .convert_point,
    )
}

#[derive(cynic::QueryVariables)]
struct CreateSchachtVariables {
    schacht: SchachtInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CreateSchachtVariables")]
struct CreateSchachtMutation {
    #[arguments(schacht: $schacht)]
    create_schacht: SchachtId,
}

/// The id of the new Schacht.
pub async fn create_schacht(
    credentials: Option<&OAuth2Context>,
    schacht: SchachtInput,
) -> Result<i32, FrontendError> {
    Ok(
        mutate::<CreateSchachtMutation, _>(CreateSchachtVariables { schacht }, credentials)
            .await?
            .create_schacht
            .id,
    )
}

#[derive(cynic::QueryVariables)]
struct UpdateSchachtVariables {
    schacht_id: i32,
    schacht: SchachtInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "UpdateSchachtVariables")]
struct UpdateSchachtMutation {
    #[arguments(schachtId: $schacht_id, schacht: $schacht)]
    update_schacht: SchachtProperties,
}

/// The Schacht as stored.
pub async fn update_schacht(
    credentials: Option<&OAuth2Context>,
    schacht_id: i32,
    schacht: SchachtInput,
) -> Result<SchachtProperties, FrontendError> {
    Ok(mutate::<UpdateSchachtMutation, _>(
        UpdateSchachtVariables {
            schacht_id,
            schacht,
        },
        credentials,
    )
    .await?
    .update_schacht)
}

#[derive(cynic::QueryVariables)]
struct DeleteSchachtVariables {
    schacht_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "DeleteSchachtVariables")]
struct DeleteSchachtMutation {
    #[arguments(schachtId: $schacht_id)]
    #[allow(unused)]
    delete_schacht: bool,
}

pub async fn delete_schacht(
    credentials: Option<&OAuth2Context>,
    schacht_id: i32,
) -> Result<(), FrontendError> {
    mutate::<DeleteSchachtMutation, _>(DeleteSchachtVariables { schacht_id }, credentials).await?;
    Ok(())
}
