use crate::{
    error::FrontendError,
    graphql::authenticated::netbox_sync::{
        AsymetricTargetConnectionEntry, AsymmetricDuplexError, BlindEndError,
        MissingNetboxReferenceError, SyncIssue, SyncNetbox,
    },
    util::get_credentials,
};
use patternfly_yew::prelude::{Button, ButtonVariant, Modal, ModalVariant};
use yew::{
    Callback, Component, Context, Html, Properties, html, html::IntoPropValue,
    platform::spawn_local,
};

pub struct NetboxSyncModal {
    syncing: bool,
    error: Option<FrontendError>,
    sync_issues: Box<[SyncIssue]>,
}
pub enum Msg {
    StartSync,
    Error(FrontendError),
    Synced(Vec<SyncIssue>),
}
#[derive(Properties, PartialEq)]
pub struct NetboxSyncProps {
    pub plan_id: i32,
    pub on_close: Callback<()>,
}

impl Component for NetboxSyncModal {
    type Message = Msg;
    type Properties = NetboxSyncProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            syncing: false,
            error: None,
            sync_issues: Box::new([]),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::StartSync => {
                self.syncing = true;
                let scope = ctx.link().clone();
                let plan_id = ctx.props().plan_id;
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(
                        SyncNetbox::sync_netbox(credentials.as_ref(), plan_id)
                            .await
                            .map_or_else(Msg::Error, Msg::Synced),
                    );
                });

                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                self.syncing = false;
                true
            }
            Msg::Synced(sync_issues) => {
                self.sync_issues = sync_issues.into_boxed_slice();
                self.error = None;
                self.syncing = false;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_cancel = {
            let callback = ctx.props().on_close.clone();
            Callback::from(move |_| callback.emit(()))
        };
        let syncing = self.syncing;
        let start_sync = {
            let scope = ctx.link().clone();
            Callback::from(move |_| scope.send_message(Msg::StartSync))
        };
        let error: Option<Html> = self.error.as_ref().map(|e| e.into_prop_value());
        let issues = self
            .sync_issues
            .iter()
            .map(|issue| match issue {
                SyncIssue::MissingNetboxReference(MissingNetboxReferenceError { port }) => {
                    let msg = format!("Fehlende Netbox refernz beim Port {}", port.port_label());
                    html!(msg)
                }
                SyncIssue::BlindEnd(BlindEndError { port }) => format!(
                    "Verbindung endet nicht auf einem Stecker {} ",
                    port.port_label()
                )
                .into_prop_value(),
                SyncIssue::AsymmetricDuplex(AsymmetricDuplexError {
                    start_netbox_port,
                    connections,
                }) => {
                    let endpoints = connections.iter().map(
                        |AsymetricTargetConnectionEntry {
                             target_netbox_port,
                             source_port,
                             target_port,
                         }| {
                            let msg = format!(
                                "{}: {} -> {}",
                                target_netbox_port.display_name(),
                                source_port.port_label(),
                                target_port.port_label()
                            );
                            html!(<dd>{msg}</dd>)
                        },
                    );

                    let msg = format!(
                        "Verschiedene Netbox-Gegenstellen zu {}",
                        start_netbox_port.display_name()
                    );
                    html! {
                        <>
                        <dt>{msg}</dt>
                        {for endpoints}
                        </>
                    }
                }
                SyncIssue::PortBlockedInNetbox(err) => {
                    let msg = format!(
                        "Port {} blockiert in Netbox: Netbox Port {}",
                        err.port.port_label(),
                        err.netbox_port.display_name()
                    );
                    html!(msg)
                }
                SyncIssue::NameCollision(err) => {
                    let msg = format!("Namenskollision bei Circuit: {}", err.circuit_name);
                    html!(msg)
                }
                SyncIssue::MissingNetboxMasterData(err) => {
                    let msg = format!("Fehlende Netbox Stammdaten für Typ: {}", err.entity_type);
                    html!(msg)
                }
                SyncIssue::RoutingLoop(err) => {
                    let msg = format!(
                        "Routing Loop festgestellt bei Port {}",
                        err.port.port_label()
                    );
                    html!(msg)
                }
                SyncIssue::InvalidTargetReference(err) => {
                    let msg = format!("Ungültiges Ziel bei Port {}", err.port.port_label());
                    html!(msg)
                }
                SyncIssue::Unknown => {
                    html!("Unbekannter Fehler")
                }
            })
            .map(|i: Html| html!(<p>{i}</p>));
        html! {
            <Modal
                title="Netbox Sync"
                variant={ModalVariant::Small}
                footer={html!{
                    <>
                        <Button label="Synchronisieren" variant={ButtonVariant::Danger} disabled={syncing} onclick={start_sync}/>
                        <Button label="Abbrechen" variant={ButtonVariant::Link} onclick={on_cancel}/>
                    </>
                }}>
                {error}
            {for issues}
                <p>{"Synchronisiere Netbox."}</p>
            </Modal>
        }
    }
}
