use crate::db::entity::plan::BASELINE_PLAN_ID;
use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::{cable::Fiber, plan::Plan, schacht::Schacht},
        schema,
    },
    graphql::{
        authenticated::get_connection,
        loader::{
            EffectiveUsage, PanelId, PanelPortId, PanelPorts, PlanId, SchachtId, get_loader,
            load_one,
        },
    },
    netbox::{
        fetch::{DeviceWithRearPorts, RearPort},
        fetch_device_with_ports,
        id::NumberId,
    },
};
use async_graphql::{Context, Enum, Object};
use async_recursion::async_recursion;
use diesel::{
    Associations, ExpressionMethods, HasQuery, Identifiable, Insertable, OptionalExtension,
    QueryDsl, QueryResult, QueryableByName,
    pg::Pg,
    sql_query,
    sql_types::{Array, Bool, Integer, Nullable},
};
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool};
use diesel_derive_enum::DbEnum;
use std::borrow::Cow;

#[derive(QueryableByName, Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schema::panel)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Panel {
    pub id: i32,
    pub name: Option<String>,
    pub schacht_id: i32,
    pub parent_panel: Option<i32>,
    pub parent_order: Option<i32>,
    pub netbox_device_id: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = schema::panel)]
pub struct InsertPanel {
    pub name: Option<String>,
    pub schacht_id: i32,
    pub parent_panel: Option<i32>,
    pub parent_order: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = schema::panel_port)]
pub struct InsertPanelPort {
    pub panel_id: i32,
    pub port_order: i32,
    pub port_type: PanelPortType,
    pub label: Option<String>,
    pub netbox_port_id: Option<i32>,
}

#[derive(
    Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq, Hash, PartialOrd, Ord, Eq,
)]
#[diesel(table_name = schema::panel_port)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(id))]
pub struct PanelPort {
    pub id: i32,
    pub panel_id: i32,
    pub port_order: i32,
    pub label: Option<String>,
    pub port_type: PanelPortType,
    pub netbox_port_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Copy, Eq, DbEnum, Enum, Hash, PartialOrd, Ord)]
#[ExistingTypePath = "crate::db::schema::sql_types::PortSideEnum"]
pub enum PortSide {
    #[db_rename = "Front"]
    Front,
    #[db_rename = "Back"]
    Back,
}

