use crate::{
    db::entity::plan::Plan,
    db::{
        entity::{cable::Fiber, schacht::Schacht},
        schema,
    },
    graphql::authenticated::get_connection,
};
use async_graphql::{Context, Enum, Object};
use async_recursion::async_recursion;
use diesel::{
    Associations, BoolExpressionMethods, ExpressionMethods, HasQuery, Identifiable, Insertable,
    OptionalExtension, QueryDsl, QueryableByName, pg::Pg, sql_query, sql_types::Integer,
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool};
use diesel_derive_enum::DbEnum;
use log::{error, info};
use std::collections::HashSet;

#[derive(QueryableByName, Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schema::panel)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Panel {
    pub id: i32,
    pub name: Option<String>,
    pub schacht_id: i32,
    pub parent_panel: Option<i32>,
    pub parent_order: Option<i32>,
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
    pub async fn other_side_of_port(
        &self,
        plan_id: i32,
        connection: &mut deadpool::Object<AsyncPgConnection>,
    ) -> Result<Option<PortUsage>, diesel::result::Error> {
        PortUsage::query()
            .filter(schema::port_usage::port_id.eq(self.port_id))
            .filter(schema::port_usage::side.eq(self.side.other()))
            .filter(schema::port_usage::plan_id.eq_any([0, plan_id]))
            .order(schema::port_usage::plan_id.desc())
            .first(connection)
            .await
            .optional()
    }
    pub async fn other_side_of_fiber(
        &self,
        plan_id: i32,
        connection: &mut deadpool::Object<AsyncPgConnection>,
    ) -> Result<Option<PortUsage>, diesel::result::Error> {
        if let (Some(cable), Some(fiber), Some(bundle)) = (self.cable, self.fiber, self.bundle) {
            PortUsage::query()
                .filter(schema::port_usage::port_id.ne(self.port_id))
                .filter(schema::port_usage::cable.eq(cable))
                .filter(schema::port_usage::bundle.eq(bundle))
                .filter(schema::port_usage::fiber.eq(fiber))
                .filter(schema::port_usage::plan_id.eq_any([0, plan_id]))
                .order(schema::port_usage::plan_id.desc())
                .first(connection)
                .await
                .optional()
        } else {
            Ok(None)
        }
    }
}

