//! The delivery to the Leitungskataster as GraphQL (see `crate::lkmap`).

use crate::{
    db::{
        entity::{Duct, schacht::Schacht},
        schema,
    },
    graphql::authenticated::get_connection,
    lkmap,
};
use async_graphql::{Context, SimpleObject};
use diesel::{ExpressionMethods, HasQuery, QueryDsl};
use diesel_async::RunQueryDsl;

/// A transfer file of the delivery.
#[derive(SimpleObject)]
pub struct TransferFile {
    /// `<uid>-kommunikation-lkmap.xtf`, `<uid>-zustaendigkeit-peri.xtf`
    pub file_name: String,
    /// The XTF, UTF-8
    pub xtf: String,
}

impl TryFrom<lkmap::TransferFile> for TransferFile {
    type Error = std::string::FromUtf8Error;

    fn try_from(file: lkmap::TransferFile) -> Result<Self, Self::Error> {
        Ok(TransferFile {
            file_name: file.file_name,
            xtf: String::from_utf8(file.xtf)?,
        })
    }
}

/// An owner's transfer files (SIA405 LKMap, Zuständigkeitsperimeter) and what couldn't go into
/// them: a report, the UI says why (docs/fehlermeldungen.md).
#[derive(SimpleObject)]
pub struct LkmapExport {
    /// The ducts and Schächte
    pub lkmap: TransferFile,
    /// The area they lie in
    pub perimeter: TransferFile,
    pub schacht_count: i32,
    pub duct_count: i32,
    /// Schächte without position: neither they nor their ducts can be delivered
    pub schaechte_without_position: Vec<Schacht>,
    /// Delivered ducts ending at a Schacht without position
    pub ducts_without_line: Vec<Duct>,
}

pub async fn export(ctx: &Context<'_>, owner_id: i32) -> async_graphql::Result<LkmapExport> {
    let mut connection = get_connection(ctx).await?;
    let export = lkmap::export(&mut connection, owner_id).await?;
    let schaechte_without_position = Schacht::query()
        .filter(schema::schacht::id.eq_any(&export.schaechte_without_position))
        .order(schema::schacht::id)
        .load(&mut connection)
        .await?;
    let ducts_without_line = Duct::query()
        .filter(schema::trasse::id.eq_any(&export.ducts_without_line))
        .order(schema::trasse::id)
        .load(&mut connection)
        .await?;
    Ok(LkmapExport {
        lkmap: export.lkmap.try_into()?,
        perimeter: export.perimeter.try_into()?,
        schacht_count: i32::try_from(export.schacht_count)?,
        duct_count: i32::try_from(export.duct_count)?,
        schaechte_without_position,
        ducts_without_line,
    })
}
