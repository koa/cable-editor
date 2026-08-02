use crate::error::FrontendError;
use crate::graphql::authenticated::schema;
use crate::graphql::authenticated::schema::CreatePlan;
use crate::graphql::{mutate, query};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct ListPlanQuery {
    pub list_plan: Vec<PlanListEntry>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Plan")]
pub struct PlanListEntry {
    pub id: i32,
    pub name: String,
    pub status: PlanStatus,
}
#[derive(cynic::Enum, Debug, Clone, Copy, PartialEq, Hash, Ord, PartialOrd, Eq)]
#[cynic(graphql_type = "PlanStatusType")]
pub enum PlanStatus {
    IMPLEMENTED,
    OPEN,
    REJECTED,
}

impl PlanStatus {
    pub fn name(self) -> &'static str {
        match self {
            PlanStatus::IMPLEMENTED => "Implemented",
            PlanStatus::OPEN => "Open",
            PlanStatus::REJECTED => "Rejected",
        }
    }
}

#[derive(cynic::QueryVariables)]
struct CreatePlanMutationVariables {
    data: CreatePlanInput,
}
#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "CreatePlan")] // CreatePlan
struct CreatePlanInput {
    name: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CreatePlanMutationVariables")]
struct CreatePlanMutation {
    #[arguments( plan: $data)]
    pub create_plan: bool,
}

impl PlanListEntry {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
    ) -> Result<Box<[PlanListEntry]>, FrontendError> {
        Ok(query::<ListPlanQuery, _>((), credentials)
            .await?
            .data
            .map(|l| l.list_plan)
            .unwrap_or_default()
            .into_boxed_slice())
    }
    pub async fn create(
        credentials: Option<&OAuth2Context>,
        name: String,
    ) -> Result<(), FrontendError> {
        mutate::<CreatePlanMutation, _>(
            CreatePlanMutationVariables {
                data: CreatePlanInput { name },
            },
            credentials,
        )
        .await?;
        Ok(())
    }
}
