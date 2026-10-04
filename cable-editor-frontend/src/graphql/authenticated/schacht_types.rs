//! Types of Schächte (see docs/stammdaten.md).

use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, mutate, query},
};
use yew_oauth2::context::OAuth2Context;

/// `Objektart` of a type's Schächte in the Leitungskataster (SIA405 `LKPunkt`); `Display` and
/// `FromStr` are the value of its `Select` option.
#[derive(
    cynic::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString,
)]
pub enum LkmapPunktObjektart {
    SchachtRund,
    SchachtRechteckig,
    Bauwerk,
    Tragwerk,
    Unbekannt,
}

impl LkmapPunktObjektart {
    pub const ALL: [LkmapPunktObjektart; 5] = [
        LkmapPunktObjektart::SchachtRund,
        LkmapPunktObjektart::SchachtRechteckig,
        LkmapPunktObjektart::Bauwerk,
        LkmapPunktObjektart::Tragwerk,
        LkmapPunktObjektart::Unbekannt,
    ];

    pub fn title(self) -> &'static str {
        match self {
            LkmapPunktObjektart::SchachtRund => "Schacht rund",
            LkmapPunktObjektart::SchachtRechteckig => "Schacht rechteckig",
            LkmapPunktObjektart::Bauwerk => "Bauwerk",
            LkmapPunktObjektart::Tragwerk => "Tragwerk",
            LkmapPunktObjektart::Unbekannt => "unbekannt",
        }
    }
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "SchachtTyp")]
pub struct SchachtTypEntry {
    pub id: i32,
    pub name: Option<String>,
    /// SVG
    pub icon: String,
    pub lkmap_objektart: LkmapPunktObjektart,
    /// Larger inner dimension in millimetres
    pub dimension1_mm: Option<i32>,
    /// Smaller inner dimension in millimetres
    pub dimension2_mm: Option<i32>,
    pub schacht_count: i32,
}

impl SchachtTypEntry {
    pub fn title(&self) -> &str {
        self.name.as_deref().unwrap_or("(ohne Namen)")
    }
    /// "1200 × 800 mm", "1000 mm", missing without dimensions
    pub fn dimensions(&self) -> Option<String> {
        match (self.dimension1_mm, self.dimension2_mm) {
            (Some(larger), Some(smaller)) => Some(format!("{larger} × {smaller} mm")),
            (Some(larger), None) => Some(format!("{larger} mm")),
            _ => None,
        }
    }
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct ListSchachtTypQuery {
    list_schacht_typ: Vec<SchachtTypEntry>,
}

/// By name.
pub async fn fetch_schacht_typ_list(
    credentials: Option<&OAuth2Context>,
) -> Result<Vec<SchachtTypEntry>, FrontendError> {
    let mut types = query::<ListSchachtTypQuery, _>((), credentials)
        .await?
        .list_schacht_typ;
    types.sort_by(|a, b| a.title().cmp(b.title()));
    Ok(types)
}

#[derive(cynic::QueryVariables)]
struct SchachtTypVariables {
    typ_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "SchachtTypVariables")]
struct SchachtTypQuery {
    #[arguments(typId: $typ_id)]
    schacht_typ: Option<SchachtTypEntry>,
}

/// Missing if it doesn't exist.
pub async fn fetch_schacht_typ(
    credentials: Option<&OAuth2Context>,
    typ_id: i32,
) -> Result<Option<SchachtTypEntry>, FrontendError> {
    Ok(
        query::<SchachtTypQuery, _>(SchachtTypVariables { typ_id }, credentials)
            .await?
            .schacht_typ,
    )
}

#[derive(cynic::InputObject, Debug, Clone, PartialEq)]
pub struct SchachtTypInput {
    pub name: String,
    /// Missing: keep the stored one (a new type gets a plain circle)
    pub icon: Option<String>,
    pub lkmap_objektart: LkmapPunktObjektart,
    pub dimension1_mm: Option<i32>,
    pub dimension2_mm: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug, Clone, Copy)]
#[cynic(graphql_type = "SchachtTyp")]
struct SchachtTypId {
    id: i32,
}

#[derive(cynic::QueryVariables)]
struct CreateSchachtTypVariables {
    typ: SchachtTypInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CreateSchachtTypVariables")]
struct CreateSchachtTypMutation {
    #[arguments(typ: $typ)]
    create_schacht_typ: SchachtTypId,
}

/// The id of the new type.
pub async fn create_schacht_typ(
    credentials: Option<&OAuth2Context>,
    typ: SchachtTypInput,
) -> Result<i32, FrontendError> {
    Ok(
        mutate::<CreateSchachtTypMutation, _>(CreateSchachtTypVariables { typ }, credentials)
            .await?
            .create_schacht_typ
            .id,
    )
}

#[derive(cynic::QueryVariables)]
struct UpdateSchachtTypVariables {
    typ_id: i32,
    typ: SchachtTypInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "UpdateSchachtTypVariables")]
struct UpdateSchachtTypMutation {
    #[arguments(typId: $typ_id, typ: $typ)]
    #[allow(unused)]
    update_schacht_typ: SchachtTypId,
}

pub async fn update_schacht_typ(
    credentials: Option<&OAuth2Context>,
    typ_id: i32,
    typ: SchachtTypInput,
) -> Result<(), FrontendError> {
    mutate::<UpdateSchachtTypMutation, _>(UpdateSchachtTypVariables { typ_id, typ }, credentials)
        .await?;
    Ok(())
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SchachtTypVariables")]
struct DeleteSchachtTypMutation {
    #[arguments(typId: $typ_id)]
    #[allow(unused)]
    delete_schacht_typ: bool,
}

pub async fn delete_schacht_typ(
    credentials: Option<&OAuth2Context>,
    typ_id: i32,
) -> Result<(), FrontendError> {
    mutate::<DeleteSchachtTypMutation, _>(SchachtTypVariables { typ_id }, credentials).await?;
    Ok(())
}

/// A Schacht's type, for its icon on the map (`geo::map::schacht_marker`).
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "SchachtTyp")]
pub struct SchachtTypIcon {
    pub icon: String,
}

/// The icon of a Schacht's type, if it has one.
pub fn type_icon(typ: &Option<SchachtTypIcon>) -> Option<&str> {
    typ.as_ref().map(|typ| typ.icon.as_str())
}

/// The icon as `src` of an `<img>`: a script in the SVG doesn't run there, unlike inline.
pub fn icon_src(svg: &str) -> String {
    format!(
        "data:image/svg+xml;charset=utf-8,{}",
        js_sys::encode_uri_component(svg)
    )
}
