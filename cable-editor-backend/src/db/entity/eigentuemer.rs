//! Owners of Schächte and ducts, named as `Eigentuemer` in the delivery to the
//! Leitungskataster (see docs/leitungskataster.md).

use crate::graphql::error::ApiResult;
use crate::{
    db::schema,
    graphql::loader::{EigentuemerCounts, OwnedCounts, get_loader},
};
use async_graphql::{Context, Object};
use diesel::{HasQuery, Identifiable};

#[derive(Identifiable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schema::eigentuemer)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Eigentuemer {
    pub id: i32,
    pub name: String,
    pub lk_name: Option<String>,
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
    /// Owner of new Schächte and ducts
    async fn is_default(&self) -> bool {
        self.standard
    }
    async fn schacht_count(&self, ctx: &Context<'_>) -> ApiResult<i32> {
        Ok(self.counts(ctx).await?.schaechte)
    }
    async fn duct_count(&self, ctx: &Context<'_>) -> ApiResult<i32> {
        Ok(self.counts(ctx).await?.ducts)
    }
    /// Ducts delivered to the Leitungskataster
    async fn delivered_duct_count(&self, ctx: &Context<'_>) -> ApiResult<i32> {
        Ok(self.counts(ctx).await?.delivered_ducts)
    }
}

impl Eigentuemer {
    async fn counts(&self, ctx: &Context<'_>) -> ApiResult<OwnedCounts> {
        Ok(get_loader(ctx)?
            .load_one(EigentuemerCounts(self.id))
            .await?
            .unwrap_or_default())
    }
}
