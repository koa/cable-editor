use patternfly_yew::prelude::Icon;
use yew::{Callback, function_component, html};

/// Opens the device's print dialog for the whole page (printer or PDF). The only place calling
/// `window.print()`, not to be mixed up with printing a label on the label printer.
#[function_component]
pub fn PrintPageButton() -> yew::Html {
    let onclick = Callback::from(|_| {
        let _ = gloo_utils::window().print();
    });
    html! {
        <button
            class="pf-v6-c-button pf-m-secondary"
            type="button"
            title="Druckdialog des Geräts öffnen (Drucker oder PDF)"
            {onclick}
        >
            <span class="pf-v6-c-button__icon pf-m-start">{Icon::Print}</span>
            {"Seite drucken"}
        </button>
    }
}
