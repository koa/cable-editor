pub mod lkmap;
pub mod mutation;
pub mod netbox_sync;
pub mod planned;

use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::{
            Duct,
            cable::Cable,
            eigentuemer::Eigentuemer,
            panel::{Panel, PanelPort},
            plan::Plan,
            schacht::{Schacht, SchachtTyp},
        },
        schema::{eigentuemer, kabel, panel, panel_port, plan, schacht, schacht_typ, trasse},
    },
    graphql::{
        authorization::{Role, RoleGuard},
        context::UserInfo,
        duct_line::{self, DuctLineCheck, LineInput},
        geo::{self, ConvertedPoint, PositionInput},
        loader::SharedConnection,
    },
    netbox::{fetch::DeviceWithRearPorts, fetch_device_with_ports, fetch_devices_and_ports},
};
use async_graphql::{Context, EmptySubscription, Object, Schema};
use diesel::{ExpressionMethods, HasQuery, OptionalExtension, QueryDsl};
use diesel_async::{
    AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Object as DpObject,
};
use mutation::Mutation;
use tokio::sync::MutexGuard;

pub type AuthenticatedGraphqlSchema = Schema<Query, Mutation, EmptySubscription>;

pub struct Query;

#[Object]
impl Query {
    async fn current_user<'a>(&self, ctx: &Context<'a>) -> ApiResult<&'a UserInfo> {
        Ok(ctx.data::<UserInfo>()?)
    }
    async fn list_schacht(&self, ctx: &Context<'_>) -> ApiResult<Vec<Schacht>> {
        //let mut connection = pool.get().await?;
        let mut connection = get_connection(ctx).await?;
        let query = Schacht::query();
        let list = query.load(&mut connection).await?;
        Ok(list)
    }
    async fn schacht(&self, ctx: &Context<'_>, schacht_id: i32) -> ApiResult<Option<Schacht>> {
        let mut connection = get_connection(ctx).await?;

        let schacht = Schacht::query()
            .filter(schacht::id.eq(schacht_id))
            .first(&mut connection)
            .await
            .optional()?;
        Ok(schacht)
    }
    async fn list_schacht_typ(&self, ctx: &Context<'_>) -> ApiResult<Vec<SchachtTyp>> {
        let mut connection = get_connection(ctx).await?;
        let query = SchachtTyp::query();
        let list = query.load(&mut connection).await?;
        Ok(list)
    }
    /// Owners of Schächte and ducts, by name
    async fn list_owner(&self, ctx: &Context<'_>) -> ApiResult<Vec<Eigentuemer>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Eigentuemer::query()
            .order_by(eigentuemer::name)
            .load(&mut connection)
            .await?)
    }
    async fn schacht_typ(&self, ctx: &Context<'_>, typ_id: i32) -> ApiResult<Option<SchachtTyp>> {
        let mut connection = get_connection(ctx).await?;
        Ok(SchachtTyp::query()
            .filter(schacht_typ::id.eq(typ_id))
            .first(&mut connection)
            .await
            .optional()?)
    }
    async fn list_cable(&self, ctx: &Context<'_>) -> ApiResult<Vec<Cable>> {
        let mut connection = get_connection(ctx).await?;
        let query = Cable::query();
        Ok(query.load(&mut connection).await?)
    }
    async fn cable(&self, ctx: &Context<'_>, cable_id: u32) -> ApiResult<Option<Cable>> {
        let mut connection = get_connection(ctx).await?;
        Ok(kabel::table
            .find(cable_id as i32)
            .first::<Cable>(&mut connection)
            .await
            .optional()?)
    }
    async fn list_duct(&self, ctx: &Context<'_>) -> ApiResult<Vec<Duct>> {
        let mut connection = get_connection(ctx).await?;
        let query = Duct::query();
        Ok(query.load(&mut connection).await?)
    }
    async fn duct(&self, ctx: &Context<'_>, duct_id: i32) -> ApiResult<Option<Duct>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Duct::query()
            .filter(trasse::id.eq(duct_id))
            .first(&mut connection)
            .await
            .optional()?)
    }
    /// Where the automatic sync of the plan active in Netbox stands
    async fn netbox_sync(&self, ctx: &Context<'_>) -> ApiResult<netbox_sync::NetboxSync> {
        netbox_sync::NetboxSync::load(ctx).await
    }
    async fn list_plan(&self, ctx: &Context<'_>) -> ApiResult<Vec<Plan>> {
        let mut connection = get_connection(ctx).await?;
        let query = Plan::query();
        Ok(query.load(&mut connection).await?)
    }
    async fn plan(&self, ctx: &Context<'_>, plan_id: i32) -> ApiResult<Option<Plan>> {
        let mut connection = get_connection(ctx).await?;
        Ok(plan::table
            .find(plan_id)
            .first::<Plan>(&mut connection)
            .await
            .optional()?)
    }
    async fn panel(&self, ctx: &Context<'_>, panel_id: i32) -> ApiResult<Option<Panel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(panel::table
            .find(panel_id)
            .first(&mut connection)
            .await
            .optional()?)
    }
    /// Ports by id, e.g. to name the ones an error refers to; ids not found are left out.
    async fn ports(&self, ctx: &Context<'_>, port_ids: Vec<i32>) -> ApiResult<Vec<PanelPort>> {
        let mut connection = get_connection(ctx).await?;
        Ok(PanelPort::query()
            .filter(panel_port::id.eq_any(port_ids))
            .load(&mut connection)
            .await?)
    }
    /// A position in LV95 and WGS84, e.g. to preview a typed position on the map.
    async fn convert_point(
        &self,
        ctx: &Context<'_>,
        position: PositionInput,
    ) -> ApiResult<ConvertedPoint> {
        let mut connection = get_connection(ctx).await?;
        geo::convert(&mut connection, position).await
    }
    /// A duct's course from a file, as it would be stored: turned to run from Schacht A to Z,
    /// its ends repeating the Schächte left out.
    async fn check_duct_line(
        &self,
        ctx: &Context<'_>,
        schacht_a: i32,
        schacht_z: i32,
        line: LineInput,
    ) -> ApiResult<DuctLineCheck> {
        let mut connection = get_connection(ctx).await?;
        duct_line::check(&mut connection, schacht_a, schacht_z, &line).await
    }
    /// The delivery to the Leitungskataster (SIA405 LKMap, Zuständigkeitsperimeter)
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn lkmap_export(&self, ctx: &Context<'_>) -> ApiResult<lkmap::LkmapExport> {
        lkmap::export(ctx).await
    }
    async fn netbox_devices(&self) -> ApiResult<Box<[DeviceWithRearPorts]>> {
        Ok(fetch_devices_and_ports().await?)
    }
    async fn netbox_device(&self, netbox_device_id: u32) -> ApiResult<Option<DeviceWithRearPorts>> {
        Ok(fetch_device_with_ports(netbox_device_id.into()).await?)
    }
}

pub fn create_authenticated_schema() -> AuthenticatedGraphqlSchema {
    Schema::build(Query, Mutation::default(), EmptySubscription).finish()
}

pub async fn get_connection<'a>(
    ctx: &'a Context<'_>,
) -> ApiResult<MutexGuard<'a, DpObject<AsyncPgConnection>>> {
    let shared_conn = ctx.data::<SharedConnection>()?;
    Ok(shared_conn.lock().await)
}
