use crate::error::FrontendError;
use brady_web_sdk::{BradySdk, image_from_canvas, use_brady};
use patternfly_yew::prelude::{Alert, AlertType, Button, ButtonVariant};
use std::{future::Future, rc::Rc};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, window};
use yew::{
    AttrValue, Callback, Html, Properties, UseStateHandle, function_component, html,
    html::IntoPropValue, platform::spawn_local, use_effect_with, use_state,
};

/// Self-laminating cable label tape the layout is made for.
const SUPPLY: &str = "M21-1250-427";
/// Height of the white print zone of SUPPLY.
const ZONE_HEIGHT_INCH: f64 = 0.5;
const DEFAULT_DPI: f64 = 300.0;

/// Printer connection and status, shown above the labels.
#[function_component]
pub fn LabelPrinter() -> Html {
    let brady = use_brady().expect("Missing BradyProvider");
    let error = use_state(|| None);
    let busy = use_state(|| false);
    let supported = use_state(|| true);
    {
        let (sdk, supported) = (brady.sdk.clone(), supported.clone());
        use_effect_with((), move |_| {
            spawn_local(async move { supported.set(sdk.is_supported_browser().await) });
        });
    }
    if !*supported {
        return html!(<Alert inline=true title="Dieser Browser unterstützt kein Web Bluetooth (Chrome oder Edge verwenden)" r#type={AlertType::Info}/>);
    }

    let status = &brady.status;
    let toggle = {
        let (sdk, connected) = (brady.sdk.clone(), status.connected);
        let (error, busy) = (error.clone(), busy.clone());
        Callback::from(move |_| {
            let sdk = sdk.clone();
            run(&error, &busy, async move {
                if connected {
                    sdk.disconnect().await?;
                } else {
                    sdk.connect().await?;
                }
                Ok(())
            });
        })
    };
    let (label, description) = if status.connected {
        let battery = status
            .battery_level_percentage
            .map(|b| format!("Akku {b:.0}%"));
        let description = [
            status.printer_name.clone(),
            status.supply_name.clone(),
            battery,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" | ");
        ("Drucker trennen", description)
    } else {
        ("Drucker verbinden", "Kein Drucker verbunden".to_string())
    };
    let wrong_supply = status
        .supply_y_number
        .as_deref()
        .filter(|y| !y.contains(SUPPLY))
        .map(|y| html!(<Alert inline=true title={format!("Eingelegtes Etikett {y} statt {SUPPLY}")} r#type={AlertType::Warning}/>));
    html! {
        <>
            <div style="display: flex; align-items: center; gap: 12px;">
                <Button variant={ButtonVariant::Secondary} {label} onclick={toggle} disabled={*busy}/>
                <span>{description}</span>
            </div>
            {wrong_supply}
            {error.as_ref().map(IntoPropValue::<Html>::into_prop_value)}
        </>
    }
}

#[derive(Properties, PartialEq)]
pub struct PrintLabelButtonProps {
    pub text: AttrValue,
}

/// Prints `text` as cable label, connecting first if needed.
#[function_component]
pub fn PrintLabelButton(props: &PrintLabelButtonProps) -> Html {
    let brady = use_brady().expect("Missing BradyProvider");
    let error = use_state(|| None);
    let busy = use_state(|| false);
    let onclick = {
        let (sdk, text) = (brady.sdk.clone(), props.text.clone());
        let (error, busy) = (error.clone(), busy.clone());
        Callback::from(move |_| run(&error, &busy, print_label(sdk.clone(), text.clone())))
    };
    html! {
        <>
            <Button variant={ButtonVariant::Secondary} label="Drucken" {onclick} disabled={*busy}/>
            {error.as_ref().map(IntoPropValue::<Html>::into_prop_value)}
        </>
    }
}

/// Runs a printer task, keeping `busy` set and `error` updated.
fn run(
    error: &UseStateHandle<Option<FrontendError>>,
    busy: &UseStateHandle<bool>,
    task: impl Future<Output = Result<(), FrontendError>> + 'static,
) {
    let (error, busy) = (error.clone(), busy.clone());
    busy.set(true);
    spawn_local(async move {
        error.set(task.await.err());
        busy.set(false);
    });
}

async fn print_label(sdk: Rc<BradySdk>, text: AttrValue) -> Result<(), FrontendError> {
    // Connect within the click, Web Bluetooth requires a user gesture
    if !sdk.is_connected() {
        sdk.connect().await?;
    }
    let dpi = sdk.status().dots_per_inch.unwrap_or(DEFAULT_DPI);
    let image = image_from_canvas(&render_label(&text, dpi)?).await?;
    Ok(sdk.print(&image).await?)
}

/// One line of black text filling the print zone height, as long as the text needs.
fn render_label(text: &str, dpi: f64) -> Result<HtmlCanvasElement, brady_web_sdk::Error> {
    let height = (ZONE_HEIGHT_INCH * dpi).round();
    let margin = (height * 0.2).round();
    let font = format!("bold {}px sans-serif", (height * 0.8).round());
    let canvas: HtmlCanvasElement = window()
        .and_then(|w| w.document())
        .expect("Missing Document")
        .create_element("canvas")?
        .dyn_into()
        .map_err(JsValue::from)?;
    let context: CanvasRenderingContext2d = canvas
        .get_context("2d")?
        .and_then(|c| c.dyn_into().ok())
        .expect("Missing 2d context");
    context.set_font(&font);
    let width = (context.measure_text(text)?.width() + 2.0 * margin).ceil();
    canvas.set_width(width as u32);
    canvas.set_height(height as u32);
    // Resizing resets the context state
    context.set_font(&font);
    context.set_fill_style_str("white");
    context.fill_rect(0.0, 0.0, width, height);
    context.set_fill_style_str("black");
    context.set_text_baseline("middle");
    context.fill_text(text, margin, height / 2.0)?;
    Ok(canvas)
}
