//! Batches the lookups of referenced objects (a duct's Schächte, a Schacht's type, ...) of one
//! request into one query per kind, instead of one query per object.
//!
//! The loader uses the request's connection, so it sees the request's transaction. Resolvers
//! must not hold `get_connection`'s guard while waiting for the loader, that would deadlock.

use crate::db::{
    entity::{
        cable::Cable,
        schacht::{Schacht, SchachtTyp},
    },
    schema,
};
use async_graphql::{Context, dataloader::DataLoader, dataloader::Loader};
use diesel::{ExpressionMethods, HasQuery, QueryDsl};
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Object};
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
