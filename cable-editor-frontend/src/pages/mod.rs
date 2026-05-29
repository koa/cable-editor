pub mod router;

use yew_oauth2::agent::OAuth2Operations;
use crate::graphql::anonymous::{AuthenticationData, AuthenticationQuery};
use crate::graphql::query_anonymous;
use cynic::{http::ReqwestExt, GraphQlResponse, QueryBuilder};
use patternfly_yew::prelude::{BackdropViewer, Brand, Button, MastheadBrand, Nav, NavItem, Page, PageSidebar, ToastViewer};
use web_sys::MouseEvent;
use yew::platform::spawn_local;
use yew::{function_component, html, html_nested, Callback, Context, Html, Properties};
use yew_nested_router::{Router, Switch};
use yew_oauth2::oauth2::{OAuth2, use_auth_agent};
use yew_oauth2::prelude::{Authenticated, NotAuthenticated};
use crate::pages::router::{AppRoute, Sidebar};

#[derive(Debug)]
pub struct App {
    oauth2_config: Option<AuthenticationData>,
}
#[derive(Debug)]
pub enum AppMessage {
    AuthenticationData(AuthenticationData),
}
impl yew::Component for App {
    type Message = AppMessage;
    type Properties = ();
    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            oauth2_config: None,
        }
    }
    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMessage::AuthenticationData(config) => {
                self.oauth2_config = Some(config);
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
                    Err(_) => {}
                }

                /*let result =
                    query_anonymous::<Settings, _>(scope.clone(), settings::Variables {}).await;
                match result {
                    Ok(ResponseData {
                        authentication:
                            SettingsAuthentication {
                                auth_url,
                                client_id,
                                token_url,
                            },
                    }) => {
                        scope.send_message(AppMessage::AuthenticationData(Config::new(
                            client_id, auth_url, token_url,
                        )));
                    }
                    Err(err) => error!("Error on server {err:?}"),
                }*/
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
