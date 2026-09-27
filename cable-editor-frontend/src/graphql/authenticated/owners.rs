//! Owners of Schächte and ducts (see docs/stammdaten.md).

use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, mutate, query},
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct ListOwnerQuery {
    list_owner: Vec<OwnerListEntry>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Owner")]
pub struct OwnerListEntry {
    pub id: i32,
    pub name: String,
    /// Name in the delivery to the Leitungskataster if not the name
    pub lk_name: Option<String>,
    pub uid: Option<String>,
    pub is_default: bool,
    pub schacht_count: i32,
    pub duct_count: i32,
    /// Ducts delivered to the Leitungskataster
    pub delivered_duct_count: i32,
}

/// `Eigentuemer` in the delivery when the name isn't released (SIA405 LKMap).
pub const NAME_NOT_RELEASED: &str = "Keine_Angabe";

impl OwnerListEntry {
    /// Ducts that would be delivered, but can't be without a UID.
    pub fn undeliverable_ducts(&self) -> i32 {
        if self.uid.is_none() {
            self.delivered_duct_count
        } else {
            0
        }
    }
    /// The values the dialog edits.
    pub fn input(&self) -> OwnerInput {
        OwnerInput {
            name: self.name.clone(),
            lk_name: self.lk_name.clone(),
            uid: self.uid.clone(),
        }
    }
}

/// By name, as the backend lists them.
pub async fn fetch_owner_list(
    credentials: Option<&OAuth2Context>,
) -> Result<Box<[OwnerListEntry]>, FrontendError> {
    Ok(query::<ListOwnerQuery, _>((), credentials)
        .await?
        .list_owner
        .into_boxed_slice())
}

#[derive(cynic::InputObject, Debug, Clone, PartialEq, Default)]
pub struct OwnerInput {
    pub name: String,
    pub lk_name: Option<String>,
    pub uid: Option<String>,
}

#[derive(cynic::QueryFragment, Debug, Clone, Copy)]
#[cynic(graphql_type = "Owner")]
struct OwnerId {
    #[allow(unused)]
    id: i32,
}

#[derive(cynic::QueryVariables)]
struct CreateOwnerVariables {
    owner: OwnerInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CreateOwnerVariables")]
struct CreateOwnerMutation {
    #[arguments(owner: $owner)]
    #[allow(unused)]
    create_owner: OwnerId,
}

pub async fn create_owner(
    credentials: Option<&OAuth2Context>,
    owner: OwnerInput,
) -> Result<(), FrontendError> {
    mutate::<CreateOwnerMutation, _>(CreateOwnerVariables { owner }, credentials).await?;
    Ok(())
}

#[derive(cynic::QueryVariables)]
struct UpdateOwnerVariables {
    owner_id: i32,
    owner: OwnerInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "UpdateOwnerVariables")]
struct UpdateOwnerMutation {
    #[arguments(ownerId: $owner_id, owner: $owner)]
    #[allow(unused)]
    update_owner: OwnerId,
}

pub async fn update_owner(
    credentials: Option<&OAuth2Context>,
    owner_id: i32,
    owner: OwnerInput,
) -> Result<(), FrontendError> {
    mutate::<UpdateOwnerMutation, _>(UpdateOwnerVariables { owner_id, owner }, credentials).await?;
    Ok(())
}

#[derive(cynic::QueryVariables)]
struct OwnerIdVariables {
    owner_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "OwnerIdVariables")]
struct SetDefaultOwnerMutation {
    #[arguments(ownerId: $owner_id)]
    #[allow(unused)]
    set_default_owner: OwnerId,
}

pub async fn set_default_owner(
    credentials: Option<&OAuth2Context>,
    owner_id: i32,
) -> Result<(), FrontendError> {
    mutate::<SetDefaultOwnerMutation, _>(OwnerIdVariables { owner_id }, credentials).await?;
    Ok(())
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "OwnerIdVariables")]
struct DeleteOwnerMutation {
    #[arguments(ownerId: $owner_id)]
    #[allow(unused)]
    delete_owner: bool,
}

pub async fn delete_owner(
    credentials: Option<&OAuth2Context>,
    owner_id: i32,
) -> Result<(), FrontendError> {
    mutate::<DeleteOwnerMutation, _>(OwnerIdVariables { owner_id }, credentials).await?;
    Ok(())
}
