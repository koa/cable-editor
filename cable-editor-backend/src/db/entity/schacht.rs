use async_graphql::{Context, Object};
use diesel::{
    Associations, BoolExpressionMethods, ExpressionMethods, HasQuery, Identifiable, Insertable,
    OptionalExtension, QueryDsl,
};
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Object};

use crate::{
    db::{
        entity::{
            Duct, XmlDocument,
            cable::{Cable, CableEnd, PotentialPathSegment},
            panel::Panel,
        },
        schema,
    },
    graphql::{authenticated::get_connection, model},
};
use postgis_diesel::types::Point;

#[derive(Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schema::schacht)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Schacht {
    pub id: i32,
    pub name: Option<String>,
    pub typ: Option<i32>,
    pub geom: Option<Point>,
}

#[derive(HasQuery, Identifiable, Insertable, Associations, Debug, PartialEq)]
#[diesel(belongs_to(Schacht, foreign_key = id))]
#[diesel(table_name = schema::schacht_typ)]
pub struct SchachtTyp {
    pub id: i32,
    pub name: Option<String>,
    pub icon: XmlDocument,
}

#[Object]
impl Schacht {
    async fn name(&self) -> &str {
        self.name.as_deref().unwrap_or_default()
    }

    async fn id(&self) -> i32 {
        self.id
    }
    async fn typ(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<SchachtTyp>> {
        if let Some(typ) = self.typ {
            let mut connection = get_connection(ctx).await?;
            Ok(Some(
                SchachtTyp::query()
                    .filter(schema::schacht_typ::id.eq(typ))
                    .get_result(&mut connection)
                    .await?,
            ))
        } else {
            Ok(None)
        }
    }
    async fn position(&self) -> Option<model::Point> {
        self.geom.map(Point::into)
    }
    async fn connecting_duct(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<PotentialPathSegment>> {
        let mut connection = get_connection(ctx).await?;

        let ducts: Vec<Duct> = Duct::query()
            .filter(
                schema::trasse::schacht_a
                    .eq(self.id)
                    .or(schema::trasse::schacht_z.eq(self.id)),
            )
            .load(&mut connection)
            .await?;

        let mut results = Vec::new();
        for duct in ducts {
            let other_schacht_id = if duct.schacht_a == self.id {
                duct.schacht_z
            } else {
                duct.schacht_a
            };

            let other_schacht = Schacht::query()
                .filter(schema::schacht::id.eq(other_schacht_id))
                .get_result(&mut connection)
                .await?;
            results.push(PotentialPathSegment {
                duct,
                schacht: other_schacht,
            });
        }

        Ok(results)
    }
    async fn root_panels(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Panel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Panel::query()
            .filter(
                schema::panel::schacht_id
                    .eq(self.id)
                    .and(schema::panel::parent_panel.is_null()),
            )
            .order(schema::panel::parent_order.asc())
            .load(&mut connection)
            .await?)
    }
    async fn cable(
        &self,
        ctx: &Context<'_>,
        cable_id: i32,
    ) -> async_graphql::Result<Option<CableEnd>> {
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
    async fn cables(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<CableEnd>> {
        let mut connection = get_connection(ctx).await?;
        Ok(schema::kabel::table
            // 1. Die Relationen joinen (Kabel -> KabelTrasse -> Trasse)
            .inner_join(schema::kabel_trasse::table.inner_join(schema::trasse::table))
            // 2. Nur Trassen betrachten, die an unseren Ziel-Schacht grenzen
            .filter(
                schema::trasse::schacht_a
                    .eq(self.id)
                    .or(schema::trasse::schacht_z.eq(self.id)),
            )
            // 3. Nach den Kabel-Spalten gruppieren, um zählen zu können
            .group_by(schema::kabel::id)
            // 4. Die Magie: Nur Kabel behalten, die exakt 1 Berührungspunkt mit dem Schacht haben
            .having(diesel::dsl::count(schema::trasse::id).eq(1))
            // 5. Die Daten auslesen
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
    async fn list_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Schacht>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Schacht::query()
            .filter(schema::schacht::typ.eq(self.id))
            .load(&mut connection)
            .await?)
    }
}

pub async fn fetch_schacht(
    connection: &mut Object<AsyncPgConnection>,
    id: i32,
) -> async_graphql::Result<Schacht> {
    Ok(Schacht::query()
        .filter(schema::schacht::id.eq(id))
        .get_result(connection)
        .await?)
}
