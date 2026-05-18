// The web entry point compiles the shared desktop app model while browser
// service backends are still being wired, so many native-oriented methods are
// intentionally unused in the wasm-only library build.
#![cfg_attr(
    all(feature = "web-app", not(feature = "native-app")),
    allow(dead_code)
)]

#[cfg(feature = "web-app")]
mod app;
#[cfg(feature = "web-app")]
mod audio;
#[cfg(feature = "web-app")]
mod logging;
#[cfg(feature = "web-app")]
mod midi;
#[cfg(feature = "web-app")]
mod project;
#[cfg(feature = "web-app")]
mod sample_preview;
#[cfg(feature = "web-app")]
mod scala;
#[cfg(feature = "web-app")]
mod scale;
#[cfg(feature = "web-app")]
mod settings;
#[cfg(feature = "web-app")]
mod synth;
#[cfg(feature = "web-app")]
mod time;
#[cfg(feature = "web-app")]
mod ui;

#[cfg(feature = "web-app")]
pub mod web;
