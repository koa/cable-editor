//! Owners of Schächte and ducts (see docs/stammdaten.md); Admin only, as the owner is the
//! Datenherr of the delivery to the Leitungskataster.

use crate::{
    config::is_uid,
    db::{entity::eigentuemer::Eigentuemer, schema},
    graphql::authenticated,
    graphql::authorization::{Role, RoleGuard},
};
use async_graphql::{Context, InputObject, Object};
use diesel::{ExpressionMethods, HasQuery, OptionalExtension, QueryDsl, SelectableHelper};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

#[derive(Default)]
pub struct OwnerMutation;

#[Object]
impl OwnerMutation {
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn create_owner(
        &self,
        ctx: &Context<'_>,
        owner: OwnerInput,
    ) -> async_graphql::Result<Eigentuemer> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let owner = owner.checked(&mut connection, None).await?;
        Ok(diesel::insert_into(schema::eigentuemer::table)
            .values((
                schema::eigentuemer::name.eq(owner.name),
                schema::eigentuemer::lk_name.eq(owner.lk_name),
                schema::eigentuemer::uid.eq(owner.uid),
            ))
            .returning(Eigentuemer::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Name, name in the delivery and UID; the owner's Schächte and ducts count as changed
    /// for the Leitungskataster.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn update_owner(
        &self,
        ctx: &Context<'_>,
        owner_id: i32,
        owner: OwnerInput,
    ) -> async_graphql::Result<Eigentuemer> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let owner = owner.checked(&mut connection, Some(owner_id)).await?;
        diesel::update(schema::eigentuemer::table.find(owner_id))
            .set((
                schema::eigentuemer::name.eq(owner.name),
                schema::eigentuemer::lk_name.eq(owner.lk_name),
                schema::eigentuemer::uid.eq(owner.uid),
            ))
            .returning(Eigentuemer::as_returning())
            .get_result(&mut connection)
            .await
            .optional()?
            .ok_or_else(|| format!("Eigentümer {owner_id} nicht gefunden").into())
    }
    /// The owner new Schächte and ducts get; there is always exactly one.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn set_default_owner(
        &self,
        ctx: &Context<'_>,
        owner_id: i32,
    ) -> async_graphql::Result<Eigentuemer> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let owner = find_owner(&mut connection, owner_id).await?;
        if owner.standard {
            return Ok(owner);
        }
        // The unique index allows only one standard owner at any time
        diesel::update(schema::eigentuemer::table.filter(schema::eigentuemer::standard))
            .set(schema::eigentuemer::standard.eq(false))
            .execute(&mut connection)
            .await?;
        Ok(diesel::update(schema::eigentuemer::table.find(owner_id))
            .set(schema::eigentuemer::standard.eq(true))
            .returning(Eigentuemer::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Only an owner without Schächte, ducts and deliveries, and not the default one.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn delete_owner(&self, ctx: &Context<'_>, owner_id: i32) -> async_graphql::Result<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let owner = find_owner(&mut connection, owner_id).await?;
        if owner.standard {
            return Err(
                "Der Standard-Eigentümer kann nicht gelöscht werden, zuerst einen anderen als Standard setzen"
                    .into(),
            );
        }
        let schaechte: i64 = schema::schacht::table
            .filter(schema::schacht::eigentuemer_id.eq(owner_id))
            .count()
            .get_result(&mut connection)
            .await?;
        let ducts: i64 = schema::trasse::table
            .filter(schema::trasse::eigentuemer_id.eq(owner_id))
            .count()
            .get_result(&mut connection)
            .await?;
        let deliveries: i64 = schema::lk_lieferung::table
            .filter(schema::lk_lieferung::eigentuemer_id.eq(owner_id))
            .count()
            .get_result(&mut connection)
            .await?;
        if schaechte > 0 || ducts > 0 {
            return Err(format!(
                "Dem Eigentümer gehören noch {schaechte} Schächte und {ducts} Trassen"
            )
            .into());
        }
        if deliveries > 0 {
            return Err(format!(
                "Für den Eigentümer sind {deliveries} Lieferungen an den Leitungskataster protokolliert"
            )
            .into());
        }
        let deleted = diesel::delete(schema::eigentuemer::table.find(owner_id))
            .execute(&mut connection)
            .await?;
        Ok(deleted > 0)
    }
}

async fn find_owner(
    connection: &mut AsyncPgConnection,
    owner_id: i32,
) -> async_graphql::Result<Eigentuemer> {
    Eigentuemer::query()
        .filter(schema::eigentuemer::id.eq(owner_id))
        .first(connection)
        .await
        .optional()?
        .ok_or_else(|| format!("Eigentümer {owner_id} nicht gefunden").into())
}

/// Name, name in the delivery and UID of an owner.
#[derive(Debug, Clone, PartialEq, InputObject)]
struct OwnerInput {
    name: String,
    /// Name in the delivery to the Leitungskataster if not the name, `Keine_Angabe` if it
    /// isn't released; empty: the name
    lk_name: Option<String>,
    /// `CHE-123.456.789`, fictitious `ZHE-…`; empty: none (not delivered)
    uid: Option<String>,
}

/// `OwnerInput` checked, empty values as `None`.
struct CheckedOwner {
    name: String,
    lk_name: Option<String>,
    uid: Option<String>,
}

/// `Eigentuemer` in SIA405 is `TEXT*80`
const MAX_LK_NAME: usize = 80;

impl OwnerInput {
    /// Refuses what the table would refuse, with a message; `owner_id` is the owner being
    /// changed (its own name and UID don't count as taken).
    async fn checked(
        self,
        connection: &mut AsyncPgConnection,
        owner_id: Option<i32>,
    ) -> async_graphql::Result<CheckedOwner> {
        let name = self.name.trim().to_string();
        if name.is_empty() {
            return Err("Der Eigentümer braucht einen Namen".into());
        }
        let lk_name = non_empty(self.lk_name);
        if let Some(lk_name) = &lk_name
            && lk_name.chars().count() > MAX_LK_NAME
        {
            return Err(
                format!("Der Name in der Lieferung hat mehr als {MAX_LK_NAME} Zeichen").into(),
            );
        }
        let uid = non_empty(self.uid);
        if let Some(uid) = &uid
            && !is_uid(uid)
        {
            return Err(format!(
                "Die UID {uid} hat nicht die Form CHE-123.456.789 (fiktiv: ZHE-…)"
            )
            .into());
        }
        let others = || {
            schema::eigentuemer::table
                .filter(schema::eigentuemer::id.ne(owner_id.unwrap_or(0)))
                .select(schema::eigentuemer::name)
        };
        if let Some(other) = others()
            .filter(schema::eigentuemer::name.eq(&name))
            .first::<String>(connection)
            .await
            .optional()?
        {
            return Err(format!("Es gibt schon einen Eigentümer {other}").into());
        }
        if let Some(uid) = &uid
            && let Some(other) = others()
                .filter(schema::eigentuemer::uid.eq(uid))
                .first::<String>(connection)
                .await
                .optional()?
        {
            return Err(format!("Die UID {uid} hat schon der Eigentümer {other}").into());
        }
        Ok(CheckedOwner { name, lk_name, uid })
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}
