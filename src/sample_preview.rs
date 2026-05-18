use std::path::Path;
use std::sync::Arc;

use crate::synth::SamplePreviewBuffer;

const MAX_PREVIEW_SECONDS: usize = 12;

#[derive(Clone, Copy, Debug)]
struct WavFormat {
    audio_format: u16,
    channels: u16,
    sample_rate_hz: u32,
    bits_per_sample: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WavPreviewInfo {
    pub(crate) channels: u16,
    pub(crate) sample_rate_hz: u32,
    pub(crate) frame_count: u64,
    pub(crate) duration_seconds: f32,
}

struct ParsedWav<'a> {
    format: WavFormat,
    sample_data: &'a [u8],
}

pub(crate) fn load_wav_preview(path: &Path) -> Result<SamplePreviewBuffer, String> {
    let data = std::fs::read(path)
        .map_err(|err| format!("Sample preview read error: {}: {err}", path.display()))?;
    decode_wav_preview(&data)
        .map_err(|err| format!("Sample preview decode error: {}: {err}", path.display()))
}

pub(crate) fn read_wav_preview_info(path: &Path) -> Result<WavPreviewInfo, String> {
    let data = std::fs::read(path)
        .map_err(|err| format!("Sample metadata read error: {}: {err}", path.display()))?;
    decode_wav_preview_info(&data)
        .map_err(|err| format!("Sample metadata decode error: {}: {err}", path.display()))
}

pub(crate) fn decode_wav_preview(data: &[u8]) -> Result<SamplePreviewBuffer, String> {
    let parsed = parse_wav(data)?;
    decode_samples(parsed.format, parsed.sample_data)
}

pub(crate) fn decode_wav_preview_info(data: &[u8]) -> Result<WavPreviewInfo, String> {
    let parsed = parse_wav(data)?;
    wav_preview_info(parsed.format, parsed.sample_data)
}

fn parse_wav(data: &[u8]) -> Result<ParsedWav<'_>, String> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("expected RIFF/WAVE file".to_string());
    }

    let mut offset = 12;
    let mut format = None;
    let mut sample_data = None;
    while offset + 8 <= data.len() {
        let chunk_id = &data[offset..offset + 4];
        let chunk_size = read_u32(data, offset + 4)? as usize;
        let chunk_start = offset + 8;
        let chunk_end = chunk_start
            .checked_add(chunk_size)
            .ok_or_else(|| "chunk size overflows file length".to_string())?;
        if chunk_end > data.len() {
            return Err("chunk extends past end of file".to_string());
        }

        match chunk_id {
            b"fmt " => format = Some(parse_format_chunk(&data[chunk_start..chunk_end])?),
            b"data" => sample_data = Some(&data[chunk_start..chunk_end]),
            _ => {}
        }

        offset = chunk_end + (chunk_size % 2);
    }

    let format = format.ok_or_else(|| "missing fmt chunk".to_string())?;
    let sample_data = sample_data.ok_or_else(|| "missing data chunk".to_string())?;
    Ok(ParsedWav {
        format,
        sample_data,
    })
}

fn parse_format_chunk(data: &[u8]) -> Result<WavFormat, String> {
    if data.len() < 16 {
        return Err("fmt chunk is too short".to_string());
    }
    let audio_format = read_u16(data, 0)?;
    let channels = read_u16(data, 2)?;
    let sample_rate_hz = read_u32(data, 4)?;
    let bits_per_sample = read_u16(data, 14)?;
    if channels == 0 {
        return Err("channel count is zero".to_string());
    }
    if sample_rate_hz == 0 {
        return Err("sample rate is zero".to_string());
    }
    Ok(WavFormat {
        audio_format,
        channels,
        sample_rate_hz,
        bits_per_sample,
    })
}

fn decode_samples(format: WavFormat, data: &[u8]) -> Result<SamplePreviewBuffer, String> {
    let bytes_per_sample = bytes_per_sample(format)?;
    let channels = format.channels as usize;
    let frame_size = bytes_per_sample
        .checked_mul(channels)
        .ok_or_else(|| "frame size overflows".to_string())?;
    if frame_size == 0 || data.len() < frame_size {
        return Err("no sample frames".to_string());
    }

    let frame_count = data.len() / frame_size;
    let max_frames = (format.sample_rate_hz as usize).saturating_mul(MAX_PREVIEW_SECONDS);
    let frames_to_read = frame_count.min(max_frames.max(1));
    let mut samples = Vec::with_capacity(frames_to_read);
    for frame in 0..frames_to_read {
        let frame_offset = frame * frame_size;
        let mut mono = 0.0_f32;
        for channel in 0..channels {
            let offset = frame_offset + channel * bytes_per_sample;
            mono += decode_sample(format, data, offset)?;
        }
        samples.push((mono / channels as f32).clamp(-1.0, 1.0));
    }

    if samples.is_empty() {
        return Err("no sample frames".to_string());
    }
    Ok(SamplePreviewBuffer {
        samples: Arc::from(samples.into_boxed_slice()),
        sample_rate_hz: format.sample_rate_hz,
    })
}