impl PortSide {
    pub fn other(self) -> PortSide {
        match self {
            PortSide::Front => PortSide::Back,
            PortSide::Back => PortSide::Front,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Copy, Eq, DbEnum, Enum, Hash, PartialOrd, Ord)]
#[ExistingTypePath = "crate::db::schema::sql_types::PortTypeEnum"]
pub enum PanelPortType {
    #[db_rename = "Splice"]
    Splice,
    #[db_rename = "Connector"]
    Connector,
    #[db_rename = "Loop"]
    Loop,
}

#[derive(
    Identifiable, Insertable, HasQuery, Associations, Debug, Copy, Clone, PartialEq, QueryableByName,
)]
#[diesel(table_name = schema::port_usage)]
#[diesel(primary_key(port_id, plan_id, side))]
#[diesel(belongs_to(PanelPort, foreign_key = port_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PortUsage {
    pub port_id: i32,
    pub plan_id: i32,
    pub side: PortSide,
    pub cable: Option<i32>,
    pub fiber: Option<i32>,
    pub bundle: Option<i32>,
}
impl PortUsage {
    /// The cable and fiber, if used (a row of a plan removing the fiber has none)
    pub fn used_fiber(&self) -> Option<Fiber> {
        if let (Some(cable), Some(bundle), Some(fiber)) = (self.cable, self.bundle, self.fiber) {
            Some(Fiber {
                cable,
                bundle,
                fiber,
            })
        } else {
            None
        }
    }
    /// What the port's side holds in the plan (`effective_port_usage`)
    pub async fn effective(
        connection: &mut AsyncPgConnection,
        plan_id: i32,
        port_id: i32,
        side: PortSide,
    ) -> QueryResult<Option<PortUsage>> {
        sql_query("select * from effective_port_usage($1) where port_id = $2 and side = $3")
            .bind::<Integer, _>(plan_id)
            .bind::<Integer, _>(port_id)
            .bind::<schema::sql_types::PortSideEnum, _>(side)
            .get_result(connection)
            .await
            .optional()
    }
    /// What the ports hold in the plan, both sides (`effective_port_usage`)
    pub async fn effective_of_ports(
        connection: &mut AsyncPgConnection,
        plan_id: i32,
        port_ids: &[i32],
    ) -> QueryResult<Vec<PortUsage>> {
        sql_query("select * from effective_port_usage($1) where port_id = any($2)")
            .bind::<Integer, _>(plan_id)
            .bind::<Array<Integer>, _>(port_ids)
            .load(connection)
            .await
    }
    /// The usages a signal reaches from this one in the plan, in order: along the fiber to the
    /// next port and through a port to its other side, alternately, `first` first. It stops
    /// before a usage it reached already (`Trace::looped`).
    pub async fn trace(
        &self,
        connection: &mut AsyncPgConnection,
        plan_id: i32,
        first: FirstHop,
    ) -> QueryResult<Trace> {
        // A port's side is `port_id * 2 + 1` for the back, `port_id * 2` for the front
        let steps = sql_query(
            r#"
            WITH RECURSIVE walk AS (
                SELECT 0 AS step, $2::int4 AS port_id, $3 AS side, $4::int4 AS cable,
                       $5::int4 AS bundle, $6::int4 AS fiber, $7::int4 AS plan_id,
                       false AS closed, ARRAY[$2 * 2 + ($3 = 'Back')::int4] AS visited
                UNION ALL
                SELECT w.step + 1, e.port_id, e.side, e.cable, e.bundle, e.fiber, e.plan_id,
                       e.port_id * 2 + (e.side = 'Back')::int4 = ANY (w.visited),
                       w.visited || e.port_id * 2 + (e.side = 'Back')::int4
                FROM walk w
                JOIN effective_port_usage($1) e
                  ON CASE WHEN (w.step + $8) % 2 = 0
                          -- along the fiber to another port
                          THEN e.cable = w.cable AND e.bundle = w.bundle AND e.fiber = w.fiber
                               AND e.port_id <> w.port_id
                          -- through the port to its other side
                          ELSE e.port_id = w.port_id AND e.side <> w.side
                     END
                WHERE NOT w.closed
            )
            SELECT port_id, side, cable, bundle, fiber, plan_id, closed
            FROM walk
            ORDER BY step, port_id, side
            "#,
        )
        .bind::<Integer, _>(plan_id)
        .bind::<Integer, _>(self.port_id)
        .bind::<schema::sql_types::PortSideEnum, _>(self.side)
        .bind::<Nullable<Integer>, _>(self.cable)
        .bind::<Nullable<Integer>, _>(self.bundle)
        .bind::<Nullable<Integer>, _>(self.fiber)
        .bind::<Integer, _>(self.plan_id)
        .bind::<Integer, _>(match first {
            FirstHop::Fiber => 0,
            FirstHop::Port => 1,
        })
        .load::<TraceStep>(connection)
        .await?;
        let looped = steps.last().is_some_and(|step| step.closed);
        Ok(Trace {
            usages: steps
                .into_iter()
                .filter(|step| !step.closed)
                .map(|step| step.usage)
                .collect(),
            looped,
        })
    }
}

/// Where `PortUsage::trace` goes first: along the fiber or through the port
pub enum FirstHop {
    Fiber,
    Port,
}

/// The usages a signal reaches, from the one it started at
pub struct Trace {
    pub usages: Box<[PortUsage]>,
    /// It came back to a usage it reached already
    pub looped: bool,
}

impl Trace {
    /// The last usage reached, none if it runs in a circle
    pub fn end(&self) -> Option<PortUsage> {
        if self.looped {
            None
        } else {
            self.usages.last().copied()
        }
    }
}

#[derive(QueryableByName)]
struct TraceStep {
    #[diesel(embed)]
    usage: PortUsage,
    /// The usage was reached already
    #[diesel(sql_type = Bool)]
    closed: bool,
}

#[Object]
impl PortUsage {
    async fn side(&self) -> PortSide {
        self.side
    }
    /// The cable and fiber, if used (a row of a plan removing the fiber has none)
    async fn fiber(&self) -> Option<Fiber> {
        self.used_fiber()
    }
    async fn port(&self, ctx: &Context<'_>) -> ApiResult<PanelPort> {
        load_one(ctx, PanelPortId(self.port_id)).await
    }
    async fn plan(&self, ctx: &Context<'_>) -> ApiResult<Plan> {
        load_one(ctx, PlanId(self.plan_id)).await
    }
    /// What the other side of the port holds in the plan
    async fn other_side(&self, ctx: &Context<'_>, plan_id: i32) -> ApiResult<Option<PortUsage>> {
        Ok(get_loader(ctx)?
            .load_one(EffectiveUsage {
                plan: plan_id,
                port: self.port_id,
                side: self.side.other(),
            })
            .await?)
    }
    async fn modified_in_plan(&self) -> bool {
        self.plan_id != BASELINE_PLAN_ID
    }
    /// Where the signal ends following the fiber into the cable, none if it runs in a circle
    async fn cable_side_end_port(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
    ) -> ApiResult<Option<PortUsage>> {
        let mut connection = get_connection(ctx).await?;
        Ok(self
            .trace(&mut connection, plan_id, FirstHop::Fiber)
            .await?
            .end())
    }
    /// Where the signal ends going through the port first, none if it runs in a circle
    async fn panel_side_end_port(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
    ) -> ApiResult<Option<PortUsage>> {
        let mut connection = get_connection(ctx).await?;
        Ok(self
            .trace(&mut connection, plan_id, FirstHop::Port)
            .await?
            .end())
    }
}

