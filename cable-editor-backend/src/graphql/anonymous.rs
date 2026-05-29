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
    token_url: String,
    auth_url: String,
}

#[Object]
impl Query {
    /// gives the coordinates for authentication
    async fn authentication(&self) -> AuthenticationData {
        AuthenticationData {
            client_id: CONFIG.auth_client_id(),
            auth_url: CONFIG.auth_url(),
            token_url: CONFIG.auth_token_url(),
        }
    }
}