fn wav_preview_info(format: WavFormat, data: &[u8]) -> Result<WavPreviewInfo, String> {
    let bytes_per_sample = bytes_per_sample(format)?;
    let frame_size = bytes_per_sample
        .checked_mul(format.channels as usize)
        .ok_or_else(|| "frame size overflows".to_string())?;
    if frame_size == 0 {
        return Err("frame size is zero".to_string());
    }
    let frame_count = (data.len() / frame_size) as u64;
    if frame_count == 0 {
        return Err("no sample frames".to_string());
    }
    Ok(WavPreviewInfo {
        channels: format.channels,
        sample_rate_hz: format.sample_rate_hz,
        frame_count,
        duration_seconds: frame_count as f32 / format.sample_rate_hz.max(1) as f32,
    })
}

fn bytes_per_sample(format: WavFormat) -> Result<usize, String> {
    match (format.audio_format, format.bits_per_sample) {
        (1, 8 | 16 | 24 | 32) | (3, 32) => Ok((format.bits_per_sample / 8) as usize),
        (1, bits) => Err(format!("unsupported PCM bit depth: {bits}")),
        (3, bits) => Err(format!("unsupported float bit depth: {bits}")),
        (format_id, _) => Err(format!("unsupported WAV format: {format_id}")),
    }
}

fn decode_sample(format: WavFormat, data: &[u8], offset: usize) -> Result<f32, String> {
    match (format.audio_format, format.bits_per_sample) {
        (1, 8) => Ok((data[offset] as f32 - 128.0) / 128.0),
        (1, 16) => {
            let bytes = read_array::<2>(data, offset)?;
            Ok(i16::from_le_bytes(bytes) as f32 / 32768.0)
        }
        (1, 24) => {
            let bytes = read_array::<3>(data, offset)?;
            let raw = (bytes[0] as i32) | ((bytes[1] as i32) << 8) | ((bytes[2] as i32) << 16);
            let signed = if raw & 0x80_0000 != 0 {
                raw | !0xFF_FFFF
            } else {
                raw
            };
            Ok(signed as f32 / 8_388_608.0)
        }
        (1, 32) => {
            let bytes = read_array::<4>(data, offset)?;
            Ok(i32::from_le_bytes(bytes) as f32 / 2_147_483_648.0)
        }
        (3, 32) => {
            let bytes = read_array::<4>(data, offset)?;
            Ok(f32::from_le_bytes(bytes))
        }
        _ => Err("unsupported sample format".to_string()),
    }
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, String> {
    read_array::<2>(data, offset).map(u16::from_le_bytes)
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    read_array::<4>(data, offset).map(u32::from_le_bytes)
}

fn read_array<const N: usize>(data: &[u8], offset: usize) -> Result<[u8; N], String> {
    data.get(offset..offset + N)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| "unexpected end of file".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_mono_pcm16_wav_preview() {
        let wav = test_wav(
            1,
            1,
            48_000,
            16,
            &[0_i16.to_le_bytes(), 16384_i16.to_le_bytes()].concat(),
        );

        let preview = decode_wav_preview(&wav).expect("wav should decode");

        assert_eq!(preview.sample_rate_hz, 48_000);
        assert_eq!(preview.samples.len(), 2);
        assert!((preview.samples[0] - 0.0).abs() < 0.0001);
        assert!((preview.samples[1] - 0.5).abs() < 0.0001);
    }

    #[test]
    fn mixes_stereo_pcm16_to_mono() {
        let wav = test_wav(
            1,
            2,
            44_100,
            16,
            &[
                32767_i16.to_le_bytes(),
                0_i16.to_le_bytes(),
                0_i16.to_le_bytes(),
                (-32768_i16).to_le_bytes(),
            ]
            .concat(),
        );

        let preview = decode_wav_preview(&wav).expect("wav should decode");

        assert_eq!(preview.samples.len(), 2);
        assert!((preview.samples[0] - 0.5).abs() < 0.0001);
        assert!((preview.samples[1] + 0.5).abs() < 0.0001);
    }

    #[test]
    fn reports_wav_preview_metadata() {
        let sample_data = vec![0_u8; 44_100 * 2 * 2];
        let wav = test_wav(1, 2, 44_100, 16, &sample_data);

        let info = decode_wav_preview_info(&wav).expect("wav metadata should decode");

        assert_eq!(info.channels, 2);
        assert_eq!(info.sample_rate_hz, 44_100);
        assert_eq!(info.frame_count, 44_100);
        assert!((info.duration_seconds - 1.0).abs() < 0.0001);
    }

    #[test]
    fn rejects_non_wav_metadata() {
        let err = decode_wav_preview_info(b"not a wav").expect_err("invalid wav should fail");

        assert_eq!(err, "expected RIFF/WAVE file");
    }

    #[test]
    fn rejects_non_wav_preview() {
        let err = decode_wav_preview(b"not a wav").expect_err("invalid wav should fail");

        assert_eq!(err, "expected RIFF/WAVE file");
    }

    fn test_wav(
        audio_format: u16,
        channels: u16,
        sample_rate: u32,
        bits_per_sample: u16,
        data: &[u8],
    ) -> Vec<u8> {
        let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
        let block_align = channels * bits_per_sample / 8;
        let riff_size = 36 + data.len() as u32;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&riff_size.to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&audio_format.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits_per_sample.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        out
    }
}
