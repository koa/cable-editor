use patternfly_yew::prelude::{ButtonVariant, Icon};
use yew::{AttrValue, Callback, MouseEvent, Properties, function_component, html};

#[derive(Properties, PartialEq)]
pub struct IconButtonProps {
    pub icon: Icon,
    /// Name of the action: the accessible name and the tooltip
    pub name: AttrValue,
    #[prop_or(ButtonVariant::Plain)]
    pub variant: ButtonVariant,
    #[prop_or_default]
    pub disabled: bool,
    pub onclick: Callback<MouseEvent>,
}

/// A button showing only a symbol. Named by `aria-label` for screen readers and `title` for the
/// mouse; patternfly-yew's `Button` has no `title`.
#[function_component]
pub fn IconButton(props: &IconButtonProps) -> yew::Html {
    let class = yew::classes!("pf-v6-c-button", props.variant.as_classes());
    html! {
        <button
            {class}
            type="button"
            aria-label={props.name.clone()}
            title={props.name.clone()}
            disabled={props.disabled}
            onclick={props.onclick.clone()}
        >
            <span class="pf-v6-c-button__icon">{props.icon}</span>
        </button>
    }
}
