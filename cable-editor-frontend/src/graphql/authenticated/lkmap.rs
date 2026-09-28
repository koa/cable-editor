//! The delivery to the Leitungskataster (see docs/leitungskataster.md): per owner the files,
//! what goes into them, the deliveries so far and when the next one is due.

use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{DateTime, GeoPoint, schema},
        mutate, query,
    },
};
use js_sys::Date;
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct LkmapExportsQuery {
    lkmap_exports: Vec<LkmapExport>,
}

/// An owner's delivery.
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
pub struct LkmapExport {
    pub owner: LkmapOwner,
    /// Missing without UID and when nothing has a position
    pub checksum: Option<String>,
    pub perimeter_area: Option<Vec<GeoPoint>>,
    pub schaechte: Vec<LkmapSchacht>,
    pub ducts: Vec<LkmapDuct>,
    pub schaechte_without_position: Vec<LkmapSchacht>,
    pub ducts_without_line: Vec<LkmapDuct>,
    /// The latest first
    pub deliveries: Vec<LkmapDelivery>,
    pub first_change_since_delivery: Option<DateTime>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Owner")]
pub struct LkmapOwner {
    pub id: i32,
    pub name: String,
    pub uid: Option<String>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct LkmapSchacht {
    pub id: i32,
    pub name: String,
    pub location: Option<GeoPoint>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Duct")]
pub struct LkmapDuct {
    pub id: i32,
    pub description: Option<String>,
    pub schacht_a: LkmapDuctEnd,
    pub schacht_z: LkmapDuctEnd,
    pub line: Option<Vec<GeoPoint>>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct LkmapDuctEnd {
    pub name: String,
}

impl LkmapDuct {
    pub fn title(&self) -> String {
        super::list_ducts::duct_title(
            self.description.as_deref(),
            &self.schacht_a.name,
            &self.schacht_z.name,
        )
    }
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
pub struct LkmapDelivery {
    pub id: i32,
    /// When the files were downloaded
    pub created_at: DateTime,
    pub created_by: String,
    pub schacht_count: i32,
    pub duct_count: i32,
    pub checksum: String,
    /// When they reached the Checkservice, missing while not confirmed
    pub delivered_at: Option<DateTime>,
}

/// Where an owner's delivery stands, the most urgent first.
#[derive(Debug, Clone, PartialEq)]
pub enum DeliveryState {
    /// Something to deliver, but no UID (Datenherr)
    WithoutUid,
    /// Nothing with a position to deliver
    NothingToDeliver,
    /// Never delivered: the Zuständigkeitsperimeter is due before anything else
    NeverDelivered,
    /// Changed since the last delivery, due a week after the first change (unknown when only
    /// something was deleted)
    Changed {
        due: Option<Date>,
    },
    /// Unchanged, but not delivered in this quarter yet
    QuarterDue {
        due: Date,
    },
    UpToDate,
}

/// The week after a change to deliver it (§4 lit. a LKV).
const DAYS_TO_DELIVER: f64 = 7.0;
const DAY_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;

impl LkmapExport {
    /// The deliveries whose files reached the Checkservice, the latest first.
    fn delivered(&self) -> impl Iterator<Item = &LkmapDelivery> {
        self.deliveries
            .iter()
            .filter(|delivery| delivery.delivered_at.is_some())
    }

    pub fn state(&self, now: &Date) -> DeliveryState {
        if self.owner.uid.is_none() {
            return DeliveryState::WithoutUid;
        }
        let Some(checksum) = &self.checksum else {
            return DeliveryState::NothingToDeliver;
        };
        let Some(last) = self.delivered().next() else {
            return DeliveryState::NeverDelivered;
        };
        if &last.checksum != checksum {
            let due = self
                .first_change_since_delivery
                .as_ref()
                .and_then(DateTime::date)
                .map(|changed| Date::new(&(changed.get_time() + DAYS_TO_DELIVER * DAY_MS).into()));
            return DeliveryState::Changed { due };
        }
        let quarter_start = quarter_start(now).get_time();
        let this_quarter = self.delivered().any(|delivery| {
            delivery
                .delivered_at
                .as_ref()
                .and_then(DateTime::date)
                .is_some_and(|at| at.get_time() >= quarter_start)
        });
        if this_quarter {
            DeliveryState::UpToDate
        } else {
            DeliveryState::QuarterDue {
                due: quarter_end(now),
            }
        }
    }

    /// The download of the current files, if it isn't confirmed yet.
    pub fn pending(&self) -> Option<&LkmapDelivery> {
        let checksum = self.checksum.as_ref()?;
        self.deliveries
            .iter()
            .find(|delivery| delivery.delivered_at.is_none() && &delivery.checksum == checksum)
    }
}

/// The first day of the date's quarter, local time.
fn quarter_start(date: &Date) -> Date {
    let month = date.get_month() / 3 * 3;
    Date::new_with_year_month_day(date.get_full_year(), month as i32, 1)
}

/// The last day of the date's quarter, local time.
pub fn quarter_end(date: &Date) -> Date {
    let month = date.get_month() / 3 * 3 + 3;
    // Day 0 of the next quarter's first month
    Date::new_with_year_month_day(date.get_full_year(), month as i32, 0)
}

/// Whole days from `now` until the end of `day` (negative: past).
pub fn days_until(now: &Date, day: &Date) -> i32 {
    let end = Date::new_with_year_month_day(
        day.get_full_year(),
        day.get_month() as i32,
        day.get_date() as i32 + 1,
    );
    ((end.get_time() - now.get_time()) / DAY_MS).floor() as i32
}

/// The owners with something to deliver or delivered before, by name.
pub async fn fetch_lkmap_exports(
    credentials: Option<&OAuth2Context>,
) -> Result<Vec<LkmapExport>, FrontendError> {
    Ok(query::<LkmapExportsQuery, _>((), credentials)
        .await?
        .lkmap_exports)
}

#[derive(cynic::QueryVariables)]
struct DownloadVariables {
    owner_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "DownloadVariables")]
struct DownloadMutation {
    #[arguments(ownerId: $owner_id)]
    download_lkmap: LkmapDownload,
}

#[derive(cynic::QueryFragment, Debug)]
struct LkmapDownload {
    files: Vec<DownloadFile>,
}

/// A file to save, its content in base64.
#[derive(cynic::QueryFragment, Debug)]
pub struct DownloadFile {
    pub file_name: String,
    pub content: String,
}

/// The owner's ZIPs (LKMap, Zuständigkeitsperimeter); logs a delivery.
pub async fn download_lkmap(
    credentials: Option<&OAuth2Context>,
    owner_id: i32,
) -> Result<Vec<DownloadFile>, FrontendError> {
    Ok(
        mutate::<DownloadMutation, _>(DownloadVariables { owner_id }, credentials)
            .await?
            .download_lkmap
            .files,
    )
}

#[derive(cynic::QueryVariables)]
struct SetDeliveredVariables {
    delivery_id: i32,
    delivered: bool,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SetDeliveredVariables")]
struct SetDeliveredMutation {
    #[arguments(deliveryId: $delivery_id, delivered: $delivered)]
    #[allow(unused)]
    set_lkmap_delivered: DeliveryId,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "LkmapDelivery")]
struct DeliveryId {
    #[allow(unused)]
    id: i32,
}

/// Confirms that the delivery reached the Checkservice, or takes that back.
pub async fn set_lkmap_delivered(
    credentials: Option<&OAuth2Context>,
    delivery_id: i32,
    delivered: bool,
) -> Result<(), FrontendError> {
    mutate::<SetDeliveredMutation, _>(
        SetDeliveredVariables {
            delivery_id,
            delivered,
        },
        credentials,
    )
    .await?;
    Ok(())
}
