use yew::{Html, Properties, classes, function_component, html};

#[derive(Properties, PartialEq)]
pub struct FiberLabelProps {
    pub fiber: u8,
    #[prop_or_default]
    pub children: Html,
}

#[function_component(FiberLabel)]
pub fn fiber_label(props: &FiberLabelProps) -> Html {
    let n = props.fiber;
    let children = props.children.clone();
    if n == 0 {
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
