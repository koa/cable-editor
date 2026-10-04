//! Batches the lookups of referenced objects (a duct's Schächte, a Schacht's type, ...) of one
//! request into one query per kind, instead of one query per object.
//!
//! The loader uses the request's connection, so it sees the request's transaction. Resolvers
//! must not hold `get_connection`'s guard while waiting for the loader, that would deadlock.

use crate::graphql::error::{ApiError, ApiResult};
use crate::{
    db::{
        entity::{
            Duct, WGS84,
            cable::{Cable, cable_usages_at},
            eigentuemer::Eigentuemer,
            panel::{Panel, PanelPort, PortSide, PortUsage},
            plan::Plan,
            schacht::{Schacht, SchachtTyp},
            st_length, st_transform,
        },
        schema,
    },
    graphql::model::GeoPoint,
};
use async_graphql::{Context, dataloader::DataLoader, dataloader::Loader};
use cable_editor_common::{ObjectKind, UserError};
use diesel::{
    ExpressionMethods, HasQuery, QueryDsl, QueryableByName, SelectableHelper,
    dsl::{count_star, sum},
    sql_query,
    sql_types::{Array, Integer},
};
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

pub fn get_loader<'a>(ctx: &'a Context<'_>) -> ApiResult<&'a DataLoader<DbLoader>> {
    Ok(ctx.data::<DataLoader<DbLoader>>()?)
}

/// Loads one referenced object, which must exist (foreign key).
pub async fn load_one<K>(ctx: &Context<'_>, key: K) -> ApiResult<DbValue<K>>
where
    K: ObjectKey + Send + Sync + std::hash::Hash + Eq + Clone + std::fmt::Debug + 'static,
    DbLoader: Loader<K, Error = ApiError>,
{
    let id = key.id();
    get_loader(ctx)?.load_one(key).await?.ok_or_else(|| {
        UserError::NotFound {
            kind: K::KIND,
            id: id.into(),
        }
        .into()
    })
}

/// A key naming one object, for the error when it doesn't exist.
pub trait ObjectKey {
    const KIND: ObjectKind;
    fn id(&self) -> i32;
}

macro_rules! object_key {
    ($($key:ident => $kind:ident),* $(,)?) => {
        $(impl ObjectKey for $key {
            const KIND: ObjectKind = ObjectKind::$kind;
            fn id(&self) -> i32 {
                self.0
            }
        })*
    };
}

object_key!(
    SchachtId => Schacht,
    SchachtTypId => SchachtTyp,
    CableId => Cable,
    EigentuemerId => Owner,
    PanelId => Panel,
    PanelPortId => Port,
    PlanId => Plan,
);
type DbValue<K> = <DbLoader as Loader<K>>::Value;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SchachtId(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SchachtTypId(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CableId(pub i32);

/// How many Schächte have a type; missing: none.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SchachtTypCount(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EigentuemerId(pub i32);

/// How many Schächte and ducts an owner has; missing: none.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EigentuemerCounts(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct OwnedCounts {
    pub schaechte: i32,
    pub ducts: i32,
    /// Ducts delivered to the Leitungskataster
    pub delivered_ducts: i32,
}

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

/// The ports of a panel, in their order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PanelPorts(pub i32);

/// The panels above a panel, from its root panel down to its parent; missing for a root panel.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PanelParentChain(pub i32);

/// What a port's side holds in a plan (`effective_port_usage`), missing if nothing.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EffectiveUsage {
    pub plan: i32,
    pub port: i32,
    pub side: PortSide,
}

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

/// The lists grouped while loading, done.
fn boxed<K: std::hash::Hash + Eq, V>(groups: HashMap<K, Vec<V>>) -> HashMap<K, Box<[V]>> {
    groups
        .into_iter()
        .map(|(key, values)| (key, values.into_boxed_slice()))
        .collect()
}

impl Loader<SchachtId> for DbLoader {
    type Value = Schacht;
    type Error = ApiError;

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
    type Error = ApiError;

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

impl Loader<EigentuemerId> for DbLoader {
    type Value = Eigentuemer;
    type Error = ApiError;

    async fn load(
        &self,
        keys: &[EigentuemerId],
    ) -> Result<HashMap<EigentuemerId, Eigentuemer>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<Eigentuemer> = Eigentuemer::query()
            .filter(schema::eigentuemer::id.eq_any(ids(keys, |k| k.0)))
            .load(&mut connection)
            .await?;
        Ok(list.into_iter().map(|e| (EigentuemerId(e.id), e)).collect())
    }
}

impl Loader<SchachtTypCount> for DbLoader {
    type Value = i32;
    type Error = ApiError;

    async fn load(
        &self,
        keys: &[SchachtTypCount],
    ) -> Result<HashMap<SchachtTypCount, i32>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let counts: Vec<(Option<i32>, i64)> = schema::schacht::table
            .filter(schema::schacht::typ.eq_any(ids(keys, |k| k.0)))
            .group_by(schema::schacht::typ)
            .select((schema::schacht::typ, count_star()))
            .load(&mut connection)
            .await?;
        counts
            .into_iter()
            .filter_map(|(typ, count)| Some((SchachtTypCount(typ?), count)))
            .map(|(typ, count)| Ok((typ, i32::try_from(count)?)))
            .collect()
    }
}

