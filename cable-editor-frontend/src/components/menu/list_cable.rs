use crate::components::load::Load;
use crate::components::menu::{MenuDropdown, MenuEntry, view_load};
use crate::error::FrontendError;
use crate::graphql::authenticated::list_cables::{CableListEntry, fetch_cables_list};
use crate::pages::router::{AppRoute, CableView, PlanView};
use crate::util::get_credentials;
use std::borrow::Cow;
use yew::platform::spawn_local;
use yew::{Component, Context, Html, Properties, html};

pub struct ListCable {
    loaded_cables: Load<Box<[CableListEntry]>>,
}

#[derive(Debug)]
pub enum Msg {
    FetchCables,
    Error(FrontendError),
    UpdateCableList(Box<[CableListEntry]>),
}

#[derive(Properties, PartialEq)]
pub struct ListPlanProps {
    pub plan_id: i32,
    #[prop_or_default]
    pub cable_id: Option<i32>,
    #[prop_or_default]
    pub view: Option<CableView>,
}

impl Component for ListCable {
    type Message = Msg;
    type Properties = ListPlanProps;

    fn create(_ctx: &Context<Self>) -> Self {
        ListCable {
            loaded_cables: Load::Pending,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchCables => {
                self.loaded_cables = Load::Pending;
                let scope = ctx.link().clone();
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    scope.send_message(
                        fetch_cables_list(credentials.as_ref())
                            .await
                            .map_or_else(Msg::Error, Msg::UpdateCableList),
                    );
                });
                true
            }
            Msg::Error(error) => {
                self.loaded_cables = Load::Failed(error);
                true
            }
            Msg::UpdateCableList(list) => {
                self.loaded_cables = Load::Loaded(list);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        view_load(&self.loaded_cables, |cables| {
            let plan_id = ctx.props().plan_id;
            let title = ctx
                .props()
                .cable_id
                .and_then(|cid| cables.iter().find(|cables| cables.id == cid))
                .map(|cable| Cow::Owned(cable.name.clone()))
                .unwrap_or(Cow::Borrowed(" - "));
            let view = ctx.props().view.as_ref().unwrap_or(&CableView::Edit);
            let entries = cables
                .iter()
                .map(|e| MenuEntry {
                    selected: false,
                    text: e.name.clone().into_boxed_str(),
                    target: AppRoute::Plan {
                        plan_id,
                        view: PlanView::Cable {
                            id: e.id,
                            view: view.clone(),
                        },
                    },
                })
                .collect::<Box<[_]>>();
            html! {
                <MenuDropdown {title} {entries}/>
            }
        })
    }
    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchCables);
        }
    }
}
