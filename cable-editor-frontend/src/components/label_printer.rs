use crate::error::FrontendError;
use brady_web_sdk::{BradySdk, PrinterStatus, image_from_canvas, use_brady};
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
    use_effect_with, use_memo, use_state,
};

/// Text height relative to the print zone without a cable diameter.
const MAX_TEXT_RATIO: f64 = 0.8;
/// Text height relative to the cable diameter, readable from one side.
const DIAMETER_TEXT_RATIO: f64 = 0.7;
const MM_PER_INCH: f64 = 25.4;
/// Last entered cable diameter, kept per browser.
const DIAMETER_KEY: &str = "cable-label-diameter-mm";
const DEFAULT_DPI: f64 = 300.0;
const SUPPLY_TIMEOUT: Duration = Duration::from_secs(10);
// Print head widths and label feeds hard-coded in the SDK
const M211_HEAD_INCH: f64 = 0.63;
const M511_HEAD_INCH: f64 = 1.44;
const M211_FEED_INCH: f64 = 0.87;
const DEFAULT_FEED_INCH: f64 = 0.125;

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

/// Connects, asks for the cable diameter and prints `text` as cable label.
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
            let (sdk, text) = (sdk.clone(), text.clone());
            let (print_error, print_busy) = (error.clone(), busy.clone());
            run(&error, &busy, async move {
                let geometry = prepare(&sdk).await?;
                let onsubmit = {
                    let (backdrop, text) = (backdrop.clone(), text.clone());
                    Callback::from(move |diameter| {
                        backdrop.close();
                        let task = print_label(sdk.clone(), text.clone(), diameter, geometry);
                        run(&print_error, &print_busy, task);
                    })
                };
                let oncancel = {
                    let backdrop = backdrop.clone();
                    Callback::from(move |_| backdrop.close())
                };
                backdrop.open(Backdrop::new(html! {
                    <Bullseye>
                        <Modal title="Etikett drucken" variant={ModalVariant::Small}>
                            <DiameterForm {text} {geometry} {onsubmit} {oncancel}/>
                        </Modal>
                    </Bullseye>
                }));
                Ok(())
            });
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
    text: AttrValue,
    geometry: LabelGeometry,
    onsubmit: Callback<Option<f64>>,
    oncancel: Callback<()>,
}

/// Asks for the cable diameter in mm (empty for the largest text) and previews the label.
#[function_component]
fn DiameterForm(props: &DiameterFormProps) -> Html {
    let value = use_state(|| {
        storage()
            .and_then(|s| s.get_item(DIAMETER_KEY).ok().flatten())
            .unwrap_or_default()
    });
    let diameter = parse_diameter(&value);
    let valid = value.trim().is_empty() || diameter.is_some();
    let preview = use_memo(
        (props.text.clone(), diameter, props.geometry),
        |(text, diameter, geometry)| {
            let canvas = render_label(text, *diameter, geometry)?;
            let length =
                f64::from(canvas.width()) / f64::from(canvas.height()) * geometry.band_inch;
            Ok::<_, brady_web_sdk::Error>((canvas.to_data_url()?, length * MM_PER_INCH))
        },
    );
    let preview = match &*preview {
        Ok((src, length)) => html! {
            <>
                <img src={src.clone()} alt={props.text.clone()} style="display: block; max-width: 100%; max-height: 48px; border: 1px solid #8a8d90;"/>
                {format!("Etikett ca. {length:.0} mm (+{:.0} mm Vorschub)", props.geometry.feed_inch * MM_PER_INCH)}
            </>
        },
        Err(e) => (&FrontendError::from(e.clone())).into_prop_value(),
    };
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
            <FormGroup label="Vorschau">
                {preview}
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

/// Label layout for the installed tape, mirroring how the SDK places the image.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LabelGeometry {
    /// Width across the tape the SDK scales the image height to.
    band_inch: f64,
    /// Print offset across the tape.
    x_offset_inch: f64,
    /// Blank tape fed per label.
    feed_inch: f64,
    dpi: f64,
}

impl LabelGeometry {
    fn from_status(status: &PrinterStatus) -> Result<Self, FrontendError> {
        let width = status.supply_width.ok_or(FrontendError::PrinterNoSupply)?;
        if status.media_is_die_cut {
            return Err(FrontendError::UnsupportedTape);
        }
        let model = status.printer_model.as_deref();
        let covered = match model {
            Some("M211") => width.min(M211_HEAD_INCH),
            Some("M511") => width.min(M511_HEAD_INCH),
            _ => width,
        };
        let (band_inch, x_offset_inch) = match status.print_zone {
            // SDK places the zone without subtracting the tape left of the head
            Some(zone) => (zone.width, covered - width),
            None => (covered, 0.0),
        };
        Ok(LabelGeometry {
            band_inch,
            x_offset_inch,
            feed_inch: match model {
                Some("M211") => M211_FEED_INCH,
                _ => DEFAULT_FEED_INCH,
            },
            dpi: status.dots_per_inch.unwrap_or(DEFAULT_DPI),
        })
    }
}

/// Connects if needed and derives the label layout from the reported tape.
async fn prepare(sdk: &BradySdk) -> Result<LabelGeometry, FrontendError> {
    // Connect within the click, Web Bluetooth requires a user gesture
    if !sdk.is_connected() {
        sdk.connect().await?;
    }
    wait_for_supply(sdk).await?;
    LabelGeometry::from_status(&sdk.status())
}

async fn print_label(
    sdk: Rc<BradySdk>,
    text: AttrValue,
    diameter_mm: Option<f64>,
    geometry: LabelGeometry,
) -> Result<(), FrontendError> {
    log::debug!("Label {text:?} with {geometry:?} on {:?}", sdk.status());
    let image = image_from_canvas(&render_label(&text, diameter_mm, &geometry)?).await?;
    Ok(sdk
        .print_all_offset(std::slice::from_ref(&image), geometry.x_offset_inch, 0.0)
        .await?)
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

/// One line of black text on the print band, cropped to the glyphs. The text height is
/// capped to a share of the cable diameter. The printer feeds blank tape around each label.
fn render_label(
    text: &str,
    diameter_mm: Option<f64>,
    geometry: &LabelGeometry,
) -> Result<HtmlCanvasElement, brady_web_sdk::Error> {
    let band = geometry.band_inch;
    let height = (band * geometry.dpi).round();
    let ratio = diameter_mm.map_or(MAX_TEXT_RATIO, |d| {
        (DIAMETER_TEXT_RATIO * d / MM_PER_INCH / band).min(MAX_TEXT_RATIO)
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
