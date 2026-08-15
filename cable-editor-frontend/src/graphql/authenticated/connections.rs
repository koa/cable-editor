use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, query},
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Cable")]
pub struct CableInfo {
    pub id: i32,
    pub name: String,
    pub bundle_count: i32,
    pub fiber_count: i32,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "CableEnd")]
pub struct CableEndInfo {
    pub cable: CableInfo,
    pub path: CablePathInfo,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "CablePath")]
pub struct CablePathInfo {
    pub far_schacht: FarSchachtInfo,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Schacht")]
pub struct FarSchachtInfo {
    pub name: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
struct ListCablesByCabinetQuery {
    #[arguments(schachtId: $id)]
    pub schacht: Option<SchachtData>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
struct ListCablesByPanelQuery {
    #[arguments(panelId: $id)]
    pub panel: Option<PanelData>,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Panel")]
struct PanelData {
    pub schacht: SchachtData,
}

#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Schacht")]
struct SchachtData {
    pub cables: Vec<CableEndInfo>,
}

#[derive(cynic::QueryVariables)]
struct Variables {
    id: i32,
}

impl CableEndInfo {
    pub async fn list_by_cabinet(
        credentials: Option<&OAuth2Context>,
        id: i32,
    ) -> Result<Vec<CableEndInfo>, FrontendError> {
        let response = query::<ListCablesByCabinetQuery, _>(Variables { id }, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            Ok(response
                .data
                .and_then(|l| l.schacht)
                .map(|cabinet| cabinet.cables)
                .unwrap_or_default())
        }
    }
    pub async fn list_candidate_by_panel(
        credentials: Option<&OAuth2Context>,
        id: i32,
    ) -> Result<Vec<CableEndInfo>, FrontendError> {
        let response = query::<ListCablesByPanelQuery, _>(Variables { id }, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            Ok(response
                .data
                .and_then(|p| p.panel)
                .map(|l| l.schacht)
                .map(|cabinet| cabinet.cables)
                .unwrap_or_default())
        }
    }
}
