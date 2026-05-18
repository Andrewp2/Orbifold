use std::ops::Add;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub(crate) struct AppInstant {
    seconds: f64,
}

impl AppInstant {
    pub(crate) fn now() -> Self {
        Self {
            seconds: monotonic_seconds(),
        }
    }

    pub(crate) fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::from_secs_f64((self.seconds - earlier.seconds).max(0.0))
    }
}

impl Add<Duration> for AppInstant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self {
            seconds: self.seconds + rhs.as_secs_f64(),
        }
    }
}

#[cfg(all(feature = "web-app", target_arch = "wasm32"))]
fn monotonic_seconds() -> f64 {
    js_sys::Date::now() / 1000.0
}

#[cfg(not(all(feature = "web-app", target_arch = "wasm32")))]
fn monotonic_seconds() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;

    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    Instant::now()
        .saturating_duration_since(*ORIGIN.get_or_init(Instant::now))
        .as_secs_f64()
}
