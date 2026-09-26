//! Batches the lookups of referenced objects (a duct's Schächte, a Schacht's type, ...) of one
//! request into one query per kind, instead of one query per object.
//!
//! The loader uses the request's connection, so it sees the request's transaction. Resolvers
//! must not hold `get_connection`'s guard while waiting for the loader, that would deadlock.

use crate::{
    db::{
        entity::{
            WGS84,
            cable::Cable,
            schacht::{Schacht, SchachtTyp},
            st_length, st_transform,
        },
        schema,
    },
    graphql::model::GeoPoint,
};
use async_graphql::{Context, dataloader::DataLoader, dataloader::Loader};
use diesel::{ExpressionMethods, HasQuery, QueryDsl};
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Object};
use postgis_diesel::types::{LineString, Point};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

/// The request's connection, inside its transaction (see `binary/src/main.rs`).
pub type SharedConnection = Arc<Mutex<Object<AsyncPgConnection>>>;

pub struct DbLoader {
    connection: SharedConnection,
}

impl DbLoader {
    pub fn new(connection: SharedConnection) -> Self {
        Self { connection }
    }
}

pub fn get_loader<'a>(ctx: &'a Context<'_>) -> async_graphql::Result<&'a DataLoader<DbLoader>> {
    ctx.data::<DataLoader<DbLoader>>()
}

/// Loads one referenced object, which must exist (foreign key).
pub async fn load_one<K>(ctx: &Context<'_>, key: K) -> async_graphql::Result<DbValue<K>>
where
    K: Send + Sync + std::hash::Hash + Eq + Clone + std::fmt::Debug + 'static,
    DbLoader: Loader<K, Error = async_graphql::Error>,
{
    get_loader(ctx)?
        .load_one(key.clone())
        .await?
        .ok_or_else(|| format!("{key:?} not found").into())
}
type DbValue<K> = <DbLoader as Loader<K>>::Value;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SchachtId(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SchachtTypId(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CableId(pub i32);

/// Position of a Schacht in WGS84, missing without geometry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SchachtLocation(pub i32);

/// Line of a duct in WGS84 including its Schächte, missing without geometry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DuctLine(pub i32);

/// Length of a duct in metres (from Schacht A to Z), missing without geometry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DuctLength(pub i32);

/// Cables through a duct, missing for an empty duct.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DuctCables(pub i32);

fn ids<K>(keys: &[K], id: impl Fn(&K) -> i32) -> Vec<i32> {
    keys.iter().map(id).collect()
}

impl Loader<SchachtId> for DbLoader {
    type Value = Schacht;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[SchachtId]) -> Result<HashMap<SchachtId, Schacht>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<Schacht> = Schacht::query()
            .filter(schema::schacht::id.eq_any(ids(keys, |k| k.0)))
            .load(&mut connection)
            .await?;
        Ok(list.into_iter().map(|s| (SchachtId(s.id), s)).collect())
    }
}

impl Loader<SchachtTypId> for DbLoader {
    type Value = SchachtTyp;
    type Error = async_graphql::Error;

    async fn load(
        &self,
        keys: &[SchachtTypId],
    ) -> Result<HashMap<SchachtTypId, SchachtTyp>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<SchachtTyp> = SchachtTyp::query()
            .filter(schema::schacht_typ::id.eq_any(ids(keys, |k| k.0)))
            .load(&mut connection)
            .await?;
        Ok(list.into_iter().map(|t| (SchachtTypId(t.id), t)).collect())
    }
}

impl Loader<CableId> for DbLoader {
    type Value = Cable;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[CableId]) -> Result<HashMap<CableId, Cable>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<Cable> = Cable::query()
            .filter(schema::kabel::id.eq_any(ids(keys, |k| k.0)))
            .load(&mut connection)
            .await?;
        Ok(list.into_iter().map(|c| (CableId(c.id), c)).collect())
    }
}

impl Loader<SchachtLocation> for DbLoader {
    type Value = GeoPoint;
    type Error = async_graphql::Error;

    async fn load(
        &self,
        keys: &[SchachtLocation],
    ) -> Result<HashMap<SchachtLocation, GeoPoint>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<(i32, Option<Point>)> = schema::schacht::table
            .filter(schema::schacht::id.eq_any(ids(keys, |k| k.0)))
            .select((
                schema::schacht::id,
                st_transform(schema::schacht::geom, WGS84),
            ))
            .load(&mut connection)
            .await?;
        Ok(list
            .into_iter()
            .filter_map(|(id, point)| Some((SchachtLocation(id), point?.into())))
            .collect())
    }
}

impl Loader<DuctLine> for DbLoader {
    type Value = Vec<GeoPoint>;
    type Error = async_graphql::Error;

    async fn load(
        &self,
        keys: &[DuctLine],
    ) -> Result<HashMap<DuctLine, Vec<GeoPoint>>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<(i32, Option<LineString<Point>>)> = schema::trassen_mit_endpunkten::table
            .filter(schema::trassen_mit_endpunkten::id.eq_any(ids(keys, |k| k.0)))
            .select((
                schema::trassen_mit_endpunkten::id,
                st_transform(schema::trassen_mit_endpunkten::geom, WGS84),
            ))
            .load(&mut connection)
            .await?;
        Ok(list
            .into_iter()
            .filter_map(|(id, line)| {
                let points = line?.points.into_iter().map(GeoPoint::from).collect();
                Some((DuctLine(id), points))
            })
            .collect())
    }
}

impl Loader<DuctCables> for DbLoader {
    type Value = Vec<Cable>;
    type Error = async_graphql::Error;

    async fn load(
        &self,
        keys: &[DuctCables],
    ) -> Result<HashMap<DuctCables, Vec<Cable>>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<(i32, Cable)> = schema::kabel_trasse::table
            .inner_join(schema::kabel::table)
            .filter(schema::kabel_trasse::trasse.eq_any(ids(keys, |k| k.0)))
            .order(schema::kabel::name.asc())
            .select((schema::kabel_trasse::trasse, schema::kabel::all_columns))
            .load(&mut connection)
            .await?;
        let mut cables: HashMap<DuctCables, Vec<Cable>> = HashMap::new();
        for (duct, cable) in list {
            cables.entry(DuctCables(duct)).or_default().push(cable);
        }
        Ok(cables)
    }
}

impl Loader<DuctLength> for DbLoader {
    type Value = f64;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[DuctLength]) -> Result<HashMap<DuctLength, f64>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<(i32, Option<f64>)> = schema::trassen_mit_endpunkten::table
            .filter(schema::trassen_mit_endpunkten::id.eq_any(ids(keys, |k| k.0)))
            .select((
                schema::trassen_mit_endpunkten::id,
                st_length(schema::trassen_mit_endpunkten::geom),
            ))
            .load(&mut connection)
            .await?;
        Ok(list
            .into_iter()
            .filter_map(|(id, length)| Some((DuctLength(id), length?)))
            .collect())
    }
}
