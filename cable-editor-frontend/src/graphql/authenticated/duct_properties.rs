use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{GeoPoint, schema},
        mutate, query,
    },
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables)]
struct DuctPropertiesVariables {
    duct_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "DuctPropertiesVariables")]
struct DuctPropertiesQuery {
    #[arguments(ductId: $duct_id)]
    duct: Option<DuctProperties>,
    list_schacht: Vec<SchachtChoice>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct SchachtChoicesQuery {
    list_schacht: Vec<SchachtChoice>,
}

/// Schächte, description and course of a duct, as its properties page edits them.
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Duct")]
pub struct DuctProperties {
    pub id: i32,
    pub description: Option<String>,
    pub schacht_a: DuctPropertiesEnd,
    pub schacht_z: DuctPropertiesEnd,
    /// From Schacht A to Schacht Z, missing without geometry
    pub line: Option<Vec<GeoPoint>>,
    /// Metres, missing without geometry
    pub length: Option<f64>,
    pub cables: Vec<DuctPropertiesCable>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct DuctPropertiesEnd {
    pub id: i32,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Cable")]
pub struct DuctPropertiesCable {
    pub id: i32,
}

/// A Schacht to choose as end of a duct.
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct SchachtChoice {
    pub id: i32,
    pub name: String,
    pub location: Option<GeoPoint>,
}

/// The duct (missing if it doesn't exist; not asked for a new one) and the Schächte to choose.
pub async fn fetch_duct_properties(
    credentials: Option<&OAuth2Context>,
    duct_id: Option<i32>,
) -> Result<(Option<DuctProperties>, Vec<SchachtChoice>), FrontendError> {
    match duct_id {
        Some(duct_id) => {
            let result =
                query::<DuctPropertiesQuery, _>(DuctPropertiesVariables { duct_id }, credentials)
                    .await?;
            Ok((result.duct, result.list_schacht))
        }
        None => Ok((
            None,
            query::<SchachtChoicesQuery, _>((), credentials)
                .await?
                .list_schacht,
        )),
    }
}

#[derive(cynic::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateSystem {
    Lv95,
    Lv03,
    Wgs84,
}

#[derive(cynic::InputObject, Debug, Clone, Copy, PartialEq)]
pub struct CoordinateInput {
    pub x: f64,
    pub y: f64,
}

#[derive(cynic::InputObject, Debug, Clone, PartialEq)]
pub struct LineInput {
    pub system: CoordinateSystem,
    pub points: Vec<CoordinateInput>,
}

#[derive(cynic::InputObject, Debug, Clone, PartialEq)]
pub struct DuctInput {
    pub schacht_a: i32,
    pub schacht_z: i32,
    pub description: Option<String>,
}

/// A course as it would be stored.
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
pub struct DuctLineCheck {
    /// From Schacht A to Schacht Z
    pub line: Vec<GeoPoint>,
    pub reversed: bool,
    pub removed_ends: i32,
    pub start_distance: f64,
    pub end_distance: f64,
    pub length: f64,
    pub needs_confirmation: bool,
}

#[derive(cynic::QueryVariables)]
struct CheckDuctLineVariables {
    schacht_a: i32,
    schacht_z: i32,
    line: LineInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "CheckDuctLineVariables")]
struct CheckDuctLineQuery {
    #[arguments(schachtA: $schacht_a, schachtZ: $schacht_z, line: $line)]
    check_duct_line: DuctLineCheck,
}

pub async fn check_duct_line(
    credentials: Option<&OAuth2Context>,
    schacht_a: i32,
    schacht_z: i32,
    line: LineInput,
) -> Result<DuctLineCheck, FrontendError> {
    Ok(query::<CheckDuctLineQuery, _>(
        CheckDuctLineVariables {
            schacht_a,
            schacht_z,
            line,
        },
        credentials,
    )
    .await?
    .check_duct_line)
}

#[derive(cynic::QueryFragment, Debug, Clone, Copy)]
#[cynic(graphql_type = "Duct")]
pub struct DuctId {
    pub id: i32,
}

#[derive(cynic::QueryVariables)]
struct CreateDuctVariables {
    duct: DuctInput,
    line: Option<LineInput>,
    confirmed: bool,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CreateDuctVariables")]
struct CreateDuctMutation {
    #[arguments(duct: $duct, line: $line, confirmed: $confirmed)]
    create_duct: DuctId,
}

/// The id of the new duct.
pub async fn create_duct(
    credentials: Option<&OAuth2Context>,
    duct: DuctInput,
    line: Option<LineInput>,
    confirmed: bool,
) -> Result<i32, FrontendError> {
    Ok(mutate::<CreateDuctMutation, _>(
        CreateDuctVariables {
            duct,
            line,
            confirmed,
        },
        credentials,
    )
    .await?
    .create_duct
    .id)
}

#[derive(cynic::QueryVariables)]
struct UpdateDuctVariables {
    duct_id: i32,
    duct: DuctInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "UpdateDuctVariables")]
struct UpdateDuctMutation {
    #[arguments(ductId: $duct_id, duct: $duct)]
    #[allow(unused)]
    update_duct: DuctId,
}

pub async fn update_duct(
    credentials: Option<&OAuth2Context>,
    duct_id: i32,
    duct: DuctInput,
) -> Result<(), FrontendError> {
    mutate::<UpdateDuctMutation, _>(UpdateDuctVariables { duct_id, duct }, credentials).await?;
    Ok(())
}

#[derive(cynic::QueryVariables)]
struct SetDuctLineVariables {
    duct_id: i32,
    line: Option<LineInput>,
    confirmed: bool,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SetDuctLineVariables")]
struct SetDuctLineMutation {
    #[arguments(ductId: $duct_id, line: $line, confirmed: $confirmed)]
    #[allow(unused)]
    set_duct_line: DuctId,
}

/// Stores the course; `None`: a straight line.
pub async fn set_duct_line(
    credentials: Option<&OAuth2Context>,
    duct_id: i32,
    line: Option<LineInput>,
    confirmed: bool,
) -> Result<(), FrontendError> {
    mutate::<SetDuctLineMutation, _>(
        SetDuctLineVariables {
            duct_id,
            line,
            confirmed,
        },
        credentials,
    )
    .await?;
    Ok(())
}

#[derive(cynic::QueryVariables)]
struct DeleteDuctVariables {
    duct_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "DeleteDuctVariables")]
struct DeleteDuctMutation {
    #[arguments(ductId: $duct_id)]
    #[allow(unused)]
    delete_duct: bool,
}

pub async fn delete_duct(
    credentials: Option<&OAuth2Context>,
    duct_id: i32,
) -> Result<(), FrontendError> {
    mutate::<DeleteDuctMutation, _>(DeleteDuctVariables { duct_id }, credentials).await?;
    Ok(())
}
