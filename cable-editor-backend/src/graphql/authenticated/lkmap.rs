//! The delivery to the Leitungskataster as GraphQL (see `crate::lkmap`).

use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::{Duct, eigentuemer::Eigentuemer, lkmap::LkLieferung, schacht::Schacht},
        schema,
    },
    graphql::{authenticated::get_connection, model::GeoPoint},
    lkmap,
};
use async_graphql::{ComplexObject, Context, SimpleObject};
use chrono::{DateTime, Utc};
use diesel::{
    ExpressionMethods, HasQuery, QueryDsl, QueryableByName, sql_query,
    sql_types::{Array, Integer, Nullable, Timestamptz},
};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

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

/// An owner's delivery: the transfer files (SIA405 LKMap, Zuständigkeitsperimeter), what goes
/// into them and what can't, a report the UI words (docs/fehlermeldungen.md), and the
/// deliveries so far.
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct LkmapExport {
    pub owner: Eigentuemer,
    /// The ducts and Schächte; missing without UID and when nothing has a position
    pub lkmap: Option<TransferFile>,
    /// The area they lie in, missing like `lkmap`
    pub perimeter: Option<TransferFile>,
    /// The perimeter's outline, for the map
    pub perimeter_area: Option<Vec<GeoPoint>>,
    /// SHA-256 of both files; differs from the last delivery's after a change
    pub checksum: Option<String>,
    /// The delivered Schächte, ducts
    pub schaechte: Vec<Schacht>,
    pub ducts: Vec<Duct>,
    /// Schächte without position: neither they nor their ducts can be delivered
    pub schaechte_without_position: Vec<Schacht>,
    /// Delivered ducts ending at a Schacht without position
    pub ducts_without_line: Vec<Duct>,
    #[graphql(skip)]
    schacht_ids: Vec<i32>,
    #[graphql(skip)]
    duct_ids: Vec<i32>,
}

#[ComplexObject]
impl LkmapExport {
    /// The owner's deliveries, the latest first
    async fn deliveries(&self, ctx: &Context<'_>) -> ApiResult<Vec<LkLieferung>> {
        let mut connection = get_connection(ctx).await?;
        Ok(deliveries(&mut connection, self.owner.id).await?)
    }
    /// The first change of a delivered Schacht or duct after the files last delivered were
    /// downloaded, when the week to deliver started; missing if nothing was delivered yet or
    /// no delivered object changed (a deleted one or a changed perimeter only shows in the
    /// checksum).
    async fn first_change_since_delivery(
        &self,
        ctx: &Context<'_>,
    ) -> ApiResult<Option<DateTime<Utc>>> {
        let mut connection = get_connection(ctx).await?;
        let last_delivered = deliveries(&mut connection, self.owner.id)
            .await?
            .into_iter()
            .find(|delivery| delivery.geliefert_am.is_some());
        let Some(last_delivered) = last_delivered else {
            return Ok(None);
        };
        Ok(sql_query(FIRST_CHANGE)
            .bind::<Array<Integer>, _>(&self.duct_ids)
            .bind::<Array<Integer>, _>(&self.schacht_ids)
            .bind::<Timestamptz, _>(last_delivered.erstellt_am)
            .get_result::<FirstChange>(&mut connection)
            .await?
            .first_change)
    }
}

/// `now()` of a download's transaction is its `erstellt_am`, so an object changed before has
/// an earlier `geaendert_am`.
const FIRST_CHANGE: &str = "
select min(c.geaendert_am) as first_change
from (select geaendert_am from trasse where id = any($1)
      union all
      select geaendert_am from schacht where id = any($2)) c
where c.geaendert_am > $3";

#[derive(QueryableByName)]
struct FirstChange {
    #[diesel(sql_type = Nullable<Timestamptz>)]
    first_change: Option<DateTime<Utc>>,
}

async fn deliveries(
    connection: &mut AsyncPgConnection,
    owner_id: i32,
) -> diesel::QueryResult<Vec<LkLieferung>> {
    LkLieferung::query()
        .filter(schema::lk_lieferung::eigentuemer_id.eq(owner_id))
        .order(schema::lk_lieferung::erstellt_am.desc())
        .then_order_by(schema::lk_lieferung::id.desc())
        .load(connection)
        .await
}

pub async fn export(ctx: &Context<'_>, owner_id: i32) -> ApiResult<LkmapExport> {
    let mut connection = get_connection(ctx).await?;
    let export = lkmap::export(&mut connection, owner_id).await?;
    to_graphql(&mut connection, export).await
}

/// Owners with something to deliver or delivered before, by name.
const OWNERS: &str = "
select e.id
from eigentuemer e
where exists (select 1 from trasse t where t.leitungskataster and t.eigentuemer_id = e.id)
   or exists (select 1
              from schacht s
                       join trasse t on t.leitungskataster and s.id in (t.schacht_a, t.schacht_z)
              where s.eigentuemer_id = e.id)
   or exists (select 1 from lk_lieferung l where l.eigentuemer_id = e.id)
order by e.name";

#[derive(QueryableByName)]
struct OwnerRow {
    #[diesel(sql_type = Integer)]
    id: i32,
}

pub async fn exports(ctx: &Context<'_>) -> ApiResult<Vec<LkmapExport>> {
    let mut connection = get_connection(ctx).await?;
    let owners: Vec<OwnerRow> = sql_query(OWNERS).load(&mut connection).await?;
    let mut exports = Vec::with_capacity(owners.len());
    for owner in owners {
        let export = lkmap::export(&mut connection, owner.id).await?;
        exports.push(to_graphql(&mut connection, export).await?);
    }
    Ok(exports)
}

async fn to_graphql(
    connection: &mut AsyncPgConnection,
    export: lkmap::Export,
) -> ApiResult<LkmapExport> {
    let (lkmap, perimeter, perimeter_area, checksum) = match export.files {
        Some(files) => (
            Some(files.lkmap.try_into()?),
            Some(files.perimeter.try_into()?),
            Some(
                files
                    .perimeter_wgs84
                    .into_iter()
                    .map(|(lat, lng)| GeoPoint { lat, lng })
                    .collect(),
            ),
            Some(files.checksum),
        ),
        None => (None, None, None, None),
    };
    Ok(LkmapExport {
        owner: export.owner,
        lkmap,
        perimeter,
        perimeter_area,
        checksum,
        schaechte: schaechte(connection, &export.schaechte).await?,
        ducts: ducts(connection, &export.ducts).await?,
        schaechte_without_position: schaechte(connection, &export.schaechte_without_position)
            .await?,
        ducts_without_line: ducts(connection, &export.ducts_without_line).await?,
        schacht_ids: export.schaechte,
        duct_ids: export.ducts,
    })
}

async fn schaechte(
    connection: &mut AsyncPgConnection,
    ids: &[i32],
) -> diesel::QueryResult<Vec<Schacht>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    Schacht::query()
        .filter(schema::schacht::id.eq_any(ids))
        .order(schema::schacht::id)
        .load(connection)
        .await
}

async fn ducts(connection: &mut AsyncPgConnection, ids: &[i32]) -> diesel::QueryResult<Vec<Duct>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    Duct::query()
        .filter(schema::trasse::id.eq_any(ids))
        .order(schema::trasse::id)
        .load(connection)
        .await
}
