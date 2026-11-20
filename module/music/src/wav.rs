//! Sound files: WAV (read and write, with the loop in a `smpl` chunk) and MP3 (read).
//!
//! The sounds have one or two channels. A file with more channels keeps its first two.

use crate::bcstm::Music;

/// The most channels that Studio keeps.
const MAX_CHANNELS: usize = 2;

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    bytes.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Writes a WAV file of 16 bits. A sound with a loop gets a `smpl` chunk that holds the start of the loop and the end of the sound.
pub fn write_wav(music: &Music) -> Vec<u8> {
    let channels = music.channels.len().max(1) as u32;
    let frames = music.frames();
    let data_size = (frames as u32) * channels * 2;
    let smpl_size = if music.loop_start.is_some() { 8 + 36 + 24 } else { 0 };
    let mut out = Vec::with_capacity(44 + data_size as usize + smpl_size);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_size + smpl_size as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(channels as u16).to_le_bytes());
    out.extend_from_slice(&music.rate.to_le_bytes());
    out.extend_from_slice(&(music.rate * channels * 2).to_le_bytes());
    out.extend_from_slice(&((channels * 2) as u16).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    for i in 0..frames {
        for channel in &music.channels {
            out.extend_from_slice(&channel[i].to_le_bytes());
        }
    }
    if let Some(start) = music.loop_start {
        out.extend_from_slice(b"smpl");
        out.extend_from_slice(&60u32.to_le_bytes());
        // Manufacturer, product, sample period, MIDI note, fraction, SMPTE format and offset, one loop, no extra data.
        for value in [0u32, 0, 1_000_000_000 / music.rate.max(1), 60, 0, 0, 0, 1, 0] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        // The loop: identifier, type (forward), start, end, fraction, count (0 = for ever).
        for value in [0u32, 0, start, frames.saturating_sub(1) as u32, 0, 0] {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out
}

/// Reads a WAV file: PCM of 8, 16, 24 or 32 bits, or float of 32 bits. The loop comes from a `smpl` chunk when there is one.
pub fn read_wav(bytes: &[u8]) -> Result<Music, String> {
    if bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err("This is not a WAV file.".into());
    }
    let mut format = None;
    let mut data = None;
    let mut loop_start = None;
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let size = u32_at(bytes, at + 4).unwrap_or(0) as usize;
        let body = at + 8;
        let end = (body + size).min(bytes.len());
        match &bytes[at..at + 4] {
            b"fmt " => {
                let tag = u16_at(bytes, body).ok_or("The format chunk is cut.")?;
                let channels = u16_at(bytes, body + 2).ok_or("The format chunk is cut.")? as usize;
                let rate = u32_at(bytes, body + 4).ok_or("The format chunk is cut.")?;
                let bits = u16_at(bytes, body + 14).ok_or("The format chunk is cut.")?;
                // 0xFFFE is the extensible format: its real tag is in the first bytes of the sub-format.
                let tag = if tag == 0xFFFE { u16_at(bytes, body + 24).unwrap_or(1) } else { tag };
                format = Some((tag, channels, rate, bits));
            }
            b"data" => data = Some(&bytes[body..end]),
            b"smpl" if end - body >= 36 + 24 && u32_at(bytes, body + 28).unwrap_or(0) >= 1 => loop_start = u32_at(bytes, body + 44),
            _ => {}
        }
        at = body + size + (size & 1);
    }
    let (tag, channels, rate, bits) = format.ok_or("This WAV file has no format chunk.")?;
    let data = data.ok_or("This WAV file has no sound.")?;
    if channels == 0 || rate == 0 {
        return Err("This WAV file has no channel or no rate.".into());
    }
    let width = (bits / 8) as usize;
    if !matches!((tag, bits), (1, 8) | (1, 16) | (1, 24) | (1, 32) | (3, 32)) {
        return Err(format!("This WAV file is not supported (format {tag}, {bits} bits): use PCM of 8, 16, 24 or 32 bits."));
    }
    let frames = data.len() / (width * channels);
    let kept = channels.min(MAX_CHANNELS);
    let mut out = vec![Vec::with_capacity(frames); kept];
    for frame in 0..frames {
        for (c, channel) in out.iter_mut().enumerate() {
            let at = (frame * channels + c) * width;
            let sample = match (tag, bits) {
                (1, 8) => ((data[at] as i16 - 128) << 8) as i16,
                (1, 16) => i16::from_le_bytes([data[at], data[at + 1]]),
                (1, 24) => i16::from_le_bytes([data[at + 1], data[at + 2]]),
                (1, 32) => i16::from_le_bytes([data[at + 2], data[at + 3]]),
                _ => (f32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]).clamp(-1.0, 1.0) * 32767.0).round() as i16,
            };
            channel.push(sample);
        }
    }
    if frames == 0 {
        return Err("This WAV file has no sound.".into());
    }
    Ok(Music { rate, channels: out, loop_start: loop_start.filter(|&s| (s as usize) < frames) })
}

