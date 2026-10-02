pub mod components;
pub mod error;
pub mod geo;
pub mod graphql;
pub mod icons;
pub mod pages;
pub mod util;

use crate::pages::App;
use wasm_bindgen::JsValue;

#[cfg(not(debug_assertions))]
const LOG_LEVEL: log::Level = log::Level::Info;
#[cfg(debug_assertions)]
const LOG_LEVEL: log::Level = log::Level::Trace;
pub fn main() -> Result<(), JsValue> {
    // Without this, a panic (an indexing bug, an `unwrap` inside a dependency, ...) traps the
    // wasm instance with no message at all, not even in the console.
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::new(LOG_LEVEL));
    yew::Renderer::<App>::new().render();
    Ok(())
}
