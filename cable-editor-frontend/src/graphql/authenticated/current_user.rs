use crate::error::FrontendError;
use crate::graphql::authenticated::schema;
use crate::graphql::query;
use yew_oauth2::context::OAuth2Context;

/// What the user may do, each role includes the ones before it. Only decides what the frontend
/// shows, the backend checks every change itself.
#[derive(cynic::Enum, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cynic(graphql_type = "Role")]
pub enum Role {
    Reader,
    Planner,
    Admin,
}

impl Role {
    pub fn title(self) -> &'static str {
        match self {
            Role::Reader => "Leser",
            Role::Planner => "Planer",
            Role::Admin => "Admin",
        }
    }
    /// What the role allows, for users asking why they can't do something
    pub fn description(self) -> &'static str {
        match self {
            Role::Reader => "Lesen und Etiketten drucken",
            Role::Planner => "Planen und Kabel, Panels und Ports ändern",
            Role::Admin => "Planungen umsetzen, nach Netbox übertragen und Kabel löschen",
        }
    }
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct CurrentUserQuery {
    current_user: CurrentUser,
}

/// The logged in user as the backend sees it: the OIDC userinfo and the role derived from its
/// groups, shown in the user menu for support.
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "UserInfo")]
pub struct CurrentUser {
    pub display_name: String,
    pub preferred_username: String,
    pub groups: Vec<String>,
    pub role: Role,
}

impl CurrentUser {
    pub async fn fetch(credentials: Option<&OAuth2Context>) -> Result<CurrentUser, FrontendError> {
        Ok(query::<CurrentUserQuery, _>((), credentials)
            .await?
            .current_user)
    }
}
