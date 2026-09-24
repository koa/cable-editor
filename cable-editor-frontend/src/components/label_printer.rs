use crate::error::FrontendError;
use brady_web_sdk::{BradySdk, image_from_canvas, use_brady};
use futures::{
    StreamExt,
    future::{Either, ready, select},
};
use patternfly_yew::prelude::{
    ActionGroup, Alert, AlertType, Backdrop, Bullseye, Button, ButtonType, ButtonVariant, Form,
    FormGroup, Modal, ModalVariant, TextInput, TextInputType, use_backdrop,
};
use std::{future::Future, pin::pin, rc::Rc, time::Duration};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, Storage, window};
use yew::{
    AttrValue, Callback, Html, Properties, UseStateHandle, function_component, html,
    html::IntoPropValue,
    platform::{spawn_local, time::sleep},
    prelude::SubmitEvent,
    use_effect_with, use_state,
};

/// Print zone of the M21-1250-427 across the tape, the SDK scales the label height to it.
const ZONE_HEIGHT_INCH: f64 = 0.43;
/// Text height relative to the print zone without a cable diameter.
const MAX_TEXT_RATIO: f64 = 0.8;
/// Text height relative to the cable diameter, readable from one side.
const DIAMETER_TEXT_RATIO: f64 = 0.7;
const MM_PER_INCH: f64 = 25.4;
/// Last entered cable diameter, kept per browser.
const DIAMETER_KEY: &str = "cable-label-diameter-mm";
const DEFAULT_DPI: f64 = 300.0;
const SUPPLY_TIMEOUT: Duration = Duration::from_secs(10);
const LAMINATED_WIDTH_INCH: f64 = 1.25;
const M211_HEAD_WIDTH_INCH: f64 = 0.63;

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
    html! {
        <>
            <div style="display: flex; align-items: center; gap: 12px;">
                <Button variant={ButtonVariant::Secondary} {label} onclick={toggle} disabled={*busy}/>
                <span>{description}</span>
            </div>
            {error.as_ref().map(IntoPropValue::<Html>::into_prop_value)}
        </>
    }
}

#[derive(Properties, PartialEq)]
pub struct PrintLabelButtonProps {
    pub text: AttrValue,
}

/// Asks for the cable diameter and prints `text` as cable label.
#[function_component]
pub fn PrintLabelButton(props: &PrintLabelButtonProps) -> Html {
    let brady = use_brady().expect("Missing BradyProvider");
    let backdrop = use_backdrop();
    let error = use_state(|| None);
    let busy = use_state(|| false);
    let onclick = {
        let (sdk, text) = (brady.sdk.clone(), props.text.clone());
        let (error, busy) = (error.clone(), busy.clone());
        Callback::from(move |_| {
            let Some(backdrop) = backdrop.clone() else {
                return;
            };
            let onsubmit = {
                let (backdrop, sdk, text) = (backdrop.clone(), sdk.clone(), text.clone());
                let (error, busy) = (error.clone(), busy.clone());
                Callback::from(move |diameter| {
                    backdrop.close();
                    run(
                        &error,
                        &busy,
                        print_label(sdk.clone(), text.clone(), diameter),
                    );
                })
            };
            let oncancel = {
                let backdrop = backdrop.clone();
                Callback::from(move |_| backdrop.close())
            };
            backdrop.open(Backdrop::new(html! {
                <Bullseye>
                    <Modal title="Etikett drucken" variant={ModalVariant::Small}>
                        <DiameterForm {onsubmit} {oncancel}/>
                    </Modal>
                </Bullseye>
            }));
        })
    };
    html! {
        <>
            <Button variant={ButtonVariant::Secondary} label="Drucken" {onclick} disabled={*busy}/>
            {error.as_ref().map(IntoPropValue::<Html>::into_prop_value)}
        </>
    }
}

#[derive(Properties, PartialEq)]
struct DiameterFormProps {
    onsubmit: Callback<Option<f64>>,
    oncancel: Callback<()>,
}

