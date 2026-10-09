//! Icons PatternFly's `Icon` doesn't offer: Font Awesome icons by class, and the fiber's states
//! as SVG, for which Font Awesome has nothing fitting.
use yew::{Html, function_component, html};

#[function_component(IconLink)]
pub fn icon_link() -> Html {
    html!(<i class="fas fa-link" aria-hidden="true"></i>)
}

#[function_component(IconUnlink)]
pub fn icon_unlink() -> Html {
    html!(<i class="fas fa-link-slash" aria-hidden="true"></i>)
}

/// A fiber running through: one unbroken line
#[function_component(IconFiberConnected)]
pub fn icon_fiber_connected() -> Html {
    fiber_icon("M64 224h384v64H64z")
}

/// A cut fiber: two line segments with a gap in the middle
#[function_component(IconFiberCut)]
pub fn icon_fiber_cut() -> Html {
    fiber_icon("M64 224h160v64H64zm224 0h160v64H288z")
}

/// An SVG icon in the size and colour of the text around it, like the font icons
fn fiber_icon(path: &'static str) -> Html {
    html! {
        <svg style="vertical-align: -0.125em;" fill="currentColor" height="1em" width="1em" viewBox="0 0 512 512" aria-hidden="true">
            <path d={path}></path>
        </svg>
    }
}
