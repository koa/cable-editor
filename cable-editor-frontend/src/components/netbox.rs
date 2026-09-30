//! The automatic sync to Netbox (docs/netbox-sync.md): the hint in the breadcrumb bar when
//! Netbox isn't in sync, and the issues a run found.

use crate::{
    components::menu::popup::{MenuGroup, MenuLinkItem, PopupMenu},
    error::{FrontendError, messages},
    graphql::authenticated::{
        current_user::Role,
        list_plans::BASELINE_PLAN_ID,
        netbox_sync::{AsymmetricDuplexError, NetboxStatus, NetboxSyncState, SyncIssue},
    },
    pages::router::{AppRoute, PlanView},
    util::{get_credentials, get_role},
};
use gloo_timers::callback::Interval;
use patternfly_yew::prelude::{Icon, MenuToggleVariant};
use yew::{Component, Context, Html, Properties, classes, html, platform::spawn_local};

/// How often the hint asks again, in milliseconds (besides on every navigation)
const REFRESH_MS: u32 = 60_000;

#[derive(Properties, PartialEq)]
pub struct NetboxHintProps {
    /// The page shown: the hint asks again when it changes
    pub route: AppRoute,
}

/// In the breadcrumb bar, only while Netbox isn't in sync (issues, a failed run) or its state
/// couldn't be loaded: a menu explaining it, for admins with the way to the page "Netbox".
/// A pending run shows nothing, that is the normal case after every change.
pub struct NetboxHint {
    status: Option<Result<NetboxStatus, FrontendError>>,
    _refresh: Interval,
}

pub enum Msg {
    Fetch,
    Loaded(Result<NetboxStatus, FrontendError>),
}

impl Component for NetboxHint {
    type Message = Msg;
    type Properties = NetboxHintProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Fetch);
        let link = ctx.link().clone();
        Self {
            status: None,
            _refresh: Interval::new(REFRESH_MS, move || link.send_message(Msg::Fetch)),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Fetch => {
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope
                        .send_message(Msg::Loaded(NetboxStatus::fetch(credentials.as_ref()).await));
                });
                false
            }
            Msg::Loaded(status) => {
                self.status = Some(status);
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().route != old_props.route {
            ctx.link().send_message(Msg::Fetch);
        }
        false
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        // danger: a failure, else a warning
        let (text, danger, message, details) = match &self.status {
            None => return Html::default(),
            Some(Ok(status)) => match status.state {
                NetboxSyncState::Synchron | NetboxSyncState::Ausstehend => return Html::default(),
                NetboxSyncState::NichtSynchron => (
                    "Netbox nicht synchron",
                    false,
                    "Der Sync hat Probleme gefunden und Netbox unverändert gelassen: die Circuits in Netbox entsprechen nicht der aktiven Planung.",
                    Box::default(),
                ),
                NetboxSyncState::Fehler => (
                    "Netbox-Sync fehlgeschlagen",
                    true,
                    "Der letzte Sync ist fehlgeschlagen und wird wiederholt; Netbox ist womöglich nur teilweise synchronisiert.",
                    Box::default(),
                ),
            },
            Some(Err(error)) => (
                "Netbox-Status unbekannt",
                true,
                "Der Stand des Netbox-Syncs konnte nicht geladen werden:",
                [error.title()]
                    .into_iter()
                    .chain(error.details())
                    .collect::<Box<[_]>>(),
            ),
        };
        let admin = get_role(ctx.link()) >= Role::Admin;
        let plan_id = match ctx.props().route {
            AppRoute::Plan { plan_id, .. } => plan_id,
            AppRoute::ListOfPlans => BASELINE_PLAN_ID,
        };
        let (icon, class) = if danger {
            (Icon::ExclamationCircle, "netbox-hint__icon--danger")
        } else {
            (Icon::ExclamationTriangle, "netbox-hint__icon--warning")
        };
        html! {
            <PopupMenu
                variant={MenuToggleVariant::Plain}
                icon={html!(<span class={classes!("netbox-hint__icon", class)}>{icon}</span>)}
                text={html!(<span class="netbox-hint__text">{text}</span>)}
                aria_label={text}
                align_end=true
            >
                <div class="netbox-hint__message">
                    <p>{message}</p>
                    if !details.is_empty() {
                        <ul>{for details.iter().map(|detail| html!(<li>{detail}</li>))}</ul>
                    }
                    if !admin {
                        <p>{"Die Gründe sehen Admins auf der Seite „Netbox“."}</p>
                    }
                </div>
                if admin {
                    <MenuGroup divider=true>
                        <MenuLinkItem to={AppRoute::Plan { plan_id, view: PlanView::Netbox }}>
                            {"Netbox-Sync anzeigen"}
                        </MenuLinkItem>
                    </MenuGroup>
                }
            </PopupMenu>
        }
    }
}

/// The issues of a run, each worded by `messages::sync_issue`; an asymmetric duplex with the
/// pairs of ports leading to its different counterparts.
pub fn view_issues(issues: &[SyncIssue]) -> Html {
    html! {
        <ul class="netbox-issues">
            {for issues.iter().map(|issue| html! {
                <li>
                    {messages::sync_issue(issue)}
                    if let SyncIssue::AsymmetricDuplex(AsymmetricDuplexError { connections, .. }) = issue {
                        <ul>
                            {for connections.iter().flat_map(|connection| connection.pairs.iter().map(move |pair| html! {
                                <li>
                                    {format!(
                                        "{}: {} → {}",
                                        connection.target_netbox_port.display_name(),
                                        pair.source_port.port_label(),
                                        pair.target_port.port_label()
                                    )}
                                </li>
                            }))}
                        </ul>
                    }
                </li>
            })}
        </ul>
    }
}
