pub mod error;
pub mod graphql;
pub mod pages;

use crate::pages::App;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}
#[cfg(not(debug_assertions))]
const LOG_LEVEL: log::Level = log::Level::Info;
#[cfg(debug_assertions)]
const LOG_LEVEL: log::Level = log::Level::Trace;
pub fn main() -> Result<(), JsValue> {
    wasm_logger::init(wasm_logger::Config::new(LOG_LEVEL));
    yew::Renderer::<App>::new().render();
    Ok(())
}
