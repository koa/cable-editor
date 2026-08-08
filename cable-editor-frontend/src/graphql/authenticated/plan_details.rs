use crate::error::FrontendError;
use crate::graphql::authenticated::schema;
use crate::graphql::query;
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables)]
struct Variables {
    id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
struct FetchPlanDetailsQuery {
    #[arguments(planId: $id)]
    pub plan: Option<PlanDetails>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Plan")]
pub struct PlanDetails {
    pub id: i32,
    pub name: String,
}

impl PlanDetails {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        id: i32,
    ) -> Result<Option<PlanDetails>, FrontendError> {
        let response = query::<FetchPlanDetailsQuery, _>(Variables { id }, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            Ok(response.data.and_then(|l| l.plan))
        }
    }
}
