//! Sound files: WAV (read and write) and MP3 (read).

use crate::Sound;

/// The bytes of a WAV file with one channel of 16 bits.
pub fn write_wav(sound: &Sound) -> Vec<u8> {
    let data_size = (sound.samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_size as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_size).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sound.rate.to_le_bytes());
    out.extend_from_slice(&(sound.rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    for sample in &sound.samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// Reads a WAV file: integers of 8, 16, 24 or 32 bits, or floats of 32 or 64 bits, with any number of channels (mixed into one).
pub fn read_wav(bytes: &[u8]) -> Result<Sound, String> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("This is not a WAV file.".into());
    }
    let (mut format, mut data): (Option<(u16, u16, u32, u16)>, Option<&[u8]>) = (None, None);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]]) as usize;
        let body = &bytes[at + 8..(at + 8 + size).min(bytes.len())];
        if id == b"fmt " && body.len() >= 16 {
            let tag = u16::from_le_bytes([body[0], body[1]]);
            // The extensible format holds the real format in the first bytes of its sub-format.
            let tag = if tag == 0xFFFE && body.len() >= 26 { u16::from_le_bytes([body[24], body[25]]) } else { tag };
            format = Some((tag, u16::from_le_bytes([body[2], body[3]]), u32::from_le_bytes([body[4], body[5], body[6], body[7]]), u16::from_le_bytes([body[14], body[15]])));
        } else if id == b"data" {
            data = Some(body);
        }
        at += 8 + size + (size & 1);
    }
    let ((tag, channels, rate, bits), data) = (format.ok_or("The WAV file has no format.")?, data.ok_or("The WAV file has no sound.")?);
    if channels == 0 || rate == 0 {
        return Err("The WAV file has no channel or no rate.".into());
    }
    let width = (bits / 8) as usize;
    let sample = |chunk: &[u8]| -> Result<f32, String> {
        Ok(match (tag, bits) {
            (1, 8) => (chunk[0] as f32 - 128.0) / 128.0,
            (1, 16) => i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0,
            (1, 24) => (i32::from_le_bytes([0, chunk[0], chunk[1], chunk[2]]) >> 8) as f32 / 8388608.0,
            (1, 32) => i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as f32 / 2147483648.0,
            (3, 32) => f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
            (3, 64) => f64::from_le_bytes(chunk[..8].try_into().unwrap()) as f32,
            _ => return Err(format!("The WAV file has samples of {bits} bits (format {tag}) that this program does not read.")),
        })
    };
    let frame = width * channels as usize;
    let mut mixed = Vec::with_capacity(data.len() / frame.max(1));
    for chunk in data.chunks_exact(frame.max(1)) {
        let mut sum = 0.0;
        for c in 0..channels as usize {
            sum += sample(&chunk[c * width..(c + 1) * width])?;
        }
        mixed.push(sum / channels as f32);
    }
    Ok(Sound { rate, samples: mixed.iter().map(|&v| (v.clamp(-1.0, 1.0) * 32767.0).round() as i16).collect() })
}

/// Reads an MP3 file (the channels are mixed into one).
pub fn read_mp3(bytes: &[u8]) -> Result<Sound, String> {
    let (header, frames) = puremp3::read_mp3(bytes).map_err(|e| format!("This MP3 file cannot be read: {e}"))?;
    let rate = header.sample_rate.hz();
    let samples: Vec<i16> = frames.map(|(left, right)| (((left + right) / 2.0).clamp(-1.0, 1.0) * 32767.0).round() as i16).collect();
    if samples.is_empty() {
        return Err("This MP3 file has no sound.".into());
    }
    Ok(Sound { rate, samples })
}

/// Reads a sound file by what it starts with: WAV, else MP3.
pub fn read(bytes: &[u8]) -> Result<Sound, String> {
    if bytes.starts_with(b"RIFF") {
        read_wav(bytes)
    } else {
        read_mp3(bytes)
    }
}

/// The sound at another rate (a straight line between two samples).
pub fn resample(sound: &Sound, rate: u32) -> Sound {
    if sound.rate == rate || sound.samples.is_empty() {
        return Sound { rate, samples: sound.samples.clone() };
    }
    let count = (sound.samples.len() as u64 * rate as u64 / sound.rate as u64) as usize;
    let step = sound.rate as f64 / rate as f64;
    let last = sound.samples.len() - 1;
    let samples = (0..count)
        .map(|i| {
            let at = i as f64 * step;
            let (a, fraction) = (at as usize, at - at.floor());
            let (x, y) = (sound.samples[a.min(last)] as f64, sound.samples[(a + 1).min(last)] as f64);
            (x + (y - x) * fraction).round() as i16
        })
        .collect();
    Sound { rate, samples }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_wav_file_goes_through_the_writer_and_the_reader() {
        let sound = Sound { rate: 32728, samples: (0..1000).map(|i| (i * 13 % 2000 - 1000) as i16).collect() };
        assert_eq!(read_wav(&write_wav(&sound)).unwrap(), sound);
    }

    #[test]
    fn two_channels_are_mixed_and_a_bad_file_is_refused() {
        let mut wav = write_wav(&Sound { rate: 8000, samples: vec![1000, 3000, -2000, -4000] });
        // The same bytes as a stereo file: two frames of two channels.
        wav[22] = 2;
        let stereo = read_wav(&wav).unwrap();
        assert_eq!(stereo.samples, vec![2000, -3000]);
        assert!(read_wav(b"not a wav").is_err());
    }

    #[test]
    fn the_rate_changes_and_the_length_follows() {
        let sound = Sound { rate: 16000, samples: vec![0; 1600] };
        assert_eq!(resample(&sound, 32000).samples.len(), 3200);
        assert_eq!(resample(&sound, 16000), sound);
    }
}
