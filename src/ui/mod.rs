mod accessibility;
mod actions;
mod labels;
#[cfg(feature = "native-app")]
pub(crate) mod native;
#[cfg(all(feature = "web-app", not(feature = "native-app")))]
pub(crate) mod native;
mod text;
#[cfg(any(test, feature = "web-app"))]
mod text_audit;
mod theme;
#[cfg(all(feature = "web-app", target_arch = "wasm32"))]
pub(crate) mod web;

#[cfg(feature = "native-app")]
pub(crate) use native::run;
