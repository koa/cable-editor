//! The delivery to the Leitungskataster (see docs/leitungskataster.md): downloading an owner's
//! files logs a delivery, which is marked once they reached the Checkservice. Admin only.

use crate::{
    db::{entity::lkmap::LkLieferung, schema},
    graphql::{
        authenticated,
        authorization::{Role, RoleGuard},
        context::UserInfo,
    },
    lkmap,
};
use async_graphql::{Context, Object, SimpleObject};
use base64::{Engine, engine::general_purpose::STANDARD};
use cable_editor_common::{ObjectKind, UserError};
use chrono::{DateTime, Utc};
use diesel::{
    ExpressionMethods, HasQuery, OptionalExtension, QueryDsl, SelectableHelper,
    dsl::sql,
    sql_types::{Nullable, Timestamptz},
};
use diesel_async::RunQueryDsl;

#[derive(Default)]
pub struct LkmapMutation;

/// A file to save.
#[derive(SimpleObject)]
pub struct DownloadFile {
    pub file_name: String,
    /// The content in base64
    pub content: String,
}

/// An owner's transfer files, each in a ZIP of the same name, and the delivery logged for them.
#[derive(SimpleObject)]
pub struct LkmapDownload {
    pub delivery: LkLieferung,
    /// LKMap and Zuständigkeitsperimeter
    pub files: Vec<DownloadFile>,
}

#[Object]
impl LkmapMutation {
    /// The owner's transfer files; logs them as a delivery, or reuses the one logged for the
    /// same files that isn't marked yet.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn download_lkmap(
        &self,
        ctx: &Context<'_>,
        owner_id: i32,
    ) -> async_graphql::Result<LkmapDownload> {
        let user = ctx.data::<UserInfo>()?.preferred_username.to_string();
        let mut connection = authenticated::get_connection(ctx).await?;
        let export = lkmap::export(&mut connection, owner_id).await?;
        let files = export.deliverable()?;
        let pending = LkLieferung::query()
            .filter(schema::lk_lieferung::eigentuemer_id.eq(owner_id))
            .filter(schema::lk_lieferung::pruefsumme.eq(&files.checksum))
            .filter(schema::lk_lieferung::geliefert_am.is_null())
            .order(schema::lk_lieferung::id.desc())
            .first(&mut connection)
            .await
            .optional()?;
        let delivery = match pending {
            Some(delivery) => delivery,
            None => {
                diesel::insert_into(schema::lk_lieferung::table)
                    .values((
                        schema::lk_lieferung::eigentuemer_id.eq(owner_id),
                        schema::lk_lieferung::erstellt_von.eq(user),
                        schema::lk_lieferung::anzahl_schaechte
                            .eq(i32::try_from(export.schaechte.len())?),
                        schema::lk_lieferung::anzahl_trassen.eq(i32::try_from(export.ducts.len())?),
                        schema::lk_lieferung::pruefsumme.eq(&files.checksum),
                    ))
                    .returning(LkLieferung::as_returning())
                    .get_result(&mut connection)
                    .await?
            }
        };
        let files = [&files.lkmap, &files.perimeter]
            .into_iter()
            .map(|file| {
                Ok(DownloadFile {
                    file_name: file.zip_name(),
                    content: STANDARD.encode(file.zip()?),
                })
            })
            .collect::<async_graphql::Result<_>>()?;
        Ok(LkmapDownload { delivery, files })
    }

    /// Marks a delivery as having reached the Checkservice (now, kept if already marked), or
    /// takes the mark back, e.g. when the Checkservice refused the files.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn set_lkmap_delivered(
        &self,
        ctx: &Context<'_>,
        delivery_id: i32,
        delivered: bool,
    ) -> async_graphql::Result<LkLieferung> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let delivery = schema::lk_lieferung::table.find(delivery_id);
        if delivered {
            diesel::update(delivery.filter(schema::lk_lieferung::geliefert_am.is_null()))
                .set(schema::lk_lieferung::geliefert_am.eq(sql::<Nullable<Timestamptz>>("now()")))
                .execute(&mut connection)
                .await?;
        } else {
            diesel::update(delivery)
                .set(schema::lk_lieferung::geliefert_am.eq(None::<DateTime<Utc>>))
                .execute(&mut connection)
                .await?;
        }
        LkLieferung::query()
            .filter(schema::lk_lieferung::id.eq(delivery_id))
            .first(&mut connection)
            .await
            .optional()?
            .ok_or_else(|| {
                UserError::NotFound {
                    kind: ObjectKind::LkmapDelivery,
                    id: delivery_id.into(),
                }
                .into()
            })
    }
}