impl Panel {
    /// All panels below `panel_id`, level by level, each level in `parent_order`.
    pub async fn load_all_children_recursive(
        panel_id: i32,
        connection: &mut deadpool::Object<AsyncPgConnection>,
    ) -> Result<Vec<Panel>, diesel::result::Error> {
        let raw_sql = r#"
        WITH RECURSIVE panel_tree AS (
            SELECT
                id, name, schacht_id, parent_panel, parent_order, netbox_device_id,
                1 as level
            FROM panel
            WHERE parent_panel = $1

            UNION ALL

            SELECT
                p.id, p.name, p.schacht_id, p.parent_panel, p.parent_order, p.netbox_device_id,
                pt.level + 1 as level
            FROM panel p
            INNER JOIN panel_tree pt ON p.parent_panel = pt.id
        )
        SELECT
            id, name, schacht_id, parent_panel, parent_order, netbox_device_id
        FROM panel_tree
        ORDER BY level, parent_order;
        "#;
        sql_query(raw_sql)
            .bind::<Integer, _>(panel_id)
            .load::<Panel>(connection)
            .await
    }
}

#[Object]
impl Panel {
    async fn id(&self) -> i32 {
        self.id
    }
    async fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    async fn schacht(&self, ctx: &Context<'_>) -> ApiResult<Schacht> {
        load_one(ctx, SchachtId(self.schacht_id)).await
    }
    async fn parent_id(&self) -> Option<i32> {
        self.parent_panel
    }
    async fn parent_order(&self) -> Option<i32> {
        self.parent_order
    }
    async fn parent(&self, ctx: &Context<'_>) -> ApiResult<Option<Panel>> {
        if let Some(parent_panel_id) = self.parent_panel {
            let mut connection = get_connection(ctx).await?;
            Ok(Some(
                Panel::query()
                    .filter(schema::panel::id.eq(parent_panel_id))
                    .first(&mut connection)
                    .await?,
            ))
        } else {
            Ok(None)
        }
    }
    async fn parent_chain(&self, ctx: &Context<'_>) -> ApiResult<Vec<Panel>> {
        #[async_recursion]
        async fn fetch_parent_chain(
            transaction: &mut deadpool::Object<AsyncPgConnection>,
            entry: &Panel,
            parents: &mut Vec<Panel>,
        ) -> Result<(), diesel::result::Error> {
            if let Some(parent_panel_id) = entry.parent_panel {
                let parent_panel = Panel::query()
                    .filter(schema::panel::id.eq(parent_panel_id))
                    .first(transaction)
                    .await?;
                fetch_parent_chain(transaction, &parent_panel, parents).await?;
                parents.push(parent_panel);
            }
            Ok(())
        }
        if self.parent_panel.is_some() {
            let mut connection = get_connection(ctx).await?;
            let mut result = Vec::new();
            fetch_parent_chain(&mut connection, self, &mut result).await?;
            Ok(result)
        } else {
            Ok(Vec::default())
        }
    }
    async fn children(&self, ctx: &Context<'_>) -> ApiResult<Vec<Panel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Panel::query()
            .filter(schema::panel::parent_panel.eq(self.id))
            .order(schema::panel::parent_order.asc())
            .load(&mut connection)
            .await?)
    }
    async fn siblings(&self, ctx: &Context<'_>) -> ApiResult<Vec<Panel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(if let Some(parent_id) = self.parent_panel {
            Panel::query()
                .filter(schema::panel::parent_panel.eq(parent_id))
                .filter(schema::panel::id.ne(self.id))
                .order(schema::panel::parent_order.asc())
                .load(&mut connection)
                .await?
        } else {
            Panel::query()
                .filter(schema::panel::parent_panel.is_null())
                .filter(schema::panel::schacht_id.eq(self.schacht_id))
                .filter(schema::panel::id.ne(self.id))
                .order(schema::panel::parent_order.asc())
                .load(&mut connection)
                .await?
        })
    }
    async fn all_children_recursive(&self, ctx: &Context<'_>) -> ApiResult<Vec<Panel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Panel::load_all_children_recursive(self.id, &mut connection).await?)
    }
    async fn ports(
        &self,
        ctx: &Context<'_>,
        port_type: Option<PanelPortType>,
    ) -> ApiResult<Box<[PanelPort]>> {
        let ports = get_loader(ctx)?
            .load_one(PanelPorts(self.id))
            .await?
            .unwrap_or_default();
        Ok(ports
            .into_iter()
            .filter(|port| port_type.is_none_or(|port_type| port.port_type == port_type))
            .collect())
    }
    async fn count_ports(
        &self,
        ctx: &Context<'_>,
        port_type: Option<PanelPortType>,
    ) -> ApiResult<i64> {
        let mut connection = get_connection(ctx).await?;
        let statement =
            <PanelPort as HasQuery<Pg>>::query().filter(schema::panel_port::panel_id.eq(self.id));
        Ok(if let Some(pt) = port_type {
            statement
                .filter(schema::panel_port::port_type.eq(pt))
                .count()
                .get_result(&mut connection)
                .await?
        } else {
            statement.count().get_result(&mut connection).await?
        })
    }
    async fn netbox_device(&self) -> ApiResult<Option<DeviceWithRearPorts>> {
        Ok(if let Some(device_id) = self.netbox_device_id {
            fetch_device_with_ports((device_id as u32).into())
                .await
                .ok()
                .flatten()
        } else {
            None
        })
    }
}