/// Asks for the cable diameter in mm, empty for the largest text.
#[function_component]
fn DiameterForm(props: &DiameterFormProps) -> Html {
    let value = use_state(|| {
        storage()
            .and_then(|s| s.get_item(DIAMETER_KEY).ok().flatten())
            .unwrap_or_default()
    });
    let valid = value.trim().is_empty() || parse_diameter(&value).is_some();
    let onsubmit = {
        let (onsubmit, value) = (props.onsubmit.clone(), value.clone());
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if let Some(storage) = storage() {
                // Only a convenience, ignore failures
                let _ = storage.set_item(DIAMETER_KEY, value.trim());
            }
            onsubmit.emit(parse_diameter(&value));
        })
    };
    let onchange = {
        let value = value.clone();
        Callback::from(move |v| value.set(v))
    };
    html! {
        <Form {onsubmit}>
            <FormGroup label="Kabeldurchmesser (mm)">
                <TextInput r#type={TextInputType::Number} autofocus=true value={(*value).clone()} {onchange} placeholder="leer = maximale Schrift"/>
            </FormGroup>
            <ActionGroup>
                <Button variant={ButtonVariant::Primary} r#type={ButtonType::Submit} label="Drucken" disabled={!valid}/>
                <Button variant={ButtonVariant::Secondary} label="Abbrechen" onclick={props.oncancel.reform(|_| ())}/>
            </ActionGroup>
        </Form>
    }
}

fn parse_diameter(value: &str) -> Option<f64> {
    value
        .trim()
        .replace(',', ".")
        .parse()
        .ok()
        .filter(|d: &f64| *d > 0.0)
}

fn storage() -> Option<Storage> {
    window()?.local_storage().ok().flatten()
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

async fn print_label(
    sdk: Rc<BradySdk>,
    text: AttrValue,
    diameter_mm: Option<f64>,
) -> Result<(), FrontendError> {
    // Connect within the click, Web Bluetooth requires a user gesture
    if !sdk.is_connected() {
        sdk.connect().await?;
    }
    wait_for_supply(&sdk).await?;
    let status = sdk.status();
    let dpi = status.dots_per_inch.unwrap_or(DEFAULT_DPI);
    let image = image_from_canvas(&render_label(&text, dpi, diameter_mm)?).await?;
    let x_offset = zone_correction(status.supply_width);
    Ok(sdk
        .print_all_offset(std::slice::from_ref(&image), x_offset, 0.0)
        .await?)
}

/// The SDK places the print zone of the 1.25" laminated tape (0.81" in) without
/// subtracting the part the M211 head does not cover, so nothing gets printed.
fn zone_correction(supply_width: Option<f64>) -> f64 {
    match supply_width {
        Some(width) if (width - LAMINATED_WIDTH_INCH).abs() < 0.01 => M211_HEAD_WIDTH_INCH - width,
        _ => 0.0,
    }
}

/// Waits until the printer reported its supply, the SDK fails printing before.
async fn wait_for_supply(sdk: &BradySdk) -> Result<(), FrontendError> {
    let supply = sdk
        .updates()
        .any(|status| ready(status.supply_width.is_some()));
    match select(pin!(supply), pin!(sleep(SUPPLY_TIMEOUT))).await {
        Either::Left((true, _)) => Ok(()),
        _ => Err(FrontendError::PrinterNoSupply),
    }
}

/// One line of black text on the print zone, cropped to the glyphs. The text height is
/// capped to a share of the cable diameter. The M211 feeds blank tape around each label.
fn render_label(
    text: &str,
    dpi: f64,
    diameter_mm: Option<f64>,
) -> Result<HtmlCanvasElement, brady_web_sdk::Error> {
    let height = (ZONE_HEIGHT_INCH * dpi).round();
    let ratio = diameter_mm.map_or(MAX_TEXT_RATIO, |d| {
        (DIAMETER_TEXT_RATIO * d / MM_PER_INCH / ZONE_HEIGHT_INCH).min(MAX_TEXT_RATIO)
    });
    let font = |size: f64| format!("bold {size:.1}px sans-serif");
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
    // Scale the font so the glyphs span ratio * height
    context.set_font(&font(height));
    let reference = context.measure_text(text)?;
    let glyphs = reference.actual_bounding_box_ascent() + reference.actual_bounding_box_descent();
    let font = font(height * ratio * height / glyphs.max(1.0));
    context.set_font(&font);
    let metrics = context.measure_text(text)?;
    let left = metrics.actual_bounding_box_left();
    let ascent = metrics.actual_bounding_box_ascent();
    let descent = metrics.actual_bounding_box_descent();
    let width = (left + metrics.actual_bounding_box_right()).ceil();
    canvas.set_width(width as u32);
    canvas.set_height(height as u32);
    // Resizing resets the context state
    context.set_font(&font);
    context.set_fill_style_str("white");
    context.fill_rect(0.0, 0.0, width, height);
    context.set_fill_style_str("black");
    context.fill_text(text, left, (height + ascent - descent) / 2.0)?;
    Ok(canvas)
}
