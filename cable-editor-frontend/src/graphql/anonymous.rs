use yew::html::IntoPropValue;
use yew_oauth2::oauth2::Config;

#[cynic::schema("anonymous")]
mod schema {}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
pub struct AuthenticationData {
    pub client_id: String,
    pub token_url: String,
    pub auth_url: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
pub struct AuthenticationQuery {
    pub authentication: AuthenticationData,
}
impl IntoPropValue<Config> for AuthenticationData {
    fn into_prop_value(self) -> Config {
        Config::new(self.client_id, self.auth_url, self.token_url)
    }
}