impl Loader<EigentuemerCounts> for DbLoader {
    type Value = OwnedCounts;
    type Error = ApiError;

    async fn load(
        &self,
        keys: &[EigentuemerCounts],
    ) -> Result<HashMap<EigentuemerCounts, OwnedCounts>, Self::Error> {
        let owners = ids(keys, |k| k.0);
        let mut connection = self.connection.lock().await;
        let schaechte: Vec<(i32, i64)> = schema::schacht::table
            .filter(schema::schacht::eigentuemer_id.eq_any(&owners))
            .group_by(schema::schacht::eigentuemer_id)
            .select((schema::schacht::eigentuemer_id, count_star()))
            .load(&mut connection)
            .await?;
        let ducts: Vec<(i32, bool, i64)> = schema::trasse::table
            .filter(schema::trasse::eigentuemer_id.eq_any(&owners))
            .group_by((
                schema::trasse::eigentuemer_id,
                schema::trasse::leitungskataster,
            ))
            .select((
                schema::trasse::eigentuemer_id,
                schema::trasse::leitungskataster,
                count_star(),
            ))
            .load(&mut connection)
            .await?;
        let mut counts: HashMap<EigentuemerCounts, OwnedCounts> = HashMap::new();
        for (owner, count) in schaechte {
            counts
                .entry(EigentuemerCounts(owner))
                .or_default()
                .schaechte = i32::try_from(count)?;
        }
        for (owner, delivered, count) in ducts {
            let count = i32::try_from(count)?;
            let entry = counts.entry(EigentuemerCounts(owner)).or_default();
            entry.ducts += count;
            if delivered {
                entry.delivered_ducts = count;
            }
        }
        Ok(counts)
    }
}

impl Loader<CableId> for DbLoader {
    type Value = Cable;
    type Error = ApiError;

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
    type Error = ApiError;

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
    type Value = Box<[GeoPoint]>;
    type Error = ApiError;

    async fn load(
        &self,
        keys: &[DuctLine],
    ) -> Result<HashMap<DuctLine, Box<[GeoPoint]>>, Self::Error> {
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
    type Value = Box<[Cable]>;
    type Error = ApiError;

    async fn load(
        &self,
        keys: &[DuctCables],
    ) -> Result<HashMap<DuctCables, Box<[Cable]>>, Self::Error> {
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
        Ok(boxed(cables))
    }
}

impl Loader<DuctLength> for DbLoader {
    type Value = f64;
    type Error = ApiError;

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
    type Value = Box<[Panel]>;
    type Error = ApiError;

    async fn load(
        &self,
        keys: &[SchachtRootPanels],
    ) -> Result<HashMap<SchachtRootPanels, Box<[Panel]>>, Self::Error> {
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
        Ok(boxed(panels))
    }
}

impl Loader<CableLength> for DbLoader {
    type Value = f64;
    type Error = ApiError;

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
    type Value = Box<[(Duct, i32)]>;
    type Error = ApiError;

    async fn load(
        &self,
        keys: &[CableDucts],
    ) -> Result<HashMap<CableDucts, Box<[(Duct, i32)]>>, Self::Error> {
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
                Duct::as_select(),
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
        Ok(boxed(ducts))
    }
}

impl Loader<CableEndUsages> for DbLoader {
    type Value = Box<[PortUsage]>;
    type Error = ApiError;