/// Reads an MP3 file (one or two channels).
pub fn read_mp3(bytes: &[u8]) -> Result<Music, String> {
    let (header, frames) = puremp3::read_mp3(bytes).map_err(|e| format!("This MP3 file cannot be read: {e}"))?;
    let stereo = !matches!(header.channels, puremp3::Channels::Mono);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let to_i16 = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
    for (l, r) in frames {
        left.push(to_i16(l));
        if stereo {
            right.push(to_i16(r));
        }
    }
    if left.is_empty() {
        return Err("This MP3 file has no sound.".into());
    }
    let channels = if stereo { vec![left, right] } else { vec![left] };
    Ok(Music { rate: header.sample_rate.hz(), channels, loop_start: None })
}

/// Reads a sound file by what it starts with: WAV, else MP3.
pub fn read(bytes: &[u8]) -> Result<Music, String> {
    if bytes.starts_with(b"RIFF") {
        read_wav(bytes)
    } else {
        read_mp3(bytes)
    }
}

/// The sound at another rate (a straight line between two samples).
pub fn resample(music: &Music, rate: u32) -> Music {
    if music.rate == rate || music.frames() == 0 {
        return Music { rate, ..music.clone() };
    }
    let count = (music.frames() as u64 * rate as u64 / music.rate as u64) as usize;
    let step = music.rate as f64 / rate as f64;
    let last = music.frames() - 1;
    let channels = music
        .channels
        .iter()
        .map(|samples| {
            (0..count)
                .map(|i| {
                    let at = i as f64 * step;
                    let (a, fraction) = (at as usize, at - at.floor());
                    let (x, y) = (samples[a.min(last)] as f64, samples[(a + 1).min(last)] as f64);
                    (x + (y - x) * fraction).round() as i16
                })
                .collect()
        })
        .collect();
    Music { rate, channels, loop_start: music.loop_start.map(|s| (s as u64 * rate as u64 / music.rate as u64) as u32) }
}

#[cfg(test)]
mod test {
    use super::*;

    fn tone() -> Music {
        Music { rate: 32728, channels: vec![(0..1000).map(|i| (i * 13 % 2000 - 1000) as i16).collect(), (0..1000).map(|i| (i * 7 % 900 - 450) as i16).collect()], loop_start: Some(280) }
    }

    #[test]
    fn a_stereo_wav_file_keeps_its_samples_and_its_loop() {
        let music = tone();
        assert_eq!(read_wav(&write_wav(&music)).unwrap(), music);
        let no_loop = Music { loop_start: None, ..tone() };
        assert_eq!(read_wav(&write_wav(&no_loop)).unwrap(), no_loop);
    }

    #[test]
    fn a_bad_file_is_refused() {
        assert!(read_wav(b"not a wav").is_err());
        assert!(read_mp3(b"not an mp3 file at all").is_err());
        let mut wav = write_wav(&tone());
        wav[20] = 99; // an unknown format
        assert!(read_wav(&wav).is_err());
    }

    #[test]
    fn the_rate_changes_the_length_and_the_loop() {
        let music = Music { rate: 16000, channels: vec![vec![0; 1600]], loop_start: Some(800) };
        let up = resample(&music, 32000);
        assert_eq!(up.frames(), 3200);
        assert_eq!(up.loop_start, Some(1600));
    }
}
