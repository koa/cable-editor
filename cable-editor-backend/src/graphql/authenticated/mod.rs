pub mod planned;

use crate::db::{
    DB,
    entity::{
        Cable, Duct, InsertPanel, InsertPanelPort, PanelPortType, Schacht, SchachtTyp,
        UpdateCableChangeset,
    },
    schema::{kabel, kabel_trasse, panel, panel_port, schacht::id},
};
use async_graphql::{Context, EmptySubscription, Enum, InputObject, Object, Schema, Union};
use async_recursion::async_recursion;
use diesel::dsl::update;
use diesel::{
    AsChangeset, ExpressionMethods, HasQuery, OptionalExtension, QueryDsl, QueryResult, dsl::max,
};
use diesel_async::{
    AsyncConnection, AsyncPgConnection, RunQueryDsl,
    pooled_connection::deadpool::Object as DpObject,
};

pub type AuthenticatedGraphqlSchema = Schema<Query, Mutation, EmptySubscription>;

pub struct Query;

#[Object]
impl Query {
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
            .filter(id.eq(schacht_id))
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
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct CreatePanel {
    pub name: Option<String>,
    pub schacht_id: i32,
    pub children: Box<[CreatePanel]>,
}
#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct CreatePort {
    pub label: Option<String>,
    pub port_type: PanelPortType,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct PanelUpdate {
    panel_id: i32,
    name: Option<PanelUpdateSetName>,
    order: Option<PanelUpdateSetOrder>,
    parent: Option<PanelUpdateSetParent>,
}
#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct PanelUpdateSetName {
    value: Option<String>,
}
#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct PanelUpdateSetOrder {
    order: i32,
}
#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct PanelUpdateSetParent {
    parent: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct CreatePlan {
    pub name: String,
}

pub struct Mutation;

#[derive(InputObject)]
struct UpdateCableStructure {
    bundle_count: u32,
    fiber_count: u32,
}

