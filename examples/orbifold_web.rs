#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn start_orbifold() -> Result<(), wasm_bindgen::JsValue> {
    orbifold::web::start_orbifold().await
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("orbifold_web is intended for wasm32-unknown-unknown");
}
