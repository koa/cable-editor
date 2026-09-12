use crate::error::FrontendError;
use crate::graphql::authenticated::plan_details::{
    ImplementPlanMutation, ImplementPlanVariables, PlanDetails,
};
use crate::graphql::authenticated::schema;
use crate::graphql::mutate;
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables)]
pub struct SyncNetboxVariables {
    pub plan_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = SyncNetboxVariables)]
pub struct SyncNetbox {
    #[arguments(planId: $plan_id)]
    pub sync_plan_to_netbox: PlanDummy,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Plan")]
pub struct PlanDummy {
    pub id: i32,
}

impl SyncNetbox {
    pub async fn sync_netbox(
        credentials: Option<&OAuth2Context>,
        plan_id: i32,
    ) -> Result<PlanDummy, FrontendError> {
        let response =
            mutate::<SyncNetbox, _>(SyncNetboxVariables { plan_id }, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            response
                .data
                .map(|d| d.sync_plan_to_netbox)
                .ok_or(FrontendError::NotFound)
        }
    }
}
