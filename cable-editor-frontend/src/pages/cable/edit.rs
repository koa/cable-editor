use crate::graphql::authenticated::cable_details::CableDetails;
use patternfly_yew::prelude::{Spinner, TableHeaderSortBy};
use yew::suspense::use_future_with;
use yew::{Html, HtmlResult, Properties, Suspense, function_component, html, props, use_state};
use yew_oauth2::hook::use_auth_state;

#[function_component]
fn CableForm(props: &EditCableProperties) -> HtmlResult {
    let auth_state = use_auth_state();
    //let sort_state = use_state(|| None::<TableHeaderSortBy<crate::pages::list_of_cables::Columns>>);
    let cable_id = props.cable_id;
    let cable_data = use_future_with(auth_state, |credentials| async move {
        CableDetails::fetch((*credentials).as_ref(), cable_id).await
    })?;

    Ok(html! {
        <div>{ format!("Ich bin das Kabel mit der ID: {}", props.cable_id) }</div>
    })
}

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct EditCableProperties {
    pub cable_id: i32,
}

#[function_component]
pub fn EditCable(props: &EditCableProperties) -> Html {
    let fallback = html!(<Spinner/>);
    html! {
        <Suspense {fallback}>
            <CableForm ..props.clone()/>
        </Suspense>
    }
}
