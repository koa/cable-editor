use yew::{Html, Properties, classes, function_component, html};

#[derive(Properties, PartialEq)]
pub struct FiberLabelProps {
    /// Its number in the bundle, which gives its colour
    pub fiber: i32,
    #[prop_or_default]
    pub children: Html,
}

#[function_component(FiberLabel)]
pub fn fiber_label(props: &FiberLabelProps) -> Html {
    let n = props.fiber;
    let children = props.children.clone();
    if n < 1 {
        return children;
    }

    let base_num = ((n - 1) % 12) + 1;
    let is_ring = n > 12;

    let fiber_class = format!("swisscom-fiber-{}", base_num);
    let classes = classes!(
        "pf-v6-c-label",
        "swisscom-fiber-label",
        fiber_class,
        is_ring.then_some("swisscom-fiber-ring")
    );

    html! {
        <span class={classes}>
            <span class="pf-v6-c-label__content">
                { children }
            </span>
        </span>
    }
}

#[derive(Properties, PartialEq)]
pub struct FiberNumberProps {
    pub bundle: i32,
    pub fiber: i32,
}

/// A fiber as "<bundle>-<fiber>" in its colour
#[function_component(FiberNumber)]
pub fn fiber_number(props: &FiberNumberProps) -> Html {
    html! {
        <FiberLabel fiber={props.fiber}>{format!("{}-{}", props.bundle, props.fiber)}</FiberLabel>
    }
}
