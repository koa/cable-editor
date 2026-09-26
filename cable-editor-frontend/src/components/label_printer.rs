use crate::{
    error::FrontendError,
    icons::{IconLink, IconUnlink},
};
use brady_web_sdk::{Brady, BradySdk, PrinterStatus, image_from_canvas, use_brady};
use futures::{
    StreamExt,
    future::{Either, ready, select},
};
use patternfly_yew::prelude::{
    ActionGroup, Backdrop, Bullseye, Button, ButtonType, ButtonVariant, Form, FormGroup, Icon,
    Modal, ModalVariant, TextInput, TextInputType, use_backdrop,
};
use std::{future::Future, pin::pin, rc::Rc, time::Duration};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, Storage, window};
use yew::{
    AttrValue, Callback, Component, Html, Properties, UseStateHandle, classes, function_component,
    hook, html,
    html::{IntoPropValue, Scope},
    platform::{spawn_local, time::sleep},
    prelude::SubmitEvent,
    use_effect_with, use_memo, use_state,
};

/// Text height relative to the print zone without a cable diameter.
const MAX_TEXT_RATIO: f64 = 0.8;
/// Text height relative to the cable diameter, readable from one side.
const DIAMETER_TEXT_RATIO: f64 = 0.7;
/// Gap between text copies relative to the text height.
const COPY_GAP_RATIO: f64 = 0.5;
const MM_PER_INCH: f64 = 25.4;
/// Last entered cable diameter, kept per browser.
const DIAMETER_KEY: &str = "cable-label-diameter-mm";
const DEFAULT_DPI: f64 = 300.0;
/// DIN-like font (bundled, see index.html), its straight shapes suit the 203 dpi M211.
const LABEL_FONT: &str = "\"Barlow\", sans-serif";
const SUPPLY_TIMEOUT: Duration = Duration::from_secs(10);
// Print head widths and label feeds hard-coded in the SDK
const M211_HEAD_INCH: f64 = 0.63;
const M511_HEAD_INCH: f64 = 1.44;
const M211_FEED_INCH: f64 = 0.87;
const DEFAULT_FEED_INCH: f64 = 0.125;

/// Whether the browser can talk to the printer (Web Bluetooth), false until checked.
#[hook]
pub fn use_printer_supported() -> bool {
    let brady = use_brady().expect("Missing BradyProvider");
    let supported = use_state(|| false);
    {
        let (sdk, supported) = (brady.sdk.clone(), supported.clone());
        use_effect_with((), move |_| {
            spawn_local(async move { supported.set(sdk.is_supported_browser().await) });
        });
    }
    *supported
}

/// `use_printer_supported` for struct components: sends `msg` with the result once checked.
pub fn check_printer_supported<C: Component>(scope: &Scope<C>, msg: fn(bool) -> C::Message) {
    if let Some((brady, _)) = scope.context::<Brady>(Callback::noop()) {
        let scope = scope.clone();
        spawn_local(async move { scope.send_message(msg(brady.sdk.is_supported_browser().await)) });
    }
}

/// Printer connection and status, fixed at the bottom of every page.
#[function_component]
pub fn PrinterStatusBar() -> Html {
    let brady = use_brady().expect("Missing BradyProvider");
    let supported = use_printer_supported();
    let error = use_state(|| None);
    let busy = use_state(|| false);
    if !supported {
        return Html::default();
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
    let (label, icon, description) = if status.connected {
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
        ("Drucker trennen", html!(<IconUnlink/>), description)
    } else {
        (
            "Drucker verbinden",
            html!(<IconLink/>),
            "Kein Drucker verbunden".to_string(),
        )
    };
    html! {
        <>
            <div class="printer-status-bar-spacer"/>
            <div class="printer-status-bar">
                {error_view(&error)}
                <div class="printer-status-bar__row">
                    <button type="button" class="pf-v6-c-button pf-m-plain" aria-label={label} title={label} onclick={toggle} disabled={*busy}>
                        <span class="pf-v6-c-button__icon">{icon}</span>
                    </button>
                    <span class="printer-status-bar__text" title={description.clone()}>{description}</span>
                </div>
            </div>
        </>
    }
}

/// One text choice for a label, e.g. the panel name or its whole path.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelText {
    /// Name of the choice shown in the dialog.
    pub label: AttrValue,
    pub text: AttrValue,
}

