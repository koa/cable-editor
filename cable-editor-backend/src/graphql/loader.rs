//! Batches the lookups of referenced objects (a duct's Schächte, a Schacht's type, ...) of one
//! request into one query per kind, instead of one query per object.
//!
//! The loader uses the request's connection, so it sees the request's transaction. Resolvers
//! must not hold `get_connection`'s guard while waiting for the loader, that would deadlock.

use crate::{
    db::{
        entity::{
            Duct, WGS84,
            cable::{Cable, cable_usages_at},
            panel::{Panel, PanelPort, PortUsage},
            plan::Plan,
            schacht::{Schacht, SchachtTyp},
            st_length, st_transform,
        },
        schema,
    },
    graphql::model::GeoPoint,
};
use async_graphql::{Context, dataloader::DataLoader, dataloader::Loader};
use diesel::{ExpressionMethods, HasQuery, QueryDsl, dsl::sum};
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

/// Root panels of a Schacht, in their order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SchachtRootPanels(pub i32);

/// Length of a cable in metres along its ducts, missing without ducts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CableLength(pub i32);

/// Ducts of a cable with their sequence, in order; missing without ducts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CableDucts(pub i32);

/// Port usages of a cable at a Schacht as they are in a plan (`cable_usages_at`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CableEndUsages {
    pub cable: i32,
    pub schacht: i32,
    pub plan: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PanelId(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PanelPortId(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PlanId(pub i32);

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

impl Loader<SchachtRootPanels> for DbLoader {
    type Value = Vec<Panel>;
    type Error = async_graphql::Error;

    async fn load(
        &self,
        keys: &[SchachtRootPanels],
    ) -> Result<HashMap<SchachtRootPanels, Vec<Panel>>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<Panel> = Panel::query()
            .filter(schema::panel::schacht_id.eq_any(ids(keys, |k| k.0)))
            .filter(schema::panel::parent_panel.is_null())
            .order(schema::panel::parent_order.asc())
            .load(&mut connection)
            .await?;
        let mut panels: HashMap<SchachtRootPanels, Vec<Panel>> = HashMap::new();
        for panel in list {
            panels
                .entry(SchachtRootPanels(panel.schacht_id))
                .or_default()
                .push(panel);
        }
        Ok(panels)
    }
}

impl Loader<CableLength> for DbLoader {
    type Value = f64;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[CableLength]) -> Result<HashMap<CableLength, f64>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<(i32, Option<f64>)> = schema::trassen_mit_endpunkten::table
            .inner_join(schema::kabel_trasse::table)
            .filter(schema::kabel_trasse::kabel.eq_any(ids(keys, |k| k.0)))
            .group_by(schema::kabel_trasse::kabel)
            .select((
                schema::kabel_trasse::kabel,
                sum(st_length(schema::trassen_mit_endpunkten::geom)),
            ))
            .load(&mut connection)
            .await?;
        Ok(list
            .into_iter()
            .filter_map(|(id, length)| Some((CableLength(id), length?)))
            .collect())
    }
}

impl Loader<CableDucts> for DbLoader {
    type Value = Vec<(Duct, i32)>;
    type Error = async_graphql::Error;

    async fn load(
        &self,
        keys: &[CableDucts],
    ) -> Result<HashMap<CableDucts, Vec<(Duct, i32)>>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<(i32, Duct, i32)> = schema::trasse::table
            .inner_join(schema::kabel_trasse::table)
            .filter(schema::kabel_trasse::kabel.eq_any(ids(keys, |k| k.0)))
            .order((
                schema::kabel_trasse::kabel,
                schema::kabel_trasse::sequenz.asc(),
            ))
            .select((
                schema::kabel_trasse::kabel,
                schema::trasse::all_columns,
                schema::kabel_trasse::sequenz,
            ))
            .load(&mut connection)
            .await?;
        let mut ducts: HashMap<CableDucts, Vec<(Duct, i32)>> = HashMap::new();
        for (cable, duct, sequence) in list {
            ducts
                .entry(CableDucts(cable))
                .or_default()
                .push((duct, sequence));
        }
        Ok(ducts)
    }
}

impl Loader<CableEndUsages> for DbLoader {
    type Value = Vec<PortUsage>;
    type Error = async_graphql::Error;

    /// One query per cable end (instead of per fiber); a page shows few cable ends.
    async fn load(
        &self,
        keys: &[CableEndUsages],
    ) -> Result<HashMap<CableEndUsages, Vec<PortUsage>>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let mut usages = HashMap::new();
        for key in keys {
            let found = cable_usages_at(&mut connection, key.plan, key.cable, key.schacht).await?;
            usages.insert(*key, found);
        }
        Ok(usages)
    }
}

impl Loader<PanelId> for DbLoader {
    type Value = Panel;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[PanelId]) -> Result<HashMap<PanelId, Panel>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<Panel> = Panel::query()
            .filter(schema::panel::id.eq_any(ids(keys, |k| k.0)))
            .load(&mut connection)
            .await?;
        Ok(list.into_iter().map(|p| (PanelId(p.id), p)).collect())
    }
}

impl Loader<PanelPortId> for DbLoader {
    type Value = PanelPort;
    type Error = async_graphql::Error;

    async fn load(
        &self,
        keys: &[PanelPortId],
    ) -> Result<HashMap<PanelPortId, PanelPort>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<PanelPort> = PanelPort::query()
            .filter(schema::panel_port::id.eq_any(ids(keys, |k| k.0)))
            .load(&mut connection)
            .await?;
        Ok(list.into_iter().map(|p| (PanelPortId(p.id), p)).collect())
    }
}

impl Loader<PlanId> for DbLoader {
    type Value = Plan;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[PlanId]) -> Result<HashMap<PlanId, Plan>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<Plan> = Plan::query()
            .filter(schema::plan::id.eq_any(ids(keys, |k| k.0)))
            .load(&mut connection)
            .await?;
        Ok(list.into_iter().map(|p| (PlanId(p.id), p)).collect())
    }
}