#[Object]
impl Mutation {
    async fn create_cable(&self, ctx: &Context<'_>, name: String) -> async_graphql::Result<Cable> {
        let mut connection = get_connection(ctx).await?;
        Ok(diesel::insert_into(kabel::table)
            .values((
                kabel::name.eq(name),
                kabel::buendel_anz.eq(1),
                kabel::faser_anz.eq(12),
            ))
            .get_result::<Cable>(&mut connection)
            .await?)
    }
    async fn update_cable(
        &self,
        ctx: &Context<'_>,
        cable_id: i32,
        name: Option<String>,
        fibers: Option<UpdateCableStructure>,
        path: Option<Vec<i32>>,
    ) -> async_graphql::Result<Option<Cable>> {
        let mut connection = get_connection(ctx).await?;
        let (buendel_anz, faser_anz) = if let Some(UpdateCableStructure {
            bundle_count,
            fiber_count,
        }) = fibers
        {
            (Some(bundle_count as i32), Some(fiber_count as i32))
        } else {
            (None, None)
        };

        let changeset = UpdateCableChangeset {
            name,
            buendel_anz,
            faser_anz,
        };

        let updated_db_cable = connection
            .transaction(async move |conn| {
                if let Some(ref path_ids) = path {
                    diesel::delete(kabel_trasse::table.filter(kabel_trasse::kabel.eq(cable_id)))
                        .execute(conn)
                        .await?;

                    for (sequenz, &trasse_id) in path_ids.iter().enumerate() {
                        diesel::insert_into(kabel_trasse::table)
                            .values((
                                kabel_trasse::kabel.eq(cable_id),
                                kabel_trasse::trasse.eq(trasse_id),
                                kabel_trasse::sequenz.eq(sequenz as i32),
                            ))
                            .execute(conn)
                            .await?;
                    }
                }

                let updated = if changeset.any() {
                    diesel::update(kabel::table.find(cable_id))
                        .set(&changeset)
                        .get_result::<Cable>(conn)
                        .await
                        .optional()?
                } else {
                    kabel::table
                        .find(cable_id)
                        .first::<Cable>(conn)
                        .await
                        .optional()?
                };

                Ok::<Option<Cable>, diesel::result::Error>(updated)
            })
            .await?;

        Ok(updated_db_cable)
    }
    async fn delete_cable(&self, ctx: &Context<'_>, cable_id: i32) -> async_graphql::Result<bool> {
        get_connection(ctx)
            .await?
            .transaction(async move |conn| {
                diesel::delete(kabel_trasse::table.filter(kabel_trasse::kabel.eq(cable_id)))
                    .execute(conn)
                    .await?;
                diesel::delete(kabel::table.filter(kabel::id.eq(cable_id)))
                    .execute(conn)
                    .await?;
                Ok(true)
            })
            .await
    }
    async fn create_panel(
        &self,
        ctx: &Context<'_>,
        panel: CreatePanel,
        parent_panel: Option<i32>,
    ) -> async_graphql::Result<bool> {
        get_connection(ctx)
            .await?
            .transaction(async move |conn| {
                let parent_order = if let Some(parent_id) = parent_panel {
                    let max_order: Option<i32> = panel::table
                        .filter(panel::parent_panel.eq(parent_id))
                        .select(max(panel::parent_order))
                        .first(conn)
                        .await?;
                    Some(max_order.unwrap_or(0) + 1)
                } else {
                    None
                };
                insert_panel_tree_recursive(conn, panel, parent_panel, parent_order).await?;
                Ok(true)
            })
            .await
    }
    async fn update_panels(
        &self,
        ctx: &Context<'_>,
        updates: Vec<PanelUpdate>,
    ) -> async_graphql::Result<bool> {
        get_connection(ctx)
            .await?
            .transaction(async move |conn| {
                for PanelUpdate {
                    panel_id,
                    name,
                    order,
                    parent,
                } in updates
                {
                    diesel::update(panel::table)
                        .filter(panel::id.eq(panel_id))
                        .set(UpdatePanelChangeset {
                            name: name.map(|n| n.value),
                            parent_panel: order.map(|o| Some(o.order)),
                            parent_order: parent.map(|p| p.parent),
                        })
                        .execute(conn)
                        .await?;
                }

                Ok(true)
            })
            .await
    }
    async fn create_plan(
        &self,
        ctx: &Context<'_>,
        plan: CreatePlan,
    ) -> async_graphql::Result<bool> {
        let mut connection = get_connection(ctx).await?;
        let new_plan = InsertPlan { name: plan.name };
        diesel::insert_into(plan::table)
            .values(new_plan)
            .execute(&mut connection)
            .await?;
        Ok(true)
    }
}
#[derive(AsChangeset)]
#[diesel(table_name = panel)]
struct UpdatePanelChangeset {
    name: Option<Option<String>>,
    parent_panel: Option<Option<i32>>,
    parent_order: Option<Option<i32>>,
}

#[async_recursion]
async fn insert_panel_tree_recursive(
    conn: &mut AsyncPgConnection,
    node: CreatePanel,
    parent_id: Option<i32>,
    parent_order_val: Option<i32>,
) -> async_graphql::Result<()> {
    // 1. Das aktuelle Panel speichern
    let new_panel = InsertPanel {
        name: node.name.clone(),
        schacht_id: node.schacht_id,
        parent_panel: parent_id,
        parent_order: parent_order_val,
    };

    let inserted_panel_id: i32 = diesel::insert_into(panel::table)
        .values(new_panel)
        .returning(panel::id)
        .get_result(conn)
        .await?;

    // 2. Rekursiv alle Kinder dieses Panels speichern
    for (index, child) in node.children.into_iter().enumerate() {
        insert_panel_tree_recursive(
            conn,
            child,
            Some(inserted_panel_id),
            Some((index + 1) as i32), // parent_order startet bei 1
        )
        .await?;
    }
    Ok(())
}

