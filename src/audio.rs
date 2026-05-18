#[cfg(feature = "native-app")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(feature = "native-app")]
use std::sync::mpsc::{Receiver, Sender};
#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
use std::{cell::RefCell, rc::Rc, sync::mpsc::Sender};

#[cfg(feature = "native-app")]
use crate::synth::{AudioCommand, SynthEngine, SynthHandle};
#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
use crate::synth::{AudioCommand, SynthHandle};

#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
const WEB_AUDIO_BUFFER_FRAMES: u32 = 1024;

#[cfg(feature = "native-app")]
pub(crate) struct AudioStream {
    stream: cpal::Stream,
}

#[cfg(feature = "native-app")]
impl AudioStream {
    fn new(stream: cpal::Stream) -> Self {
        Self { stream }
    }

    pub(crate) fn play(&self) -> Result<(), String> {
        self.stream.play().map_err(|err| err.to_string())
    }
}

#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
pub(crate) struct AudioStream {
    context: wasm_bindgen::JsValue,
    node: wasm_bindgen::JsValue,
    _callback:
        wasm_bindgen::closure::Closure<dyn FnMut(js_sys::Float32Array, js_sys::Float32Array)>,
}

#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
impl AudioStream {
    fn new(
        context: wasm_bindgen::JsValue,
        node: wasm_bindgen::JsValue,
        callback: wasm_bindgen::closure::Closure<
            dyn FnMut(js_sys::Float32Array, js_sys::Float32Array),
        >,
    ) -> Self {
        Self {
            context,
            node,
            _callback: callback,
        }
    }

    pub(crate) fn play(&self) -> Result<(), String> {
        resume_orbifold_audio_context_js(&self.context).map_err(js_error_message)
    }
}

#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
impl Drop for AudioStream {
    fn drop(&mut self) {
        close_orbifold_audio_stream_js(&self.context, &self.node);
    }
}

#[cfg(not(any(
    feature = "native-app",
    all(feature = "web-app", target_arch = "wasm32")
)))]
pub(crate) struct AudioStream;

#[cfg(not(any(
    feature = "native-app",
    all(feature = "web-app", target_arch = "wasm32")
)))]
impl AudioStream {
    pub(crate) fn play(&self) -> Result<(), String> {
        Err("Browser audio backend is not connected yet".to_string())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AudioOutputDevice {
    pub(crate) name: String,
    pub(crate) is_default: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AudioStreamInfo {
    pub(crate) sample_rate_hz: u32,
    pub(crate) channels: u16,
    pub(crate) sample_format: String,
    pub(crate) buffer_frames: Option<u32>,
}

#[cfg(feature = "native-app")]
pub(crate) fn list_audio_outputs() -> Vec<AudioOutputDevice> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|device| match device.name() {
            Ok(name) => Some(name),
            Err(err) => {
                log::error!("Failed to read default audio output name: {err}");
                None
            }
        });
    let devices = match host.output_devices() {
        Ok(devices) => devices,
        Err(err) => {
            log::error!("Failed to enumerate audio outputs: {err}");
            return Vec::new();
        }
    };

    devices
        .filter_map(|device| match device.name() {
            Ok(name) => Some(name),
            Err(err) => {
                log::error!("Failed to read audio output name while listing devices: {err}");
                None
            }
        })
        .map(|name| {
            let is_default = default_name.as_deref() == Some(name.as_str());
            AudioOutputDevice { name, is_default }
        })
        .collect()
}

#[cfg(not(any(
    feature = "native-app",
    all(feature = "web-app", target_arch = "wasm32")
)))]
pub(crate) fn list_audio_outputs() -> Vec<AudioOutputDevice> {
    Vec::new()
}

#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
pub(crate) fn list_audio_outputs() -> Vec<AudioOutputDevice> {
    if !browser_audio_available_js() {
        return Vec::new();
    }
    vec![AudioOutputDevice {
        name: "Browser audio".to_string(),
        is_default: true,
    }]
}