impl PanelPort {
    pub fn create_label(&self) -> Cow<'_, str> {
        if let Some(name) = self.label.as_deref() {
            Cow::Borrowed(name)
        } else {
            Cow::Owned(format!("Port {}", self.id))
        }
    }
    pub async fn rear_port_from_netbox(
        id: NumberId,
        ctx: &Context<'_>,
    ) -> ApiResult<Vec<PanelPort>> {
        let mut connection = get_connection(ctx).await?;
        let id: u32 = id.into();
        let filter = schema::panel_port::netbox_port_id.eq(id as i32);
        Ok(PanelPort::query()
            .filter(filter)
            .load(&mut connection)
            .await?)
    }
}

#[Object]
impl PanelPort {
    async fn id(&self) -> i32 {
        self.id
    }
    async fn order_number(&self) -> i32 {
        self.port_order
    }
    async fn panel(&self, ctx: &Context<'_>) -> ApiResult<Panel> {
        load_one(ctx, PanelId(self.panel_id)).await
    }
    async fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    /*async fn connected_fibers(
        &self,
        ctx: &Context<'_>,
    ) -> ApiResult<Vec<FiberPathSegment>> {
        let mut connection = get_connection(ctx).await?;
        connection
            .transaction(async move |conn| {
                let mut path = Vec::with_capacity(2);
                for fiber in self.fibers() {
                    PanelPort::query()
                        .filter(
                            ((panel_port::f1_faser
                                .eq(fiber.fiber)
                                .and(panel_port::f1_buendel.eq(fiber.bundle))
                                .and(panel_port::f1_kabel_id.eq(fiber.cable)))
                            .or(panel_port::f2_faser
                                .eq(fiber.fiber)
                                .and(panel_port::f2_buendel.eq(fiber.bundle))
                                .and(panel_port::f2_kabel_id.eq(fiber.cable))))
                            .and(not(panel_port::panel_id
                                .eq(self.panel_id)
                                .and(panel_port::port_number.eq(self.port_number)))),
                        )
                        .load::<PanelPort>(conn)
                        .await?
                        .into_iter()
                        .map(|next_port| FiberPathSegment { fiber, next_port })
                        .for_each(|segment| path.push(segment));
                }
                Ok(path)
            })
            .await
    }*/
    async fn port_type(&self) -> PanelPortType {
        self.port_type
    }

    async fn netbox_port(&self) -> ApiResult<Option<RearPort>> {
        if let Some(netbox_port_id) = self.netbox_port_id {
            Ok(RearPort::fetch_by_id((netbox_port_id as u32).into())
                .await
                .ok()
                .flatten())
        } else {
            Ok(None)
        }
    }
}

#[derive(Copy, Clone, PartialEq, Hash, Ord, PartialOrd, Eq, Debug)]
pub struct PortId {
    pub panel_id: i32,
    pub port_number: i32,
}
