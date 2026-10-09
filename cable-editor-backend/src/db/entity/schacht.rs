use crate::graphql::error::ApiResult;
use async_graphql::{Context, Object};
use cable_editor_common::{ObjectKind, UserError};
use diesel::{
    Associations, BoolExpressionMethods, ExpressionMethods, HasQuery, Identifiable, Insertable,
    OptionalExtension, QueryDsl,
};
use diesel_async::RunQueryDsl;

use crate::{
    db::{
        entity::{
            Duct, XmlDocument,
            cable::{Cable, CableEnd, PotentialPathSegment},
            eigentuemer::Eigentuemer,
            lkmap::{Genauigkeit, LkmapPunktObjektart},
            panel::Panel,
        },
        schema,
    },
    graphql::{
        authenticated::get_connection,
        loader::{
            EigentuemerId, SchachtId, SchachtLocation, SchachtRootPanels, SchachtTypCount,
            SchachtTypId, get_loader, load_one,
        },
        model::{GeoPoint, Lv95Point},
    },
};
use chrono::{DateTime, Utc};
use postgis_diesel::types::Point;

#[derive(Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schema::schacht)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Schacht {
    pub id: i32,
    pub name: Option<String>,
    pub typ: Option<i32>,
    pub geom: Option<Point>,
    pub eigentuemer_id: i32,
    pub lagebestimmung: Genauigkeit,
    pub geaendert_am: DateTime<Utc>,
}

#[derive(HasQuery, Identifiable, Insertable, Associations, Debug, Clone, PartialEq)]
#[diesel(belongs_to(Schacht, foreign_key = id))]
#[diesel(table_name = schema::schacht_typ)]
pub struct SchachtTyp {
    pub id: i32,
    pub name: Option<String>,
    pub icon: XmlDocument,
    pub lkmap_objektart: LkmapPunktObjektart,
    pub dimension1_mm: Option<i32>,
    pub dimension2_mm: Option<i32>,
}

#[Object]
impl Schacht {
    async fn name(&self) -> &str {
        self.name.as_deref().unwrap_or_default()
    }

    async fn id(&self) -> i32 {
        self.id
    }
    async fn typ(&self, ctx: &Context<'_>) -> ApiResult<Option<SchachtTyp>> {
        match self.typ {
            Some(typ) => Ok(Some(load_one(ctx, SchachtTypId(typ)).await?)),
            None => Ok(None),
        }
    }
    async fn owner(&self, ctx: &Context<'_>) -> ApiResult<Eigentuemer> {
        load_one(ctx, EigentuemerId(self.eigentuemer_id)).await
    }
    /// Accuracy of the position
    async fn lagebestimmung(&self) -> Genauigkeit {
        self.lagebestimmung
    }
    /// Last change of the Schacht or its type (`Letzte_Aenderung`)
    async fn changed_at(&self) -> DateTime<Utc> {
        self.geaendert_am
    }
    /// LV95, as stored (`location` is WGS84)
    async fn position(&self) -> Option<Lv95Point> {
        self.geom.map(Point::into)
    }
    /// Position in WGS84 for the map (`position` is LV95)
    async fn location(&self, ctx: &Context<'_>) -> ApiResult<Option<GeoPoint>> {
        get_loader(ctx)?.load_one(SchachtLocation(self.id)).await
    }
    async fn connecting_duct(&self, ctx: &Context<'_>) -> ApiResult<Box<[PotentialPathSegment]>> {
        let ducts: Vec<Duct> = {
            let mut connection = get_connection(ctx).await?;
            Duct::query()
                .filter(
                    schema::trasse::schacht_a
                        .eq(self.id)
                        .or(schema::trasse::schacht_z.eq(self.id)),
                )
                .load(&mut connection)
                .await?
        };
        let other_end = |duct: &Duct| {
            SchachtId(if duct.schacht_a == self.id {
                duct.schacht_z
            } else {
                duct.schacht_a
            })
        };
        let others = get_loader(ctx)?
            .load_many(ducts.iter().map(other_end))
            .await?;
        ducts
            .into_iter()
            .map(|duct| {
                let other = other_end(&duct);
                let schacht = others.get(&other).cloned().ok_or(UserError::NotFound {
                    kind: ObjectKind::Schacht,
                    id: other.0.into(),
                })?;
                Ok(PotentialPathSegment { duct, schacht })
            })
            .collect()
    }
    async fn root_panels(&self, ctx: &Context<'_>) -> ApiResult<Box<[Panel]>> {
        Ok(get_loader(ctx)?
            .load_one(SchachtRootPanels(self.id))
            .await?
            .unwrap_or_default())
    }
    async fn cable(&self, ctx: &Context<'_>, cable_id: i32) -> ApiResult<Option<CableEnd>> {
        let mut connection = get_connection(ctx).await?;
        Ok(schema::kabel::table
            .find(cable_id)
            .first::<Cable>(&mut connection)
            .await
            .optional()?
            .map(|cable| CableEnd {
                cable,
                schacht: self.clone(),
            }))
    }
    async fn cables(&self, ctx: &Context<'_>) -> ApiResult<Box<[CableEnd]>> {
        let mut connection = get_connection(ctx).await?;
        Ok(schema::kabel::table
            .inner_join(schema::kabel_ende::table)
            .filter(schema::kabel_ende::schacht.eq(self.id))
            .order_by(schema::kabel::id)
            .select(schema::kabel::all_columns)
            .load::<Cable>(&mut connection)
            .await
            .map(|cables| {
                cables
                    .into_iter()
                    .map(|cable| CableEnd {
                        cable,
                        schacht: self.clone(),
                    })
                    .collect()
            })?)
    }
}

#[Object]
impl SchachtTyp {
    async fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    async fn id(&self) -> i32 {
        self.id
    }
    async fn icon(&self) -> &str {
        self.icon.0.as_ref()
    }
    /// `Objektart` of its Schächte in the Leitungskataster
    async fn lkmap_objektart(&self) -> LkmapPunktObjektart {
        self.lkmap_objektart
    }
    /// Larger inner dimension in millimetres, optional
    async fn dimension1_mm(&self) -> Option<i32> {
        self.dimension1_mm
    }
    /// Smaller inner dimension in millimetres, optional
    async fn dimension2_mm(&self) -> Option<i32> {
        self.dimension2_mm
    }
    /// How many Schächte have this type
    async fn schacht_count(&self, ctx: &Context<'_>) -> ApiResult<i32> {
        Ok(get_loader(ctx)?
            .load_one(SchachtTypCount(self.id))
            .await?
            .unwrap_or_default())
    }
    async fn list_schacht(&self, ctx: &Context<'_>) -> ApiResult<Vec<Schacht>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Schacht::query()
            .filter(schema::schacht::typ.eq(self.id))
            .load(&mut connection)
            .await?)
    }
}