impl LabelText {
    pub fn new(label: impl Into<AttrValue>, text: impl Into<AttrValue>) -> Self {
        LabelText {
            label: label.into(),
            text: text.into(),
        }
    }

    /// A label without choice.
    pub fn single(text: impl Into<AttrValue>) -> Rc<[LabelText]> {
        Rc::new([LabelText::new("Text", text)])
    }
}

#[derive(Properties, PartialEq)]
pub struct PrintLabelButtonProps {
    /// Text choices, the first is preselected. The dialog only offers a choice for several.
    pub texts: Rc<[LabelText]>,
    /// Ask for the cable diameter to cap the text height, else print the largest text.
    #[prop_or(true)]
    pub diameter: bool,
    #[prop_or(AttrValue::Static("Drucken"))]
    pub label: AttrValue,
}

/// Connects, asks for the text choice and cable diameter and prints the label.
#[function_component]
pub fn PrintLabelButton(props: &PrintLabelButtonProps) -> Html {
    let brady = use_brady().expect("Missing BradyProvider");
    let backdrop = use_backdrop();
    let error = use_state(|| None);
    let busy = use_state(|| false);
    let onclick = {
        let (sdk, texts, diameter) = (brady.sdk.clone(), props.texts.clone(), props.diameter);
        let (error, busy) = (error.clone(), busy.clone());
        Callback::from(move |_| {
            let Some(backdrop) = backdrop.clone() else {
                return;
            };
            let (sdk, texts) = (sdk.clone(), texts.clone());
            let (print_error, print_busy) = (error.clone(), busy.clone());
            run(&error, &busy, async move {
                // Fails early if the printer or tape is unusable, the dialog follows changes
                prepare(&sdk).await?;
                let onsubmit = {
                    let backdrop = backdrop.clone();
                    Callback::from(move |(text, diameter, geometry)| {
                        backdrop.close();
                        let task = print_label(sdk.clone(), text, diameter, geometry);
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
                            <LabelForm {texts} {diameter} {onsubmit} {oncancel}/>
                        </Modal>
                    </Bullseye>
                }));
                Ok(())
            });
        })
    };
    html! {
        <>
            <Button variant={ButtonVariant::Secondary} label={props.label.to_string()} {onclick} disabled={*busy}/>
            {error_view(&error)}
        </>
    }
}

#[derive(Properties, PartialEq)]
struct LabelFormProps {
    texts: Rc<[LabelText]>,
    diameter: bool,
    onsubmit: Callback<(AttrValue, Option<f64>, LabelGeometry)>,
    oncancel: Callback<()>,
}

