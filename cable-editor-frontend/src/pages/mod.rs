pub mod cable;
pub mod duct;
pub mod list_of_cabinets;
mod list_of_cables;
pub mod map;
pub mod router;

use crate::{
    error::FrontendError,
    graphql::{
        anonymous::{AuthenticationData, AuthenticationQuery},
        query_anonymous,
    },
    pages::router::{AppRoute, Sidebar},
};
use cynic::GraphQlResponse;
use patternfly_yew::prelude::{
    BackdropViewer, Brand, Button, MastheadBrand, Page, PageSidebar, ToastViewer,
};
use web_sys::MouseEvent;
use yew::{
    Callback, Context, Html, Properties, function_component, html, html_nested,
    platform::spawn_local,
};
use yew_nested_router::{Router, Switch};
use yew_oauth2::{
    agent::OAuth2Operations,
    oauth2::{OAuth2, use_auth_agent},
    prelude::{Authenticated, NotAuthenticated},
};

#[derive(Debug)]
pub struct App {
    oauth2_config: Option<AuthenticationData>,
    error: Option<FrontendError>,
}
#[derive(Debug)]
pub enum AppMessage {
    AuthenticationData(AuthenticationData),
    Error(FrontendError),
}

impl yew::Component for App {
    type Message = AppMessage;
    type Properties = ();
    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            oauth2_config: None,
            error: None,
        }
    }
    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMessage::AuthenticationData(config) => {
                self.oauth2_config = Some(config);
                true
            }
            AppMessage::Error(e) => {
                self.error = Some(e);
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        if let Some(config) = self.oauth2_config.clone() {
            html! {
                <MainOAuth2 {config}/>
            }
        } else {
            html! {
                <h1>{"Fetching"}</h1>
            }
        }
    }
    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            let scope = ctx.link().clone();
            spawn_local(async move {
                let result = query_anonymous::<AuthenticationQuery, _>(()).await;
                match result {
                    Ok(GraphQlResponse { data, errors }) => {
                        if let Some(AuthenticationQuery { authentication }) = data {
                            scope.send_message(AppMessage::AuthenticationData(authentication));
                        }
                    }
                    Err(e) => {
                        scope.send_message(AppMessage::Error(e));
                    }
                }
            });
        }
    }
}
#[derive(Properties, Clone, PartialEq, Debug)]
pub struct MainOAuth2Props {
    config: AuthenticationData,
}
#[function_component(MainOAuth2)]
pub fn main_oauth2(props: &MainOAuth2Props) -> Html {
    let oauth2_config = &props.config;
    let brand = html! (
        <MastheadBrand>
            <div className="show-light">
                <Brand
                    src="./images/pf-logo.svg"
                    alt="Cable Editor Logo"
                    style="--pf-v6-c-brand--Height: 36px;"
                />
            </div>
        </MastheadBrand>
    );
    html! {
     <OAuth2 config={oauth2_config.clone()}>
        <BackdropViewer>
            <ToastViewer>
                <Authenticated>
                    <Router<AppRoute>>
                        <Page {brand} sidebar={html_nested! {<PageSidebar><Sidebar/></PageSidebar>}}>
                            <Switch<AppRoute>
                                render = { AppRoute::content}
                            />
                        </Page>
                    </Router<AppRoute>>
                </Authenticated>
                <NotAuthenticated>
                    <LoginButton/>
                </NotAuthenticated>
            </ToastViewer>
        </BackdropViewer>
      </OAuth2>
    }
}

#[function_component(LoginButton)]
fn not_authenticated_sidebar() -> Html {
    let agent = use_auth_agent().expect("Requires OAuth2Context component in parent hierarchy");
    let onclick = Callback::from(move |_: MouseEvent| {
        if let Err(err) = agent.start_login() {
            log::warn!("Failed to start login: {err}");
        }
    });
    html! {
        <Button {onclick}>{"Login"}</Button>
    }
}