#[Object]
impl PortUsage {
    async fn side(&self) -> PortSide {
        self.side
    }
    // Löst das Kabel/die Faser auf, falls belegt (Tombstones haben hier None)
    async fn fiber(&self) -> Option<Fiber> {
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
    async fn port(&self, ctx: &Context<'_>) -> async_graphql::Result<PanelPort> {
        let mut connection = get_connection(ctx).await?;
        Ok(PanelPort::query()
            .filter(schema::panel_port::id.eq(self.port_id))
            .first(&mut connection)
            .await?)
    }
    async fn plan(&self, ctx: &Context<'_>) -> async_graphql::Result<Plan> {
        let mut connection = get_connection(ctx).await?;
        Ok(Plan::query()
            .filter(schema::plan::id.eq(self.plan_id))
            .first(&mut connection)
            .await?)
    }
    async fn other_side(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<PortUsage>> {
        let mut connection = get_connection(ctx).await?;
        Ok(PortUsage::query()
            .filter(
                schema::port_usage::port_id
                    .eq(self.port_id)
                    .and(schema::port_usage::side.eq(self.side.other()))
                    .and(schema::port_usage::plan_id.eq_any([0, self.plan_id])),
            )
            .order(schema::port_usage::plan_id.desc())
            .first(&mut connection)
            .await
            .optional()?
            .filter(|pu| pu.cable.is_some() && pu.bundle.is_some() && pu.fiber.is_some()))
    }
    async fn modified_in_plan(&self) -> bool {
        self.plan_id > 0
    }
    async fn cable_side_end_port(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
    ) -> async_graphql::Result<Option<PortUsage>> {
        let mut connection = get_connection(ctx).await?;

        // Helper function to trace fiber through port usages recursively
        #[async_recursion]
        async fn trace_fiber_to_end<'a>(
            connection: &mut deadpool::Object<AsyncPgConnection>,
            usage: &PortUsage,
            plan_id: i32,
            visited: &mut HashSet<(i32, PortSide)>,
        ) -> async_graphql::Result<Option<PortUsage>> {
            // Avoid infinite loops
            if !visited.insert((usage.port_id, usage.side)) {
                error!("Loop detected");
                return Ok(None);
            }
            let option = usage.other_side_of_fiber(plan_id, connection).await?;
            info!("Other side of fiber: {option:?}");
            let other_side_of_fiber_port = match option {
                None => return Ok(Some(*usage)),
                Some(p) => p,
            };
            let next_fiber_start_port = match other_side_of_fiber_port
                .other_side_of_port(plan_id, connection)
                .await?
            {
                None => {
                    return Ok(Some(other_side_of_fiber_port));
                }
                Some(p) => p,
            };

            trace_fiber_to_end(connection, &next_fiber_start_port, plan_id, visited).await
        }

        connection
            .transaction(async move |conn| {
                let mut visited = HashSet::new();
                trace_fiber_to_end(conn, self, plan_id, &mut visited).await
            })
            .await
    }
    async fn panel_side_end_port(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
    ) -> async_graphql::Result<Option<PortUsage>> {
        let mut connection = get_connection(ctx).await?;

        // Helper function to trace fiber through port usages recursively
        #[async_recursion]
        async fn trace_fiber_to_end<'a>(
            connection: &mut deadpool::Object<AsyncPgConnection>,
            usage: &PortUsage,
            plan_id: i32,
            visited: &mut HashSet<(i32, PortSide)>,
        ) -> async_graphql::Result<Option<PortUsage>> {
            // Avoid infinite loops
            if !visited.insert((usage.port_id, usage.side)) {
                error!("Loop detected");
                return Ok(None);
            }
            let other_side_of_panel_port =
                match usage.other_side_of_port(plan_id, connection).await? {
                    None => return Ok(Some(*usage)),
                    Some(p) => p,
                };
            let next_fiber_start_port = match other_side_of_panel_port
                .other_side_of_fiber(plan_id, connection)
                .await?
            {
                None => {
                    return Ok(Some(other_side_of_panel_port));
                }
                Some(p) => p,
            };

            trace_fiber_to_end(connection, &next_fiber_start_port, plan_id, visited).await
        }

        connection
            .transaction(async move |conn| {
                let mut visited = HashSet::new();
                trace_fiber_to_end(conn, self, plan_id, &mut visited).await
            })
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
    async fn schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        let mut connection = get_connection(ctx).await?;
        let schacht = Schacht::query()
            .filter(schema::schacht::id.eq(self.schacht_id))
            .first(&mut connection)
            .await?;
        Ok(schacht)
    }
    async fn parent_id(&self) -> Option<i32> {
        self.parent_panel
    }
    async fn parent_order(&self) -> Option<i32> {
        self.parent_order
    }
    async fn parent(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Panel>> {
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
    async fn children(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Panel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Panel::query()
            .filter(schema::panel::parent_panel.eq(self.id))
            .order(schema::panel::parent_order.asc())
            .load(&mut connection)
            .await?)
    }
    async fn all_children_recursive(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Panel>> {
        let mut connection = get_connection(ctx).await?;
        let raw_sql = r#"
        WITH RECURSIVE panel_tree AS (
            SELECT
                id, name, schacht_id, parent_panel, parent_order,
                1 as level
            FROM panel
            WHERE parent_panel = $1

            UNION ALL

            SELECT
                p.id, p.name, p.schacht_id, p.parent_panel, p.parent_order,
                pt.level + 1 as level
            FROM panel p
            INNER JOIN panel_tree pt ON p.parent_panel = pt.id
        )
        SELECT
            id, name, schacht_id, parent_panel, parent_order
        FROM panel_tree
        ORDER BY level, parent_order;
    "#;

        Ok(sql_query(raw_sql)
            .bind::<Integer, _>(self.id)
            .load::<Panel>(&mut connection)
            .await?)
    }
    async fn ports(
        &self,
        ctx: &Context<'_>,
        port_type: Option<PanelPortType>,
    ) -> async_graphql::Result<Vec<PanelPort>> {
        let mut connection = get_connection(ctx).await?;
        let filter = schema::panel_port::panel_id.eq(self.id);
        Ok(if let Some(pt) = port_type {
            PanelPort::query()
                .filter(filter.and(schema::panel_port::port_type.eq(pt)))
                .order_by(schema::panel_port::port_order.asc())
                .load(&mut connection)
                .await?
        } else {
            PanelPort::query()
                .filter(filter)
                .order_by(schema::panel_port::port_order.asc())
                .load(&mut connection)
                .await?
        })
    }
    async fn count_ports(
        &self,
        ctx: &Context<'_>,
        port_type: Option<PanelPortType>,
    ) -> async_graphql::Result<i64> {
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
}

#[Object]
impl PanelPort {
    async fn id(&self) -> i32 {
        self.id
    }
    async fn order_number(&self) -> i32 {
        self.port_order
    }
    async fn panel(&self, ctx: &Context<'_>) -> async_graphql::Result<Panel> {
        let mut connection = get_connection(ctx).await?;
        Ok(Panel::query()
            .filter(schema::panel::id.eq(self.panel_id))
            .first(&mut connection)
            .await?)
    }
    async fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    /*async fn connected_fibers(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<FiberPathSegment>> {
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
        info!("type: {:?}", self.port_type);
        self.port_type
    }
}

#[derive(Copy, Clone, PartialEq, Hash, Ord, PartialOrd, Eq, Debug)]
pub struct PortId {
    pub panel_id: i32,
    pub port_number: i32,
}
