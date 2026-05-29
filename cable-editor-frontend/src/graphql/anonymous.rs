#[cynic::schema("anonymous")]
mod schema {}

#[derive(cynic::QueryFragment, Debug)]
pub struct AuthenticationData {
    client_id: String,
    token_url: String,
    auth_url: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
pub struct AuthenticationQuery {
    authentication: AuthenticationData,
}
