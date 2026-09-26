use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{ParentChainPanel, PortSide, schema},
        mutate, query,
    },
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables)]
pub struct Variables {
    pub id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
pub struct FetchPlanDetailsQuery {
    #[arguments(planId: $id)]
    pub plan: Option<PlanDetails>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Plan")]
pub struct PlanDetails {
    pub id: i32,
    pub name: String,
    pub is_baseline: bool,
    pub usage: Vec<PortUsage>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "PortUsage")]
pub struct PortUsage {
    pub side: PortSide,
    pub fiber: Option<FiberDetails>,
    pub port: Port,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Fiber")]
pub struct FiberDetails {
    pub bundle: i32,
    pub fiber: i32,
    pub cable: CableDetails,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Cable")]
pub struct CableDetails {
    pub id: i32,
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "PanelPort")]
pub struct Port {
    pub id: i32,
    pub label: Option<String>,
    pub panel: PortPanel,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Panel")]
pub struct PortPanel {
    pub id: i32,
    pub schacht: Schacht,
    pub name: Option<String>,
    pub parent_chain: Vec<ParentChainPanel>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct Schacht {
    pub id: i32,
    pub name: String,
}

// --- Mutations für EditPlan ---

#[derive(cynic::QueryVariables)]
pub struct UpdatePlanVariables {
    pub plan_id: i32,
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "UpdatePlanVariables")]
pub struct UpdatePlanMutation {
    #[arguments(planId: $plan_id, name: $name)]
    pub update_plan: PlanDetails,
}

#[derive(cynic::QueryVariables)]
pub struct ImplementPlanVariables {
    pub plan_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "ImplementPlanVariables")]
pub struct ImplementPlanMutation {
    #[arguments(planId: $plan_id)]
    pub implement_plan: PlanDetails,
}

impl PlanDetails {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        id: i32,
    ) -> Result<Option<PlanDetails>, FrontendError> {
        Ok(
            query::<FetchPlanDetailsQuery, _>(Variables { id }, credentials)
                .await?
                .plan,
        )
    }

    pub async fn update_name(
        credentials: Option<&OAuth2Context>,
        plan_id: i32,
        name: String,
    ) -> Result<PlanDetails, FrontendError> {
        Ok(
            mutate::<UpdatePlanMutation, _>(UpdatePlanVariables { plan_id, name }, credentials)
                .await?
                .update_plan,
        )
    }

    pub async fn implement(
        credentials: Option<&OAuth2Context>,
        plan_id: i32,
    ) -> Result<PlanDetails, FrontendError> {
        Ok(
            mutate::<ImplementPlanMutation, _>(ImplementPlanVariables { plan_id }, credentials)
                .await?
                .implement_plan,
        )
    }
}
