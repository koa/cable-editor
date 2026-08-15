use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Eq, Hash, Ord, PartialOrd)]
pub struct User {
    pub id: Box<str>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, SimpleObject)]
pub struct UserInfo {
    pub display_name: Box<str>,
    pub groups: Box<[Box<str>]>,
    pub preferred_username: Box<str>,
    pub picture: Box<str>,
}
