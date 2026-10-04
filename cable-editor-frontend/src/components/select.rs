//! The app's choice of a value: a native select, patternfly-yew's `FormSelect`, which Clippy
//! denies everywhere else (`clippy.toml`).
#![allow(clippy::disallowed_types)]

use patternfly_yew::prelude::{FormSelect, FormSelectOption};
use std::{fmt::Display, str::FromStr};
use yew::{AttrValue, Callback, Html, Properties, function_component, html, html_nested};

#[derive(Properties, PartialEq)]
pub struct SelectProps<K: Clone + PartialEq + 'static> {
    pub value: Option<K>,
    /// The values to choose from, each with its text
    pub options: Box<[(K, String)]>,
    pub onchange: Callback<Option<K>>,
    /// The entry without a value; without it there is always a value
    #[prop_or_default]
    pub placeholder: Option<AttrValue>,
}

/// The options select themselves: `FormSelect` sets the select's value only when the value
/// changes, so an option added later (its data loaded after the value, options depending on
/// another choice) would leave the placeholder shown.
#[function_component(Select)]
pub fn select<K>(props: &SelectProps<K>) -> Html
where
    K: Clone + PartialEq + Display + FromStr + 'static,
{
    html! {
        <FormSelect<K>
            value={props.value.clone()}
            onchange={props.onchange.clone()}
            placeholder={props.placeholder.clone()}
        >
            { for props.options.iter().map(|(value, description)| html_nested! {
                <FormSelectOption<K>
                    value={value.clone()}
                    description={description.clone()}
                    selected={props.value.as_ref() == Some(value)}
                />
            }) }
        </FormSelect<K>>
    }
}
