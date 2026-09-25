pub mod cabinet;
pub mod cable;
pub mod duct;
pub mod list_of_cables;
pub mod map;
pub mod panel;
pub mod planning;
pub mod router;

use crate::components::label_printer::PrinterStatusBar;
use crate::{
    error::FrontendError,
    graphql::{
        anonymous::{AuthenticationData, AuthenticationQuery},
        query_anonymous,
    },
    pages::router::{AppRoute, RedirectToPlans},
};
use brady_web_sdk::BradyProvider;
use cynic::GraphQlResponse;
use patternfly_yew::prelude::{BackdropViewer, Bullseye, Spinner, ToastViewer};
use yew::{
    Context, Html, Properties, function_component, html, html::IntoPropValue,
    platform::spawn_local, use_effect_with,
};
use yew_nested_router::{Router, Switch};
use yew_oauth2::{
    agent::OAuth2Operations,
    hook::openid::use_auth_agent,
    openid::OAuth2,
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
        if let Some(error) = &self.error {
            let error: Html = error.into_prop_value();
            html!(<Bullseye>{error}</Bullseye>)
        } else if let Some(config) = self.oauth2_config.clone() {
            html! {
                <MainOAuth2 {config}/>
            }
        } else {
            html! {
                <Bullseye><Spinner/></Bullseye>
            }
        }
    }
    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            let scope = ctx.link().clone();
            spawn_local(async move {
                let result = query_anonymous::<AuthenticationQuery, _>(()).await;
                match result {
                    Ok(GraphQlResponse {
                        errors: Some(errors),
                        ..
                    }) => {
                        scope.send_message(AppMessage::Error(FrontendError::Graphql(errors)));
                    }
                    Ok(GraphQlResponse { data, .. }) => {
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
    let scopes = oauth2_config.scopes.clone();
    html! {
     <OAuth2 config={oauth2_config.clone()} {scopes}>
        // Above BackdropViewer, so dialogs can follow the printer status
        <BradyProvider>
            <BackdropViewer>
                <ToastViewer>
                    <Authenticated>
                        <Router<AppRoute>>
                            <Switch<AppRoute> render={AppRoute::content} default={html!(<RedirectToPlans/>)}/>
                        </Router<AppRoute>>
                        <PrinterStatusBar/>
                    </Authenticated>
                    <NotAuthenticated>
                        <AutoLogin/>
                    </NotAuthenticated>
                </ToastViewer>
            </BackdropViewer>
        </BradyProvider>
      </OAuth2>
    }
}

#[function_component(AutoLogin)]
fn auto_login() -> Html {
    let agent = use_auth_agent().expect("Requires OAuth2Context component in parent hierarchy");

    use_effect_with((), move |_| {
        if let Err(err) = agent.start_login() {
            log::warn!("Failed to start login: {err}");
        }
        || ()
    });

    html! {
        <Spinner />
    }
}