pub fn create_authenticated_schema() -> AuthenticatedGraphqlSchema {
    Schema::build(Query, Mutation, EmptySubscription).finish()
}

pub async fn get_connection(
    ctx: &Context<'_>,
) -> async_graphql::Result<DpObject<AsyncPgConnection>> {
    let db = ctx.data::<DB>()?;
    Ok(db.get().await?)
}

use crate::db::entity::{FiberPathNode, InsertPlan, Plan};
use crate::db::schema::plan;
use diesel::sql_query;
use diesel::sql_types::Integer;

pub async fn trace_fiber_path(
    conn: &mut AsyncPgConnection,
    start_panel_id: i32,
    start_port_number: i32,
) -> QueryResult<Vec<FiberPathNode>> {
    let raw_sql = r#"
    WITH RECURSIVE
    -- Das Overlay: Ist-Zustand (0) und EINE Planung ($3) mischen
    overlay_ports AS (
        SELECT DISTINCT ON (panel_id, port_number)
            panel_id, port_number,
            f1_kabel_id, f1_buendel, f1_faser,
            f2_kabel_id, f2_buendel, f2_faser
        FROM panel_port
        WHERE plan_id IN (0, $3) -- 0 für Ist-Zustand, $3 für die gewählte Planung
        -- plan_id DESC überschreibt den Ist-Zustand (0) mit der Planung (>0)
        ORDER BY panel_id, port_number, plan_id DESC
    ),
    -- 1. Diese CTE ist nur ein "Makro", sie lädt NICHT die ganze Tabelle
    endpoints AS (
        SELECT panel_id, port_number, f1_kabel_id AS k_id, f1_buendel AS b, f1_faser AS f
        FROM overlay_ports WHERE f1_kabel_id IS NOT NULL
    ),

    signal_path AS (
        -- 2. Basisfall: Wir filtern DIREKT HIER am Startpunkt ($1 und $2)
        SELECT
            1 AS step,
            e1.panel_id AS from_panel, e1.port_number AS from_port,
            e2.panel_id AS to_panel, e2.port_number AS to_port,
            e1.k_id AS kabel, e1.b AS buendel, e1.f AS faser,
            ARRAY[e1.panel_id::text || '-' || e1.port_number::text] AS visited
        FROM endpoints e1
        JOIN endpoints e2
          ON e1.k_id = e2.k_id AND e1.b = e2.b AND e1.f = e2.f
         AND (e1.panel_id != e2.panel_id OR e1.port_number != e2.port_number)
        WHERE e1.panel_id = $1 AND e1.port_number = $2

        UNION ALL

        -- 3. Rekursion: Sucht nur iterativ die exakten Folge-Ports
        SELECT
            sp.step + 1,
            e1.panel_id, e1.port_number,
            e2.panel_id, e2.port_number,
            e1.k_id, e1.b, e1.f,
            sp.visited || (e1.panel_id::text || '-' || e1.port_number::text)
        FROM signal_path sp
        -- Vom Ziel des letzten Schritts...
        JOIN endpoints e1 ON e1.panel_id = sp.to_panel AND e1.port_number = sp.to_port
        -- ...auf das andere Ende der verbundenen Faser springen
        JOIN endpoints e2 ON e1.k_id = e2.k_id AND e1.b = e2.b AND e1.f = e2.f
         AND (e1.panel_id != e2.panel_id OR e1.port_number != e2.port_number)
        WHERE NOT ((e2.panel_id::text || '-' || e2.port_number::text) = ANY(sp.visited))
    )
    SELECT step, from_panel, from_port, to_panel, to_port, kabel, buendel, faser
    FROM signal_path
    ORDER BY step;
"#;

    sql_query(raw_sql)
        .bind::<Integer, _>(start_panel_id)
        .bind::<Integer, _>(start_port_number)
        .load::<FiberPathNode>(conn)
        .await
}
