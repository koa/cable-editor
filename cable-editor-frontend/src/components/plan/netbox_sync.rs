use crate::{
    error::{FrontendError, messages},
    graphql::authenticated::netbox_sync::{AsymmetricDuplexError, SyncIssue, SyncNetbox},
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

    fn create(_ctx: &Context<Self>) -> Self {
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
                // The pairs of ports leading to the different counterparts
                SyncIssue::AsymmetricDuplex(AsymmetricDuplexError { connections, .. }) => html! {
                    <>
                        <dt>{messages::sync_issue(issue)}</dt>
                        {for connections.iter().flat_map(|conn| conn.pairs.iter().map(move |pair| {
                            let pair_msg = format!(
                                "{}: {} -> {}",
                                conn.target_netbox_port.display_name(),
                                pair.source_port.port_label(),
                                pair.target_port.port_label()
                            );
                            html!(<dd>{pair_msg}</dd>)
                        }))}
                    </>
                },
                issue => html!(messages::sync_issue(issue)),
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
