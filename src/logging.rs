pub(crate) fn init() {
    let env = env_logger::Env::default().default_filter_or("warn");
    let mut builder = env_logger::Builder::from_env(env);
    builder.format_timestamp_millis();
    builder.init();
    install_alsa_trace_handler();
    install_jack_trace_handler();
}

#[cfg(target_os = "linux")]
fn install_alsa_trace_handler() {
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int};

    unsafe extern "C" fn alsa_trace_handler(
        file: *const c_char,
        line: c_int,
        function: *const c_char,
        err: c_int,
        fmt: *const c_char,
    ) {
        fn cstr(ptr: *const c_char) -> String {
            if ptr.is_null() {
                return "<unknown>".to_string();
            }
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned()
        }

        log::trace!(
            target: "orbifold::alsa",
            "ALSA diagnostic {}:{}:{} err={} fmt={}",
            cstr(file),
            line,
            cstr(function),
            err,
            cstr(fmt)
        );
    }

    type FixedHandler =
        unsafe extern "C" fn(*const c_char, c_int, *const c_char, c_int, *const c_char);
    type VariadicHandler =
        unsafe extern "C" fn(*const c_char, c_int, *const c_char, c_int, *const c_char, ...);

    // Stable Rust cannot define a C-varargs callback body; the fixed arguments
    // contain enough context to route ALSA diagnostics through normal logging.
    let handler: VariadicHandler =
        unsafe { std::mem::transmute::<FixedHandler, VariadicHandler>(alsa_trace_handler) };
    let result = unsafe { alsa_sys::snd_lib_error_set_handler(Some(handler)) };
    if result != 0 {
        log::error!("Failed to install ALSA diagnostic handler: {result}");
    }
}

#[cfg(not(target_os = "linux"))]
fn install_alsa_trace_handler() {}

#[cfg(target_os = "linux")]
fn install_jack_trace_handler() {
    use std::ffi::CStr;
    use std::os::raw::c_char;
    use std::sync::OnceLock;

    static JACK_LIBRARY: OnceLock<libloading::Library> = OnceLock::new();

    unsafe extern "C" fn jack_trace_handler(message: *const c_char) {
        let message = if message.is_null() {
            "<unknown>".to_string()
        } else {
            unsafe { CStr::from_ptr(message) }
                .to_string_lossy()
                .into_owned()
        };
        log::trace!(target: "orbifold::jack", "JACK diagnostic: {message}");
    }

    type SetJackHandler = unsafe extern "C" fn(Option<unsafe extern "C" fn(*const c_char)>);

    let library = unsafe { libloading::Library::new("libjack.so.0") }
        .or_else(|_| unsafe { libloading::Library::new("libjack.so") });
    let Ok(library) = library else {
        log::trace!(target: "orbifold::jack", "JACK library not available; diagnostic handler not installed");
        return;
    };

    let mut installed = false;
    match unsafe { library.get::<SetJackHandler>(b"jack_set_error_function\0") } {
        Ok(set_error_function) => {
            unsafe { set_error_function(Some(jack_trace_handler)) };
            installed = true;
        }
        Err(err) => {
            log::error!("Failed to find JACK error callback installer: {err}");
        }
    }

    match unsafe { library.get::<SetJackHandler>(b"jack_set_info_function\0") } {
        Ok(set_info_function) => {
            unsafe { set_info_function(Some(jack_trace_handler)) };
            installed = true;
        }
        Err(err) => {
            log::error!("Failed to find JACK info callback installer: {err}");
        }
    }

    if installed && JACK_LIBRARY.set(library).is_err() {
        log::trace!(target: "orbifold::jack", "JACK diagnostic handler already installed");
    }
}

#[cfg(not(target_os = "linux"))]
fn install_jack_trace_handler() {}
