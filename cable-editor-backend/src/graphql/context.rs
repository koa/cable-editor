use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Eq, Hash, Ord, PartialOrd)]
pub struct User {
    pub id: Box<str>,
}

/// OIDC userinfo claims. Providers omit some depending on scopes (e.g. Pocket ID `groups`).
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, SimpleObject)]
pub struct UserInfo {
    #[serde(default)]
    pub display_name: Box<str>,
    #[serde(default)]
    pub groups: Box<[Box<str>]>,
    pub preferred_username: Box<str>,
    #[serde(default)]
    pub picture: Box<str>,
}
