use crate::error::FrontendError;
use crate::graphql::authenticated::cable_details::{CableDetails, UpdateCableStructure};
use patternfly_yew::prelude::{Button, ButtonVariant, Form, FormGroup, Spinner, TableHeaderSortBy, TextInput};
use std::sync::Arc;
use yew::html::IntoPropValue;
use yew::suspense::use_future_with;
use yew::{
    Callback, Html, HtmlResult, Properties, Suspense, function_component, html,
    platform::spawn_local, props, use_state,
};
use yew_oauth2::hook::use_auth_state;

#[function_component]
fn CableForm(props: &EditCableProperties) -> HtmlResult {
    let auth_state = use_auth_state();
    //let sort_state = use_state(|| None::<TableHeaderSortBy<crate::pages::list_of_cables::Columns>>);
    let cable_id = props.cable_id;
    let initial_cable_data = use_future_with(auth_state.clone(), |credentials| async move {
        CableDetails::fetch((*credentials).as_ref(), cable_id)
            .await
            .map_err(Arc::new)
    })?;
    let cable_data = use_state(|| {
        <Result<Option<CableDetails>, Arc<FrontendError>> as Clone>::clone(&*initial_cable_data)
    });
    let (details, error) = match &*cable_data {
        Ok(data) => (data.clone(), None),
        Err(e) => (None, Some(IntoPropValue::<Html>::into_prop_value(&**e))),
    };

    let original_data = use_state(|| details.clone());
    let name = use_state(|| details.as_ref().map(|d| d.name.clone()).unwrap_or_default());
    let bundle_count = use_state(|| {
        details
            .as_ref()
            .map(|d| d.bundle_count.to_string())
            .unwrap_or_default()
    });
    let fiber_count = use_state(|| {
        details
            .as_ref()
            .map(|d| d.fiber_count.to_string())
            .unwrap_or_default()
    });

    let on_name_change = {
        let name = name.clone();
        Callback::from(move |value: String| {
            name.set(value);
        })
    };

    let on_bundle_count_change = {
        let bundle_count = bundle_count.clone();
        Callback::from(move |value: String| {
            bundle_count.set(value);
        })
    };

    let on_fiber_count_change = {
        let fiber_count = fiber_count.clone();
        Callback::from(move |value: String| {
            fiber_count.set(value);
        })
    };

    let on_save = {
        let name = name.clone();
        let bundle_count = bundle_count.clone();
        let fiber_count = fiber_count.clone();
        let original_data = original_data.clone();
        let cable_data = cable_data.clone();
        let auth_state = auth_state.clone();
        Callback::from(move |_| {
            if let Some(original) = original_data.as_ref() {
                let update_name = if *name != original.name {
                    Some((*name).clone())
                } else {
                    None
                };

                let update_structure = if let (Ok(new_bundle_count), Ok(new_fiber_count)) =
                    (bundle_count.parse::<i32>(), fiber_count.parse::<i32>())
                {
                    if new_bundle_count != original.bundle_count
                        || new_fiber_count != original.fiber_count
                    {
                        Some(UpdateCableStructure {
                            bundle_count: new_bundle_count,
                            fiber_count: new_fiber_count,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };
                if update_name.is_some() || update_structure.is_some() {
                    let cable_data = cable_data.clone();
                    let auth_state = auth_state.clone();
                    spawn_local(async move {
                        cable_data.set(
                            CableDetails::update_cable(
                                auth_state.as_ref(),
                                cable_id,
                                update_name,
                                update_structure,
                            )
                            .await
                            .map_err(Arc::new),
                        );
                    });
                }
            }
        })
    };

    Ok(html! {
        <Form>
            <FormGroup label="Name">
                <TextInput
                    value={(*name).clone()}
                    onchange={on_name_change}
                />
            </FormGroup>
            <FormGroup label="Anzahl der Bündel">
                <TextInput
                    value={(*bundle_count).clone()}
                    onchange={on_bundle_count_change}
                />
            </FormGroup>
            <FormGroup label="Anzahl der Fasern">
                <TextInput
                    value={(*fiber_count).clone()}
                    onchange={on_fiber_count_change}
                />
            </FormGroup>
            <FormGroup>
                <Button variant={ButtonVariant::Primary}
                    label="Speichern"
                    onclick={on_save}
                />
            </FormGroup>
        </Form>
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