    /// One query per cable end (instead of per fiber); a page shows few cable ends.
    async fn load(
        &self,
        keys: &[CableEndUsages],
    ) -> Result<HashMap<CableEndUsages, Box<[PortUsage]>>, Self::Error> {
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
    type Error = ApiError;

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
    type Error = ApiError;

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

impl Loader<PanelPorts> for DbLoader {
    type Value = Box<[PanelPort]>;
    type Error = ApiError;

    async fn load(
        &self,
        keys: &[PanelPorts],
    ) -> Result<HashMap<PanelPorts, Box<[PanelPort]>>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<PanelPort> = PanelPort::query()
            .filter(schema::panel_port::panel_id.eq_any(ids(keys, |k| k.0)))
            .order((
                schema::panel_port::panel_id,
                schema::panel_port::port_order.asc(),
            ))
            .load(&mut connection)
            .await?;
        let mut ports: HashMap<PanelPorts, Vec<PanelPort>> = HashMap::new();
        for port in list {
            ports
                .entry(PanelPorts(port.panel_id))
                .or_default()
                .push(port);
        }
        Ok(boxed(ports))
    }
}

/// A panel above the panel `below`.
#[derive(QueryableByName)]
struct ParentRow {
    #[diesel(sql_type = Integer)]
    below: i32,
    #[diesel(embed)]
    panel: Panel,
}

impl Loader<PanelParentChain> for DbLoader {
    type Value = Box<[Panel]>;
    type Error = ApiError;

    /// One recursive query up the parents of all the panels.
    async fn load(
        &self,
        keys: &[PanelParentChain],
    ) -> Result<HashMap<PanelParentChain, Box<[Panel]>>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<ParentRow> = sql_query(
            r#"
WITH RECURSIVE chain AS (
    SELECT p.id AS below, p.parent_panel AS next, 0 AS depth
    FROM panel p
    WHERE p.id = ANY($1)
    UNION ALL
    SELECT c.below, p.parent_panel, c.depth + 1
    FROM chain c
    JOIN panel p ON p.id = c.next
)
SELECT c.below, p.id, p.name, p.schacht_id, p.parent_panel, p.parent_order, p.netbox_device_id
FROM chain c
JOIN panel p ON p.id = c.next
ORDER BY c.below, c.depth DESC
"#,
        )
        .bind::<Array<Integer>, _>(ids(keys, |k| k.0))
        .load(&mut connection)
        .await?;
        let mut chains: HashMap<PanelParentChain, Vec<Panel>> = HashMap::new();
        for row in list {
            chains
                .entry(PanelParentChain(row.below))
                .or_default()
                .push(row.panel);
        }
        Ok(boxed(chains))
    }
}

impl Loader<EffectiveUsage> for DbLoader {
    type Value = PortUsage;
    type Error = ApiError;

    /// One query per plan.
    async fn load(
        &self,
        keys: &[EffectiveUsage],
    ) -> Result<HashMap<EffectiveUsage, PortUsage>, Self::Error> {
        let mut ports: HashMap<i32, Vec<i32>> = HashMap::new();
        for key in keys {
            ports.entry(key.plan).or_default().push(key.port);
        }
        let mut connection = self.connection.lock().await;
        let mut usages = HashMap::new();
        for (plan, ports) in ports {
            for usage in PortUsage::effective_of_ports(&mut connection, plan, &ports).await? {
                let key = EffectiveUsage {
                    plan,
                    port: usage.port_id,
                    side: usage.side,
                };
                usages.insert(key, usage);
            }
        }
        Ok(usages)
    }
}

impl Loader<PlanId> for DbLoader {
    type Value = Plan;
    type Error = ApiError;

    async fn load(&self, keys: &[PlanId]) -> Result<HashMap<PlanId, Plan>, Self::Error> {
        let mut connection = self.connection.lock().await;
        let list: Vec<Plan> = Plan::query()
            .filter(schema::plan::id.eq_any(ids(keys, |k| k.0)))
            .load(&mut connection)
            .await?;
        Ok(list.into_iter().map(|p| (PlanId(p.id), p)).collect())
    }
}
