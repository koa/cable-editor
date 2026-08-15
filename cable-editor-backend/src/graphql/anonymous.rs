use crate::config::CONFIG;
use async_graphql::{EmptyMutation, EmptySubscription, Object, Schema, SimpleObject};

pub struct Query;

pub type AnonymousGraphqlSchema = Schema<Query, EmptyMutation, EmptySubscription>;
pub fn create_anonymous_schema() -> AnonymousGraphqlSchema {
    Schema::build(Query, EmptyMutation, EmptySubscription).finish()
}
#[derive(SimpleObject)]
struct AuthenticationData {
    client_id: &'static str,
    issuer_url: &'static str,
}

#[Object]
impl Query {
    /// gives the coordinates for authentication
    async fn authentication(&self) -> AuthenticationData {
        AuthenticationData {
            client_id: CONFIG.auth_client_id(),
            issuer_url: CONFIG.auth_issuer(),
        }
    }
}