/// Offers the text choices, asks for the cable diameter in mm (empty for the largest text)
/// and previews the label.
#[function_component]
fn LabelForm(props: &LabelFormProps) -> Html {
    // Rendered by the BackdropViewer, the BradyProvider sits above it
    let brady = use_brady().expect("Missing BradyProvider");
    let selected = use_state(|| 0);
    let value = use_state(|| {
        storage()
            .and_then(|s| s.get_item(DIAMETER_KEY).ok().flatten())
            .unwrap_or_default()
    });
    let text = props
        .texts
        .get(*selected)
        .map(|t| t.text.clone())
        .unwrap_or(AttrValue::Static(""));
    let diameter = props.diameter.then(|| parse_diameter(&value)).flatten();
    // Follows the tape, e.g. after a cartridge change while the dialog is open
    let geometry = LabelGeometry::from_status(&brady.status);
    let valid =
        geometry.is_ok() && (!props.diameter || value.trim().is_empty() || diameter.is_some());
    let preview = use_memo(
        (text.clone(), diameter, geometry.as_ref().ok().copied()),
        |(text, diameter, geometry)| {
            let geometry = (*geometry)?;
            Some((|| {
                let canvas = render_label(text, *diameter, &geometry)?;
                let length =
                    f64::from(canvas.width()) / f64::from(canvas.height()) * geometry.band_inch;
                let (_, copies) = text_layout(*diameter, geometry.band_inch);
                Ok::<_, brady_web_sdk::Error>((
                    canvas.to_data_url()?,
                    length * MM_PER_INCH,
                    copies,
                    geometry.feed_inch,
                ))
            })())
        },
    );
    let preview = match (&geometry, &*preview) {
        (Err(e), _) => e.into_prop_value(),
        (Ok(_), None) => Html::default(),
        (Ok(_), Some(Ok((src, length, copies, feed_inch)))) => html! {
            <>
                <img src={src.clone()} alt={text.clone()} style="display: block; max-width: 100%; max-height: 48px; border: 1px solid var(--pf-t--global--border--color--default);"/>
                {format!(
                    "Etikett ca. {length:.0} mm{} (+{:.0} mm Vorschub)",
                    if *copies > 1 { format!(", Text {copies}×") } else { String::new() },
                    feed_inch * MM_PER_INCH,
                )}
            </>
        },
        (Ok(_), Some(Err(e))) => (&FrontendError::from(e.clone())).into_prop_value(),
    };
    let choices = (props.texts.len() > 1).then(|| {
        let items = props.texts.iter().enumerate().map(|(index, choice)| {
            let active = index == *selected;
            let onclick = {
                let selected = selected.clone();
                Callback::from(move |_| selected.set(index))
            };
            html! {
                <div class="pf-v6-c-toggle-group__item">
                    <button type="button" class={classes!("pf-v6-c-toggle-group__button", active.then_some("pf-m-selected"))} aria-pressed={active.to_string()} {onclick}>
                        <span class="pf-v6-c-toggle-group__text">{choice.label.clone()}</span>
                    </button>
                </div>
            }
        });
        html! {
            <FormGroup label="Text">
                <div class="pf-v6-c-toggle-group">{for items}</div>
            </FormGroup>
        }
    });
    let onsubmit = {
        let (onsubmit, value, has_diameter) =
            (props.onsubmit.clone(), value.clone(), props.diameter);
        let (text, geometry) = (text.clone(), geometry.as_ref().ok().copied());
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            // Enter submits even while the print button is disabled
            let Some(geometry) = geometry else {
                return;
            };
            let mut diameter = None;
            if has_diameter {
                if let Some(storage) = storage() {
                    // Only a convenience, ignore failures
                    let _ = storage.set_item(DIAMETER_KEY, value.trim());
                }
                diameter = parse_diameter(&value);
            }
            onsubmit.emit((text.clone(), diameter, geometry));
        })
    };
    let onchange = {
        let value = value.clone();
        Callback::from(move |v| value.set(v))
    };
    html! {
        <Form {onsubmit}>
            {choices}
            if props.diameter {
                <FormGroup label="Kabeldurchmesser (mm)">
                    <TextInput r#type={TextInputType::Number} autofocus=true value={(*value).clone()} {onchange} placeholder="leer = maximale Schrift"/>
                </FormGroup>
            }
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

/// The printer error, if any, with a button to dismiss it.
fn error_view(error: &UseStateHandle<Option<FrontendError>>) -> Html {
    let Some(e) = error.as_ref() else {
        return Html::default();
    };
    let ondismiss = {
        let error = error.clone();
        Callback::from(move |_| error.set(None))
    };
    html! {
        <div class="printer-error">
            {IntoPropValue::<Html>::into_prop_value(e)}
            <button type="button" class="pf-v6-c-button pf-m-plain" aria-label="Schliessen" title="Schliessen" onclick={ondismiss}>
                <span class="pf-v6-c-button__icon">{Icon::Times}</span>
            </button>
        </div>
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
        if !status.connected {
            return Err(FrontendError::PrinterDisconnected);
        }
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
    // The canvas silently falls back to another font until the label font is loaded
    if let Err(e) = load_label_font().await {
        log::warn!("Label font not loaded: {e:?}");
    }
    LabelGeometry::from_status(&sdk.status())
}

async fn load_label_font() -> Result<(), JsValue> {
    let document = window()
        .and_then(|w| w.document())
        .expect("Missing Document");
    JsFuture::from(document.fonts().load(&label_font(16.0))).await?;
    Ok(())
}

fn label_font(size: f64) -> String {
    format!("bold {size:.1}px {LABEL_FONT}")
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

/// Text height relative to the print band and how many copies of the text fit across it.
/// The text height is capped to a share of the cable diameter; small text is repeated so
/// it can be read from more sides once the band is wrapped around the cable.
fn text_layout(diameter_mm: Option<f64>, band_inch: f64) -> (f64, u32) {
    let ratio = diameter_mm.map_or(MAX_TEXT_RATIO, |d| {
        (DIAMETER_TEXT_RATIO * d / MM_PER_INCH / band_inch).min(MAX_TEXT_RATIO)
    });
    // Copies with gaps stay within the area a single maximal line takes:
    // n * ratio + (n - 1) * gap * ratio <= MAX_TEXT_RATIO
    let copies =
        ((MAX_TEXT_RATIO + COPY_GAP_RATIO * ratio) / (ratio * (1.0 + COPY_GAP_RATIO))).floor();
    (ratio, copies.max(1.0) as u32)
}

/// Black text on the print band, repeated across it for small text,
/// cropped to the glyphs along the tape. The printer feeds blank tape around each label.
fn render_label(
    text: &str,
    diameter_mm: Option<f64>,
    geometry: &LabelGeometry,
) -> Result<HtmlCanvasElement, brady_web_sdk::Error> {
    let band = geometry.band_inch;
    let height = (band * geometry.dpi).round();
    let (ratio, copies) = text_layout(diameter_mm, band);
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
    context.set_font(&label_font(height));
    let reference = context.measure_text(text)?;
    let glyphs = reference.actual_bounding_box_ascent() + reference.actual_bounding_box_descent();
    let font = label_font(height * ratio * height / glyphs.max(1.0));
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
    // Spread the copies over the area of a maximal line, centered on the band
    let step = if copies > 1 {
        (MAX_TEXT_RATIO - ratio) / f64::from(copies - 1)
    } else {
        0.0
    };
    for copy in 0..copies {
        let offset = f64::from(copy) - f64::from(copies - 1) / 2.0;
        let center = height * (0.5 + offset * step);
        context.fill_text(text, left, center + (ascent - descent) / 2.0)?;
    }
    Ok(canvas)
}

#[derive(Properties, PartialEq)]
pub struct PanelLabelButtonProps {
    pub id: i32,
    pub name: Option<String>,
    /// Names of the parent panels, root first.
    pub parents: Vec<String>,
}

/// Prints the panel name or its path from the root panel as label.
#[function_component]
pub fn PanelLabelButton(props: &PanelLabelButtonProps) -> Html {
    let printing = use_printer_supported();
    let texts = use_memo(
        (props.id, props.name.clone(), props.parents.clone()),
        |(id, name, parents)| {
            let name = name.clone().unwrap_or_else(|| format!("Panel {id}"));
            let path = parents
                .iter()
                .map(String::as_str)
                .chain([name.as_str()])
                .collect::<Vec<_>>()
                .join(" - ");
            let mut texts = vec![LabelText::new("Name", name.clone())];
            if path != name {
                texts.push(LabelText::new("Pfad", path));
            }
            Rc::<[LabelText]>::from(texts)
        },
    );
    if !printing {
        return Html::default();
    }
    html! {
        <PrintLabelButton texts={(*texts).clone()} diameter=false label="Etikett drucken"/>
    }
}
