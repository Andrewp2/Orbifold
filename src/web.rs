#[cfg(target_arch = "wasm32")]
pub async fn start_orbifold() -> Result<(), wasm_bindgen::JsValue> {
    let app = crate::ui::web::app_from_browser_settings().await;
    crate::ui::web::run(app).await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn start_orbifold() -> Result<(), wasm_bindgen::JsValue> {
    Err(wasm_bindgen::JsValue::from_str(
        "Orbifold web app is intended for wasm32-unknown-unknown",
    ))
}