#[cfg(feature = "native-app")]
pub(crate) fn build_audio_stream(
    synth: &SynthHandle,
    requested_output_name: Option<&str>,
) -> Result<(AudioStream, String, Sender<AudioCommand>, AudioStreamInfo), String> {
    let host = cpal::default_host();
    let (device, device_name) = select_output_device(&host, requested_output_name)?;
    let supported_config = device.default_output_config().map_err(|e| e.to_string())?;
    let sample_rate_hz = supported_config.sample_rate().0;
    let sample_rate = sample_rate_hz as f32;
    let sample_format = supported_config.sample_format();
    let stream_config = supported_config.config();
    let buffer_frames = match stream_config.buffer_size {
        cpal::BufferSize::Default => None,
        cpal::BufferSize::Fixed(frames) => Some(frames),
    };
    let info = AudioStreamInfo {
        sample_rate_hz,
        channels: stream_config.channels,
        sample_format: format!("{sample_format:?}"),
        buffer_frames,
    };
    let (engine, receiver, sender) = synth.make_engine(sample_rate);

    let err_fn = |err| log::error!("Audio stream error: {err}");
    let stream = match sample_format {
        cpal::SampleFormat::I8 => {
            build_stream::<i8>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::I16 => {
            build_stream::<i16>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::I32 => {
            build_stream::<i32>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::I64 => {
            build_stream::<i64>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::U8 => {
            build_stream::<u8>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::U16 => {
            build_stream::<u16>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::U32 => {
            build_stream::<u32>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::U64 => {
            build_stream::<u64>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::F32 => {
            build_stream::<f32>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::F64 => {
            build_stream::<f64>(&device, &stream_config, engine, receiver, err_fn)
        }
        _ => Err(format!("Unsupported sample format: {sample_format:?}")),
    }?;

    Ok((AudioStream::new(stream), device_name, sender, info))
}

#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
pub(crate) fn build_audio_stream(
    synth: &SynthHandle,
    _requested_output_name: Option<&str>,
) -> Result<(AudioStream, String, Sender<AudioCommand>, AudioStreamInfo), String> {
    use wasm_bindgen::JsCast;

    let context_info =
        js_sys::Array::from(&create_orbifold_audio_context_js().map_err(js_error_message)?);
    let context = context_info.get(0);
    if context.is_null() || context.is_undefined() {
        return Err("Web Audio context was not created".to_string());
    }
    let sample_rate_hz = context_info
        .get(1)
        .as_f64()
        .map(|value| value.round().max(1.0) as u32)
        .unwrap_or(48_000);
    let channels = context_info
        .get(2)
        .as_f64()
        .map(|value| value.round().max(1.0) as u16)
        .unwrap_or(2);
    let buffer_frames = context_info
        .get(3)
        .as_f64()
        .map(|value| value.round().max(1.0) as u32)
        .unwrap_or(WEB_AUDIO_BUFFER_FRAMES);

    let (engine, receiver, sender) = synth.make_engine(sample_rate_hz as f32);
    let engine = Rc::new(RefCell::new(engine));
    let receiver = Rc::new(RefCell::new(receiver));
    let callback = wasm_bindgen::closure::Closure::<
        dyn FnMut(js_sys::Float32Array, js_sys::Float32Array),
    >::new({
        let engine = engine.clone();
        let receiver = receiver.clone();
        move |left: js_sys::Float32Array, right: js_sys::Float32Array| {
            let mut engine = engine.borrow_mut();
            for command in receiver.borrow_mut().try_iter() {
                engine.handle_command(command);
            }
            let left_len = left.length();
            let right_len = right.length();
            let frame_count = left_len.max(right_len);
            for index in 0..frame_count {
                let sample = engine.next_sample();
                if index < left_len {
                    left.set_index(index, sample);
                }
                if index < right_len {
                    right.set_index(index, sample);
                }
            }
            engine.update_meter();
        }
    });
    let node = attach_orbifold_audio_processor_js(
        &context,
        callback.as_ref().unchecked_ref(),
        buffer_frames,
    )
    .map_err(js_error_message)?;
    let info = AudioStreamInfo {
        sample_rate_hz,
        channels,
        sample_format: "F32".to_string(),
        buffer_frames: Some(buffer_frames),
    };

    Ok((
        AudioStream::new(context, node, callback),
        "Browser audio".to_string(),
        sender,
        info,
    ))
}

#[cfg(feature = "native-app")]
fn select_output_device(
    host: &cpal::Host,
    requested_output_name: Option<&str>,
) -> Result<(cpal::Device, String), String> {
    if let Some(requested_name) = requested_output_name.filter(|name| !name.is_empty()) {
        let devices = host.output_devices().map_err(|e| e.to_string())?;
        for device in devices {
            match device.name() {
                Ok(name) if name == requested_name => return Ok((device, name)),
                Ok(_) => {}
                Err(err) => {
                    log::error!(
                        "Failed to read audio output name while searching for {requested_name}: {err}"
                    );
                }
            }
        }
        return Err(format!("Audio output not found: {requested_name}"));
    }

    let device = host.default_output_device().ok_or("No output device")?;
    let name = device
        .name()
        .map_err(|err| format!("Failed to read default audio output name: {err}"))?;
    Ok((device, name))
}

#[cfg(feature = "native-app")]
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut engine: SynthEngine,
    receiver: Receiver<AudioCommand>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                for command in receiver.try_iter() {
                    engine.handle_command(command);
                }
                for frame in data.chunks_mut(channels) {
                    let value = engine.next_sample();
                    let sample: T = T::from_sample(value);
                    for out in frame.iter_mut() {
                        *out = sample;
                    }
                }
                engine.update_meter();
            },
            err_fn,
            None,
        )
        .map_err(|e| e.to_string())?;
    Ok(stream)
}

#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
const ORBIFOLD_AUDIO_ERRORS_KEY = "__orbifoldAudioErrors";

function pushOrbifoldAudioError(message) {
  const text = String(message || "").trim();
  if (!text) {
    return;
  }
  window[ORBIFOLD_AUDIO_ERRORS_KEY] = window[ORBIFOLD_AUDIO_ERRORS_KEY] || [];
  window[ORBIFOLD_AUDIO_ERRORS_KEY].push(text);
}

function jsErrorMessage(error) {
  return error && error.message ? error.message : String(error);
}

function recordOrbifoldAudioOutput(left, right) {
  let peak = 0;
  for (let index = 0; index < left.length; index += 1) {
    peak = Math.max(peak, Math.abs(left[index] || 0));
    if (right && right !== left) {
      peak = Math.max(peak, Math.abs(right[index] || 0));
    }
  }
  const dataset = document.body.dataset;
  const previousPeak = Number(dataset.orbifoldAudioPeak || 0);
  const callbackCount = Number(dataset.orbifoldAudioCallbackCount || 0) + 1;
  const frameCount = Number(dataset.orbifoldAudioFrameCount || 0) + left.length;
  dataset.orbifoldAudioPeak = String(Math.max(previousPeak, peak));
  dataset.orbifoldAudioCallbackCount = String(callbackCount);
  dataset.orbifoldAudioFrameCount = String(frameCount);
  if (peak > 0.0001) {
    dataset.orbifoldAudioNonzero = "1";
  }
}

export function browser_audio_available_js() {
  return !!(window.AudioContext || window.webkitAudioContext);
}

export function create_orbifold_audio_context_js() {
  const AudioContextCtor = window.AudioContext || window.webkitAudioContext;
  if (!AudioContextCtor) {
    throw "Web Audio is not available in this browser";
  }
  const context = new AudioContextCtor({ latencyHint: "interactive" });
  document.body.dataset.orbifoldAudioContextCreated = "1";
  document.body.dataset.orbifoldAudioContextState = String(context.state || "");
  const bufferFrames = 1024;
  const channels = Math.max(1, Math.min(2, context.destination.maxChannelCount || 2));
  return [context, context.sampleRate || 48000, channels, bufferFrames];
}

export function attach_orbifold_audio_processor_js(context, callback, bufferFrames) {
  if (!context || !context.createScriptProcessor) {
    throw "ScriptProcessorNode is not available in this browser";
  }
  const frameCount = bufferFrames || 1024;
  const node = context.createScriptProcessor(frameCount, 0, 2);
  node.onaudioprocess = (event) => {
    const output = event.outputBuffer;
    const left = output.getChannelData(0);
    const right = output.numberOfChannels > 1 ? output.getChannelData(1) : left;
    callback(left, right);
    recordOrbifoldAudioOutput(left, right);
  };
  node.connect(context.destination);
  document.body.dataset.orbifoldAudioProcessorAttached = "1";
  document.body.dataset.orbifoldAudioCallbackCount = "0";
  document.body.dataset.orbifoldAudioFrameCount = "0";
  document.body.dataset.orbifoldAudioPeak = "0";
  document.body.dataset.orbifoldAudioNonzero = "0";
  return node;
}

export function resume_orbifold_audio_context_js(context) {
  if (!context) {
    throw "Web Audio context is missing";
  }
  document.body.dataset.orbifoldAudioResumeRequested = "1";
  const result = context.resume();
  if (result && result.then) {
    result.then(() => {
      document.body.dataset.orbifoldAudioResumeResolved = "1";
      document.body.dataset.orbifoldAudioContextState = String(context.state || "");
    });
  }
  if (result && result.catch) {
    result.catch((error) => {
      const message = `Web Audio resume failed: ${jsErrorMessage(error)}`;
      console.error(message, error);
      pushOrbifoldAudioError(message);
    });
  }
}

export function drain_orbifold_audio_errors_js() {
  const errors = window[ORBIFOLD_AUDIO_ERRORS_KEY] || [];
  window[ORBIFOLD_AUDIO_ERRORS_KEY] = [];
  return errors;
}

export function close_orbifold_audio_stream_js(context, node) {
  try {
    if (node) {
      node.onaudioprocess = null;
      node.disconnect();
    }
  } catch (error) {
    const message = `Web Audio node cleanup failed: ${jsErrorMessage(error)}`;
    console.error(message, error);
    pushOrbifoldAudioError(message);
  }
  try {
    if (context && context.state !== "closed") {
      context.close();
    }
  } catch (error) {
    const message = `Web Audio context cleanup failed: ${jsErrorMessage(error)}`;
    console.error(message, error);
    pushOrbifoldAudioError(message);
  }
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = browser_audio_available_js)]
    fn browser_audio_available_js() -> bool;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = create_orbifold_audio_context_js)]
    fn create_orbifold_audio_context_js() -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = attach_orbifold_audio_processor_js)]
    fn attach_orbifold_audio_processor_js(
        context: &wasm_bindgen::JsValue,
        callback: &js_sys::Function,
        buffer_frames: u32,
    ) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = resume_orbifold_audio_context_js)]
    fn resume_orbifold_audio_context_js(
        context: &wasm_bindgen::JsValue,
    ) -> Result<(), wasm_bindgen::JsValue>;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = close_orbifold_audio_stream_js)]
    fn close_orbifold_audio_stream_js(
        context: &wasm_bindgen::JsValue,
        node: &wasm_bindgen::JsValue,
    );

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = drain_orbifold_audio_errors_js)]
    fn drain_orbifold_audio_errors_js() -> wasm_bindgen::JsValue;
}

#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
pub(crate) fn drain_browser_audio_errors() -> Vec<String> {
    let errors = js_sys::Array::from(&drain_orbifold_audio_errors_js());
    let mut out = Vec::new();
    for index in 0..errors.length() {
        if let Some(message) = errors.get(index).as_string()
            && !message.trim().is_empty()
        {
            out.push(message);
        }
    }
    out
}

#[cfg(all(
    feature = "web-app",
    target_arch = "wasm32",
    not(feature = "native-app")
))]
fn js_error_message(value: wasm_bindgen::JsValue) -> String {
    value
        .as_string()
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| format!("{value:?}"))
}
