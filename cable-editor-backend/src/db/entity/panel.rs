use crate::db::connect;
use crate::db::entity::plan::Plan;
use crate::{
    db::{
        entity::{cable::Fiber, schacht::Schacht},
        schema,
    },
    graphql::authenticated::get_connection,
};
use async_graphql::{Context, Enum, Object};
use diesel::{
    Associations, BoolExpressionMethods, ExpressionMethods, HasQuery, Identifiable, Insertable,
    OptionalExtension, QueryDsl, QueryableByName, sql_query, sql_types::Integer,
};
use diesel_async::RunQueryDsl;
use diesel_derive_enum::DbEnum;
use log::info;

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
    Identifiable, Insertable, HasQuery, Associations, Debug, Clone, PartialEq, QueryableByName,
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
    async fn ports(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PanelPort>> {
        let mut connection = get_connection(ctx).await?;
        Ok(PanelPort::query()
            .filter(schema::panel_port::panel_id.eq(self.id))
            .order_by(schema::panel_port::port_order.asc())
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
