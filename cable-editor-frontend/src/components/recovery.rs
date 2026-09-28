//! Getting out of a failed request without the browser's reload, which the installed app
//! doesn't have (iOS): the error alerts offer what helps where they are shown.

use patternfly_yew::prelude::{Button, ButtonVariant};
use yew::{
    Callback, Component, Context, ContextProvider, Html, Properties, function_component, html,
    use_context,
};

/// How the user can recover from a failed request, provided to the error alerts below.
/// Without it (e.g. in a dialog, where input would be lost) they offer nothing.
#[derive(Clone, PartialEq)]
pub enum Recovery {
    /// Build the current page anew, which loads its data again (`RetryScope`)
    Retry(Callback<()>),
    /// Only loading the app anew helps (errors before the router)
    Reload,
}

#[derive(Properties, PartialEq)]
pub struct RetryScopeProps {
    #[prop_or_default]
    pub children: Html,
}

/// Provides `Recovery::Retry`, which builds its children anew: every component in it is created
/// again and loads its data, without reloading the app (and logging in again).
pub struct RetryScope {
    generation: u32,
    retry: Recovery,
}

pub enum RetryScopeMsg {
    Retry,
}

impl Component for RetryScope {
    type Message = RetryScopeMsg;
    type Properties = RetryScopeProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            generation: 0,
            retry: Recovery::Retry(ctx.link().callback(|()| RetryScopeMsg::Retry)),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            RetryScopeMsg::Retry => {
                self.generation = self.generation.wrapping_add(1);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <ContextProvider<Recovery> context={self.retry.clone()}>
                // A new key replaces the component and everything below it
                <Remount key={self.generation}>{ctx.props().children.clone()}</Remount>
            </ContextProvider<Recovery>>
        }
    }
}

#[derive(Properties, PartialEq)]
struct RemountProps {
    #[prop_or_default]
    children: Html,
}

#[function_component]
fn Remount(props: &RemountProps) -> Html {
    props.children.clone()
}

#[derive(Properties, PartialEq)]
pub struct ErrorRecoveryProps {
    /// Trying again may help (the connection failed)
    #[prop_or_default]
    pub retry: bool,
    /// Loading the app anew may help (e.g. a new version with a changed schema was deployed)
    #[prop_or_default]
    pub reload: bool,
}

/// The buttons of an error alert, as far as the surrounding `Recovery` allows.
#[function_component]
pub fn ErrorRecovery(props: &ErrorRecoveryProps) -> Html {
    let (retry, reload) = match use_context::<Recovery>() {
        Some(Recovery::Retry(retry)) => (props.retry.then_some(retry), props.reload),
        Some(Recovery::Reload) => (None, true),
        None => (None, false),
    };
    if retry.is_none() && !reload {
        return Html::default();
    }
    // Reloading is the main action when it is the only one
    let reload_variant = if retry.is_some() {
        ButtonVariant::Link
    } else {
        ButtonVariant::Secondary
    };
    html! {
        <div class="error-recovery">
            if let Some(retry) = retry {
                <Button
                    variant={ButtonVariant::Secondary}
                    label="Erneut laden"
                    onclick={retry.reform(|_| ())}
                />
            }
            if reload {
                <Button variant={reload_variant} label="App neu laden" onclick={|_| reload_app()}/>
            }
        </div>
    }
}

/// Loads the app anew, like the browser's reload (the login follows, silently while the
/// provider's session lasts).
pub fn reload_app() {
    if let Some(window) = web_sys::window() {
        // Can only fail for a document without browsing context, which a click can't come from
        let _ = window.location().reload();
    }
}
