use crate::components::load::Load;
use crate::components::menu::{MenuDropdown, MenuEntry, view_load};
use crate::error::FrontendError;
use crate::graphql::authenticated::list_plans::PlanListEntry;
use crate::pages::router::{AppRoute, PlanView};
use crate::util::get_credentials;
use std::borrow::Cow;
use yew::platform::spawn_local;
use yew::{Component, Context, Html, Properties, html};

pub struct ListPlan {
    loaded_plans: Load<Box<[PlanListEntry]>>,
}
#[derive(Debug)]
pub enum Msg {
    FetchPlans,
    Error(FrontendError),
    UpdatePlanList(Box<[PlanListEntry]>),
}
#[derive(Properties, PartialEq)]
pub struct ListPlanProps {
    #[prop_or_default]
    pub plan_id: Option<i32>,
    #[prop_or_default]
    pub view: Option<PlanView>,
}

impl Component for ListPlan {
    type Message = Msg;
    type Properties = ListPlanProps;

    fn create(_ctx: &Context<Self>) -> Self {
        ListPlan {
            loaded_plans: Load::Pending,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchPlans => {
                let scope = ctx.link().clone();
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    scope.send_message(
                        PlanListEntry::fetch(credentials.as_ref())
                            .await
                            .map_or_else(Msg::Error, Msg::UpdatePlanList),
                    );
                });
                false
            }

            Msg::Error(e) => {
                self.loaded_plans = Load::Failed(e);
                true
            }
            Msg::UpdatePlanList(list) => {
                self.loaded_plans = Load::Loaded(list);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        view_load(&self.loaded_plans, |plans| {
            let title = ctx
                .props()
                .plan_id
                .and_then(|pid| plans.iter().find(|plan| plan.id == pid))
                .map(|plan| Cow::Owned(format!("Planung: {}", plan.name)))
                .unwrap_or(Cow::Borrowed("Planung: - "));
            let view = ctx
                .props()
                .view
                .as_ref()
                .unwrap_or(&PlanView::ListOfCabinets);
            let entries = Some(MenuEntry {
                selected: false,
                text: Box::from("Übersicht"),
                target: AppRoute::ListOfPlans,
            })
            .into_iter()
            .chain(plans.iter().map(|e| MenuEntry {
                selected: false,
                text: e.name.clone().into_boxed_str(),
                target: AppRoute::Plan {
                    plan_id: e.id,
                    view: view.clone(),
                },
            }))
            .collect::<Box<[_]>>();
            html! {
                <MenuDropdown {title} {entries}/>
            }
        })
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchPlans);
        }
    }
}
