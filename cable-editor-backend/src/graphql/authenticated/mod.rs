use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};

pub type AuthenticatedGraphqlSchema = Schema<AuthenticatedQuery, EmptyMutation, EmptySubscription>;

pub struct AuthenticatedQuery;

#[Object]
impl AuthenticatedQuery {
    /// Returns the sum of a and b
    async fn add(&self, ctx: &Context<'_>, a: i32, b: i32) -> async_graphql::Result<i32> {
        Ok(a + b)
    }
}

pub fn create_authenticated_schema() -> AuthenticatedGraphqlSchema {
    Schema::build(AuthenticatedQuery, EmptyMutation, EmptySubscription).finish()
}
