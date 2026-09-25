use patternfly_yew::prelude::{PageSection, Title};
use yew::{AttrValue, Html, Properties, function_component, html};

#[derive(Properties, PartialEq)]
pub struct PageLayoutProps {
    /// `"<view> – <object>"`, or just the view while the object is loading
    pub title: AttrValue,
    #[prop_or_default]
    pub children: Html,
}

/// Title and content of a routed page, each in its own `PageSection` below the breadcrumb
/// (`AppRoute::content`). Headings inside the content start at `h2`.
#[function_component]
pub fn PageLayout(props: &PageLayoutProps) -> Html {
    html! {
        <>
            <PageSection class="page-title">
                <Title>{props.title.clone()}</Title>
            </PageSection>
            <PageSection>{props.children.clone()}</PageSection>
        </>
    }
}

/// Page title for a view of an object that may still be loading.
pub fn object_title(view: &str, object: Option<impl std::fmt::Display>) -> AttrValue {
    match object {
        Some(object) => format!("{view} – {object}").into(),
        None => AttrValue::from(view.to_string()),
    }
}
