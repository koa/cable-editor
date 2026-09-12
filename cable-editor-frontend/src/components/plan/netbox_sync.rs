use crate::error::FrontendError;
use crate::graphql::authenticated::netbox_sync::{PlanDummy, SyncNetbox};
use crate::graphql::authenticated::plan_details::PlanDetails;
use crate::util::get_credentials;
use patternfly_yew::prelude::{Button, ButtonVariant, Modal, ModalVariant};
use yew::html::IntoPropValue;
use yew::platform::spawn_local;
use yew::{Callback, Component, Context, Html, Properties, html};
use yew_oauth2::agent::OpenIdClient;

pub struct NetboxSyncModal {
    syncing: bool,
    error: Option<FrontendError>,
}
pub enum Msg {
    StartSync,
    Error(FrontendError),
    Synced(PlanDummy),
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
            Msg::Synced(_) => {
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
                <p>{"Synchronisiere Netbox."}</p>
            </Modal>
        }
    }
}
