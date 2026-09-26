pub mod mutation;
pub mod planned;

use crate::{
    db::{
        entity::{
            Duct, FiberPathNode,
            cable::Cable,
            panel::Panel,
            plan::Plan,
            schacht::{Schacht, SchachtTyp},
        },
        schema::{kabel, panel, plan, schacht},
    },
    graphql::{
        context::UserInfo,
        geo::{self, ConvertedPoint, PositionInput},
        loader::SharedConnection,
    },
    netbox::{fetch::DeviceWithRearPorts, fetch_device_with_ports, fetch_devices_and_ports},
};
use async_graphql::{Context, EmptySubscription, Object, Schema};
use diesel::{
    ExpressionMethods, HasQuery, OptionalExtension, QueryDsl, QueryResult, sql_query,
    sql_types::Integer,
};
use diesel_async::{
    AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Object as DpObject,
};
use mutation::Mutation;
use tokio::sync::MutexGuard;

pub type AuthenticatedGraphqlSchema = Schema<Query, Mutation, EmptySubscription>;

pub struct Query;

#[Object]
impl Query {
    async fn current_user<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> Result<&'a UserInfo, async_graphql::Error> {
        ctx.data::<UserInfo>()
    }
    async fn list_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Schacht>> {
        //let mut connection = pool.get().await?;
        let mut connection = get_connection(ctx).await?;
        let query = Schacht::query();
        let list = query.load(&mut connection).await?;
        Ok(list)
    }
    async fn schacht(
        &self,
        ctx: &Context<'_>,
        schacht_id: i32,
    ) -> async_graphql::Result<Option<Schacht>> {
        let mut connection = get_connection(ctx).await?;

        let schacht = Schacht::query()
            .filter(schacht::id.eq(schacht_id))
            .first(&mut connection)
            .await
            .optional()?;
        Ok(schacht)
    }
    async fn list_schacht_typ(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<SchachtTyp>> {
        let mut connection = get_connection(ctx).await?;
        let query = SchachtTyp::query();
        let list = query.load(&mut connection).await?;
        Ok(list)
    }
    async fn list_cable(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Cable>> {
        let mut connection = get_connection(ctx).await?;
        let query = Cable::query();
        Ok(query.load(&mut connection).await?)
    }
    async fn cable(
        &self,
        ctx: &Context<'_>,
        cable_id: u32,
    ) -> async_graphql::Result<Option<Cable>> {
        let mut connection = get_connection(ctx).await?;
        Ok(kabel::table
            .find(cable_id as i32)
            .first::<Cable>(&mut connection)
            .await
            .optional()?)
    }
    async fn list_duct(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Duct>> {
        let mut connection = get_connection(ctx).await?;
        let query = Duct::query();
        Ok(query.load(&mut connection).await?)
    }
    async fn list_plan(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Plan>> {
        let mut connection = get_connection(ctx).await?;
        let query = Plan::query();
        Ok(query.load(&mut connection).await?)
    }
    async fn plan(&self, ctx: &Context<'_>, plan_id: i32) -> async_graphql::Result<Option<Plan>> {
        let mut connection = get_connection(ctx).await?;
        Ok(plan::table
            .find(plan_id)
            .first::<Plan>(&mut connection)
            .await
            .optional()?)
    }
    async fn panel(
        &self,
        ctx: &Context<'_>,
        panel_id: i32,
    ) -> async_graphql::Result<Option<Panel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(panel::table
            .find(panel_id)
            .first(&mut connection)
            .await
            .optional()?)
    }
    /// A position in LV95 and WGS84, e.g. to preview a typed position on the map.
    async fn convert_point(
        &self,
        ctx: &Context<'_>,
        position: PositionInput,
    ) -> async_graphql::Result<ConvertedPoint> {
        let mut connection = get_connection(ctx).await?;
        geo::convert(&mut connection, position).await
    }
    async fn netbox_devices(&self) -> async_graphql::Result<Vec<DeviceWithRearPorts>> {
        Ok(fetch_devices_and_ports().await?)
    }
    async fn netbox_device(
        &self,
        netbox_device_id: u32,
    ) -> async_graphql::Result<Option<DeviceWithRearPorts>> {
        Ok(fetch_device_with_ports(netbox_device_id.into()).await?)
    }
}

pub fn create_authenticated_schema() -> AuthenticatedGraphqlSchema {
    Schema::build(Query, Mutation, EmptySubscription).finish()
}

pub async fn get_connection<'a>(
    ctx: &'a Context<'_>,
) -> async_graphql::Result<MutexGuard<'a, DpObject<AsyncPgConnection>>> {
    let shared_conn = ctx.data::<SharedConnection>()?;
    Ok(shared_conn.lock().await)
}

pub async fn trace_fiber_path(
    conn: &mut AsyncPgConnection,
    start_port_id: i32,
    plan_id: i32,
) -> QueryResult<Vec<FiberPathNode>> {
    let raw_sql = r#"
    WITH RECURSIVE
    -- 1. Effektiven Zustand berechnen: Ist-Zustand (0) und EINE Planung ($2) mischen
    effective_usage AS (
        SELECT DISTINCT ON (port_id, side)
            port_id, side, cable, bundle, fiber
        FROM port_usage
        WHERE plan_id IN (0, $2)
        -- plan_id DESC überschreibt den Ist-Zustand (0) mit der Planung (>0)
        ORDER BY port_id, side, plan_id DESC
    ),
    -- 2. Tombstones (cable IS NULL) herausfiltern (JOIN mit panel_port fällt komplett weg!)
    endpoints AS (
        SELECT
            port_id, cable AS k_id, bundle AS b, fiber AS f
        FROM effective_usage
        WHERE cable IS NOT NULL
    ),
    -- 3. Die eigentliche Wegfindung
    signal_path AS (
        -- Basisfall: Direkter Startpunkt (gefiltert auf $1)
        SELECT
            1 AS step,
            e1.port_id AS from_port_id,
            e2.port_id AS to_port_id,
            e1.k_id AS kabel, e1.b AS buendel, e1.f AS faser,
            -- Array merkt sich besuchte ports anstatt panel/port kombinationen
            ARRAY[e1.port_id] AS visited
        FROM endpoints e1
        JOIN endpoints e2
          ON e1.k_id = e2.k_id AND e1.b = e2.b AND e1.f = e2.f
         AND e1.port_id != e2.port_id -- Faser muss auf einen ANDEREN Port springen
        WHERE e1.port_id = $1
        UNION ALL
        -- Rekursion: Springt iterativ die Folge-Ports ab
        SELECT
            sp.step + 1,
            e1.port_id, 
            e2.port_id,
            e1.k_id, e1.b, e1.f,
            sp.visited || e1.port_id
        FROM signal_path sp
        -- Vom Ziel des letzten Schritts auf den Eingang des neuen Ports...
        JOIN endpoints e1 ON e1.port_id = sp.to_port_id
        -- ...auf das andere Ende der verbundenen Faser springen
        JOIN endpoints e2 ON e1.k_id = e2.k_id AND e1.b = e2.b AND e1.f = e2.f
         AND e1.port_id != e2.port_id
        -- Verhindern, dass wir im Kreis laufen
        WHERE NOT (e2.port_id = ANY(sp.visited))
    )
    SELECT step, from_port_id, to_port_id, kabel, buendel, faser
    FROM signal_path
    ORDER BY step;
    "#;

    sql_query(raw_sql)
        .bind::<Integer, _>(start_port_id)
        .bind::<Integer, _>(plan_id)
        .load::<FiberPathNode>(conn)
        .await
}
