use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::mpsc::{Receiver, Sender};

use crate::synth::{AudioCommand, SynthEngine, SynthHandle};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AudioOutputDevice {
    pub(crate) name: String,
    pub(crate) is_default: bool,
}

pub(crate) fn list_audio_outputs() -> Vec<AudioOutputDevice> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|device| device.name().ok());
    let Ok(devices) = host.output_devices() else {
        return Vec::new();
    };

    devices
        .filter_map(|device| device.name().ok())
        .map(|name| {
            let is_default = default_name.as_deref() == Some(name.as_str());
            AudioOutputDevice { name, is_default }
        })
        .collect()
}

pub(crate) fn build_audio_stream(
    synth: &SynthHandle,
    requested_output_name: Option<&str>,
) -> Result<(cpal::Stream, String, Sender<AudioCommand>), String> {
    let host = cpal::default_host();
    let (device, device_name) = select_output_device(&host, requested_output_name)?;
    let supported_config = device.default_output_config().map_err(|e| e.to_string())?;
    let sample_rate = supported_config.sample_rate().0 as f32;
    let sample_format = supported_config.sample_format();
    let stream_config = supported_config.into();
    let (engine, receiver, sender) = synth.make_engine(sample_rate);

    let err_fn = |err| eprintln!("Audio stream error: {err}");
    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            build_stream::<f32>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::I16 => {
            build_stream::<i16>(&device, &stream_config, engine, receiver, err_fn)
        }
        cpal::SampleFormat::U16 => {
            build_stream::<u16>(&device, &stream_config, engine, receiver, err_fn)
        }
        _ => Err("Unsupported sample format".to_string()),
    }?;

    Ok((stream, device_name, sender))
}

fn select_output_device(
    host: &cpal::Host,
    requested_output_name: Option<&str>,
) -> Result<(cpal::Device, String), String> {
    if let Some(requested_name) = requested_output_name.filter(|name| !name.is_empty()) {
        let devices = host.output_devices().map_err(|e| e.to_string())?;
        for device in devices {
            let name = device.name().map_err(|e| e.to_string())?;
            if name == requested_name {
                return Ok((device, name));
            }
        }
        return Err(format!("Audio output not found: {requested_name}"));
    }

    let device = host.default_output_device().ok_or("No output device")?;
    let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    Ok((device, name))
}

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
