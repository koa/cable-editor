//! Owners of Schächte and ducts; the Leitungskataster is delivered per owner (see
//! docs/leitungskataster.md).

use crate::db::schema;
use async_graphql::Object;
use diesel::{HasQuery, Identifiable};

#[derive(Identifiable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schema::eigentuemer)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Eigentuemer {
    pub id: i32,
    pub name: String,
    pub lk_name: Option<String>,
    pub uid: Option<String>,
    pub standard: bool,
}

impl Eigentuemer {
    /// `Eigentuemer` in the delivery: the released name, else the name.
    pub fn delivered_name(&self) -> &str {
        self.lk_name.as_deref().unwrap_or(&self.name)
    }
}

#[Object(name = "Owner")]
impl Eigentuemer {
    async fn id(&self) -> i32 {
        self.id
    }
    /// The real name
    async fn name(&self) -> &str {
        &self.name
    }
    /// Name in the delivery to the Leitungskataster if not the name, `Keine_Angabe` if it
    /// isn't released
    async fn lk_name(&self) -> Option<&str> {
        self.lk_name.as_deref()
    }
    /// UID (`CHE-…`, fictitious `ZHE-…`), the Datenherr; not delivered without one
    async fn uid(&self) -> Option<&str> {
        self.uid.as_deref()
    }
    /// Owner of new Schächte and ducts
    async fn is_default(&self) -> bool {
        self.standard
    }
}
