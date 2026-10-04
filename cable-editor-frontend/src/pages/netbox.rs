//! The automatic sync to Netbox (admins, docs/netbox-sync.md): the plan Netbox shows, the last
//! run with its issues or error, and a sync right away.

use crate::components::load::Load;
use crate::{
    components::{netbox::view_issues, page_layout::PageLayout, plan_link::PlanNameLink},
    error::{FrontendError, ServerError},
    graphql::authenticated::netbox_sync::{NetboxSync, NetboxSyncError, NetboxSyncState},
    util::{get_credentials, toast_error, toast_success},
};
use cable_editor_common::{ErrorOrigin, UserError};
use gloo_timers::callback::Interval;
use patternfly_yew::prelude::{
    Alert, AlertType, Button, ButtonVariant, Color, DescriptionGroup, DescriptionList, Label,
    Level, Title,
};
use serde::Deserialize;
use yew::{Component, Context, Html, Properties, html, platform::spawn_local};

/// How often the page asks again while a run is due, in milliseconds
const REFRESH_MS: u32 = 5_000;

#[derive(Properties, PartialEq)]
pub struct NetboxPageProps {
    pub plan_id: i32,
}

pub struct NetboxPage {
    sync: Load<NetboxSync>,
    starting: bool,
    _refresh: Interval,
}

pub enum Msg {
    Fetch,
    Loaded(Result<NetboxSync, FrontendError>),
    /// Asks again, but only while the state may change by itself
    Refresh,
    Start,
    Started(Result<(), FrontendError>),
}

impl Component for NetboxPage {
    type Message = Msg;
    type Properties = NetboxPageProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Fetch);
        let link = ctx.link().clone();
        Self {
            sync: Load::Pending,
            starting: false,
            _refresh: Interval::new(REFRESH_MS, move || link.send_message(Msg::Refresh)),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Fetch => {
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(Msg::Loaded(NetboxSync::fetch(credentials.as_ref()).await));
                });
                false
            }
            Msg::Loaded(sync) => {
                self.sync = Load::from(sync);
                true
            }
            Msg::Refresh => {
                if let Load::Loaded(sync) = &self.sync
                    && (sync.pending || sync.state == NetboxSyncState::Ausstehend)
                {
                    ctx.link().send_message(Msg::Fetch);
                }
                false
            }
            Msg::Start => {
                self.starting = true;
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(Msg::Started(NetboxSync::start(credentials.as_ref()).await));
                });
                true
            }
            Msg::Started(result) => {
                self.starting = false;
                match result {
                    Ok(()) => {
                        toast_success(ctx.link(), "Sync angestossen");
                        ctx.link().send_message(Msg::Fetch);
                    }
                    Err(error) => toast_error(
                        ctx.link(),
                        "Synchronisation konnte nicht angestossen werden",
                        error,
                    ),
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let content = self.sync.view(|sync| self.view_sync(ctx, sync));
        html!(<PageLayout title="Netbox">{content}</PageLayout>)
    }
}

impl NetboxPage {
    fn view_sync(&self, ctx: &Context<Self>, sync: &NetboxSync) -> Html {
        let (state, color) = match sync.state {
            NetboxSyncState::Synchron => ("synchron", Color::Green),
            NetboxSyncState::Ausstehend => ("ausstehend", Color::Blue),
            NetboxSyncState::NichtSynchron => ("nicht synchron (Probleme)", Color::Orange),
            NetboxSyncState::Fehler => ("fehlgeschlagen", Color::Red),
        };
        let last_run = match &sync.last_run {
            Some(last_run) => last_run.local(),
            None => "noch keiner".to_string(),
        };
        let active_plan = match &sync.active_plan {
            Some(plan) => html! {
                <PlanNameLink id={plan.id} text={plan.name.clone()}/>
            },
            None => html!("keiner"),
        };
        html! {
            <>
                <DescriptionList>
                    <DescriptionGroup term="Stand">
                        <Label label={state} compact=true {color}/>
                        if sync.pending && sync.state != NetboxSyncState::Ausstehend {
                            {" Änderungen warten auf den nächsten Sync."}
                        }
                    </DescriptionGroup>
                    <DescriptionGroup term="Netbox zeigt die Planung">{active_plan}</DescriptionGroup>
                    <DescriptionGroup term="Letzter Sync">{last_run}</DescriptionGroup>
                    if let Some(retry_at) = &sync.retry_at {
                        <DescriptionGroup term="Nächster Versuch">
                            {format!("frühestens {}", retry_at.local())}
                        </DescriptionGroup>
                    }
                </DescriptionList>
                <p class="pf-v6-u-mt-md pf-v6-u-color-200">
                    {"Die Circuits werden nach jeder Änderung der aktiven Planung synchronisiert und zusätzlich alle paar Stunden, was auch Änderungen von Hand in Netbox ausgleicht. Welche Planung Netbox zeigt, bestimmt die Seite „Planung bearbeiten“ einer Planung."}
                </p>
                <div class="pf-v6-u-mt-md pf-v6-u-mb-xl">
                    <Button
                        variant={ButtonVariant::Secondary}
                        label="Jetzt synchronisieren"
                        disabled={self.starting}
                        onclick={ctx.link().callback(|_| Msg::Start)}
                    />
                </div>
                if let Some(error) = &sync.error {
                    {view_error(error)}
                }
                if !sync.issues.is_empty() {
                    <Title level={Level::H2}>{"Probleme"}</Title>
                    <p class="pf-v6-u-mb-sm">
                        {"Solange sie bestehen, lässt der Sync Netbox unverändert."}
                    </p>
                    {view_issues(&sync.issues)}
                }
            </>
        }
    }
}

/// What the backend adds to the error of a failed run (like a GraphQL error's extensions)
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ErrorExtensions {
    user_error: Option<UserError>,
    origin: Option<ErrorOrigin>,
}

/// The error of the last run: a refusal (e.g. Netbox refused a step) worded by `messages`, else
/// the message with its origin.
fn view_error(error: &NetboxSyncError) -> Html {
    let extensions: ErrorExtensions = error
        .extensions
        .as_deref()
        .and_then(|extensions| serde_json::from_str(extensions).ok())
        .unwrap_or_default();
    let detail = match extensions.user_error {
        Some(user_error) => FrontendError::User(user_error).title(),
        None => ServerError {
            message: error.message.clone().into(),
            origin: extensions.origin,
        }
        .detail(),
    };
    html! {
        <Alert inline=true title="Letzter Sync fehlgeschlagen, er wird wiederholt" r#type={AlertType::Danger}>
            <p>{detail}</p>
        </Alert>
    }
}
