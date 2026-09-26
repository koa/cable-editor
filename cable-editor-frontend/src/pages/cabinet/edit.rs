use crate::{
    components::{
        cabinet::edit::EditCabinet,
        page_layout::{PageLayout, object_title},
    },
    error::FrontendError,
    graphql::authenticated::cabinet_details::fetch_schacht_name,
    util::{get_credentials, toast_error},
};
use yew::{Component, Context, Html, Properties, html, platform::spawn_local};

#[derive(Properties, PartialEq)]
pub struct EditCabinetPanelsProps {
    pub plan_id: i32,
    pub cabinet_id: i32,
}

/// Editing the panels of a Schacht, separate from its overview (`overview.rs`).
pub struct EditCabinetPanels {
    /// Name of the Schacht for the title, `None` while loading or if it failed (the editor below
    /// reports errors itself)
    name: Option<String>,
}

pub enum Msg {
    FetchName,
    Name(Result<String, FrontendError>),
}

impl Component for EditCabinetPanels {
    type Message = Msg;
    type Properties = EditCabinetPanelsProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::FetchName);
        Self { name: None }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchName => {
                let scope = ctx.link().clone();
                let credentials = get_credentials(&scope);
                let cabinet_id = ctx.props().cabinet_id;
                spawn_local(async move {
                    let name = fetch_schacht_name(credentials.as_ref(), cabinet_id).await;
                    scope.send_message(Msg::Name(name));
                });
                false
            }
            Msg::Name(Ok(name)) => {
                self.name = Some(name);
                true
            }
            Msg::Name(Err(error)) => {
                // Only the title lacks the name, the editor below shows its own errors
                toast_error(ctx.link(), "Schachtname konnte nicht geladen werden", error);
                false
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().cabinet_id != old_props.cabinet_id {
            self.name = None;
            ctx.link().send_message(Msg::FetchName);
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let EditCabinetPanelsProps {
            plan_id,
            cabinet_id,
        } = *ctx.props();
        html! {
            <PageLayout title={object_title("Panels bearbeiten", self.name.as_ref())}>
                <EditCabinet {plan_id} {cabinet_id}/>
            </PageLayout>
        }
    }
}
