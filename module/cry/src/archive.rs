//! The wave archive of a cry (`CWAR`) with the wave (`CWAV`) that it holds.
//!
//! The files of the game have all the same layout. The archive has a header of 0x40 bytes, a block `INFO` of 0x20 bytes (the number of
//! waves, the place and the size of each) and a block `FILE` that starts with 0x20 bytes of header and padding, then the wave. The wave
//! has a header of 0x40 bytes, a block `INFO` of 0x80 bytes (the format, the rate, the number of samples, the coefficients of the
//! predictors) and a block `DATA` that starts with 0x20 bytes of header and padding, then the frames.

use crate::adpcm::{self, Coefficients, FRAME_BYTES, FRAME_SAMPLES};
use crate::Sound;

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, String> {
    bytes.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| "The file is cut.".to_string())
}

fn u32_at(bytes: &[u8], at: usize) -> Result<usize, String> {
    bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).ok_or_else(|| "The file is cut.".to_string())
}

/// The place of a block of a file by its number in the header: the start and the size.
fn block(bytes: &[u8], number: usize) -> Result<(usize, usize), String> {
    Ok((u32_at(bytes, 0x18 + 12 * number)?, u32_at(bytes, 0x1C + 12 * number)?))
}

/// Reads the wave of a wave archive (the first one, when the archive has several).
pub fn read_archive(bytes: &[u8]) -> Result<Sound, String> {
    if bytes.get(..4) != Some(b"CWAR") {
        return Err("This is not a wave archive.".into());
    }
    let (info, _) = block(bytes, 0)?;
    let (files, _) = block(bytes, 1)?;
    if u32_at(bytes, info + 8)? == 0 {
        return Err("The wave archive has no wave.".into());
    }
    // The place of the wave is counted from after the first 8 bytes of the block of the files.
    let start = files + 8 + u32_at(bytes, info + 16)?;
    let size = u32_at(bytes, info + 20)?;
    read_wave(bytes.get(start..start + size).ok_or("The wave is out of the archive.")?)
}

/// Reads a wave of 4-bit samples (or of 16 bits) with one channel.
pub fn read_wave(wave: &[u8]) -> Result<Sound, String> {
    if wave.get(..4) != Some(b"CWAV") {
        return Err("This is not a wave.".into());
    }
    let (info, _) = block(wave, 0)?;
    let (data, _) = block(wave, 1)?;
    let encoding = *wave.get(info + 8).ok_or("The wave is cut.")?;
    let rate = u32_at(wave, info + 12)? as u32;
    let count = u32_at(wave, info + 20)?;
    let channels = u32_at(wave, info + 28)?;
    if channels != 1 {
        return Err(format!("The wave has {channels} channels: this program reads the waves of one channel."));
    }
    // The channel: the place of the samples (from after the 8 first bytes of the block of the data), and the predictors.
    let channel = info + 28 + u32_at(wave, info + 36)?;
    let samples_at = data + 8 + u32_at(wave, channel + 4)?;
    match encoding {
        2 => {
            let adpcm = channel + u32_at(wave, channel + 12)?;
            let mut coefficients: Coefficients = [0; 16];
            for (k, c) in coefficients.iter_mut().enumerate() {
                *c = u16_at(wave, adpcm + 2 * k)? as i16;
            }
            let bytes = count.div_ceil(FRAME_SAMPLES) * FRAME_BYTES;
            let frames = wave.get(samples_at..samples_at + bytes).ok_or("The samples are out of the wave.")?;
            Ok(Sound { rate, samples: adpcm::decode(frames, &coefficients, count) })
        }
        1 => {
            let frames = wave.get(samples_at..samples_at + 2 * count).ok_or("The samples are out of the wave.")?;
            Ok(Sound { rate, samples: frames.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect() })
        }
        other => Err(format!("The wave has the format {other}: this program reads the formats 1 (16 bits) and 2 (4 bits).")),
    }
}

fn put16(out: &mut [u8], at: usize, value: u16) {
    out[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(out: &mut [u8], at: usize, value: usize) {
    out[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
}

/// Makes a wave of 4-bit samples from a sound.
pub fn build_wave(sound: &Sound) -> Vec<u8> {
    let encoded = adpcm::encode(&sound.samples);
    let data_size = 0x20 + encoded.bytes.len();
    let total = 0x40 + 0x80 + data_size;
    let mut out = vec![0u8; total];
    out[..4].copy_from_slice(b"CWAV");
    put16(&mut out, 4, 0xFEFF);
    put16(&mut out, 6, 0x40);
    put32(&mut out, 8, 0x0201_0000);
    put32(&mut out, 12, total);
    put16(&mut out, 16, 2);
    // The blocks: INFO then DATA.
    put16(&mut out, 0x14, 0x7000);
    put32(&mut out, 0x18, 0x40);
    put32(&mut out, 0x1C, 0x80);
    put16(&mut out, 0x20, 0x7001);
    put32(&mut out, 0x24, 0xC0);
    put32(&mut out, 0x28, data_size);
    // INFO.
    let info = 0x40;
    out[info..info + 4].copy_from_slice(b"INFO");
    put32(&mut out, info + 4, 0x80);
    out[info + 8] = 2;
    put32(&mut out, info + 12, sound.rate as usize);
    put32(&mut out, info + 20, sound.samples.len());
    put32(&mut out, info + 28, 1);
    put16(&mut out, info + 32, 0x7100);
    put32(&mut out, info + 36, 0x0C);
    // The channel (0x0C after the number of channels): the samples at 0x18 from the data, the predictors 0x14 further.
    let channel = info + 28 + 0x0C;
    put16(&mut out, channel, 0x1F00);
    put32(&mut out, channel + 4, 0x18);
    put16(&mut out, channel + 8, 0x0300);
    put32(&mut out, channel + 12, 0x14);
    let predictors = channel + 0x14;
    for (k, c) in encoded.coefficients.iter().enumerate() {
        put16(&mut out, predictors + 2 * k, *c as u16);
    }
    // The first frame header, and the history of the start (zeros).
    put16(&mut out, predictors + 32, encoded.bytes.first().copied().unwrap_or(0) as u16);
    // DATA.
    let data = 0xC0;
    out[data..data + 4].copy_from_slice(b"DATA");
    put32(&mut out, data + 4, data_size);
    out[data + 0x20..].copy_from_slice(&encoded.bytes);
    out
}

/// Makes a wave archive of one wave from a sound.
pub fn build_archive(sound: &Sound) -> Vec<u8> {
    let wave = build_wave(sound);
    let files_size = 0x20 + wave.len();
    let total = 0x60 + files_size;
    let mut out = vec![0u8; total];
    out[..4].copy_from_slice(b"CWAR");
    put16(&mut out, 4, 0xFEFF);
    put16(&mut out, 6, 0x40);
    put32(&mut out, 8, 0x0100_0000);
    put32(&mut out, 12, total);
    put16(&mut out, 16, 2);
    put16(&mut out, 0x14, 0x6800);
    put32(&mut out, 0x18, 0x40);
    put32(&mut out, 0x1C, 0x20);
    put16(&mut out, 0x20, 0x6801);
    put32(&mut out, 0x24, 0x60);
    put32(&mut out, 0x28, files_size);
    // INFO: one wave at 0x18 from the data of the block of the files, and its size.
    out[0x40..0x44].copy_from_slice(b"INFO");
    put32(&mut out, 0x44, 0x20);
    put32(&mut out, 0x48, 1);
    put16(&mut out, 0x4C, 0x1F00);
    put32(&mut out, 0x50, 0x18);
    put32(&mut out, 0x54, wave.len());
    // FILE.
    out[0x60..0x64].copy_from_slice(b"FILE");
    put32(&mut out, 0x64, files_size);
    out[0x80..].copy_from_slice(&wave);
    out
}

#[cfg(test)]
mod test {
    use super::*;

    fn tone() -> Sound {
        Sound { rate: 32728, samples: (0..20_000).map(|i| ((i as f64 / 32728.0 * 600.0 * std::f64::consts::TAU).sin() * 9000.0) as i16).collect() }
    }

    #[test]
    fn a_sound_goes_through_a_wave_archive_and_comes_back() {
        let sound = tone();
        let archive = build_archive(&sound);
        let back = read_archive(&archive).unwrap();
        assert_eq!((back.rate, back.samples.len()), (sound.rate, sound.samples.len()));
        // The 4-bit samples are not exact, but close.
        let error = sound.samples.iter().zip(&back.samples).map(|(&a, &b)| (a as i32 - b as i32).abs()).max().unwrap();
        assert!(error < 1500, "the largest error is {error}");
        assert!(read_archive(b"not an archive").is_err());
    }

    /// With the dump of the game (`SANGO_DUMP`): the archives of the cries are read, and made again with the same layout (the same size).
    #[test]
    fn the_archives_of_the_game_are_read_and_made_again_with_the_same_layout() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let Ok(dump) = sango_core::Dump::open(&root) else { return };
        let garc = sango_core::Garc::open(&dump.archive(crate::ARCHIVE)).unwrap();
        let mut checked = 0;
        for id in [3776usize, 3777, 3779, 3800, 4000, 4593] {
            let Some(file) = garc.first_file(id) else { continue };
            if file.get(..4) != Some(b"CWAR") {
                continue;
            }
            let sound = read_archive(file).unwrap();
            assert!(sound.rate > 8000 && sound.seconds() > 0.1 && sound.seconds() < 5.0, "file {id}: {} s at {} Hz", sound.seconds(), sound.rate);
            // A real sound is smooth: each sample is near the one before it (white noise would not be).
            let (mut near, mut all) = (0.0f64, 0.0f64);
            for pair in sound.samples.windows(2) {
                near += pair[0] as f64 * pair[1] as f64;
                all += pair[0] as f64 * pair[0] as f64;
            }
            assert!(all > 1e6 && near / all > 0.4, "file {id}: the decoded samples look like noise ({})", near / all.max(1.0));
            let again = build_archive(&sound);
            assert_eq!(again.len(), file.len(), "file {id}: the archive has the size of the original");
            assert_eq!(&again[..0x80], &file[..0x80], "file {id}: the headers are the original ones");
            // The header of the wave and the start of its INFO (the format, the rate, the samples, the places) are the original ones too.
            assert_eq!(&again[0x80..0xFC], &file[0x80..0xFC], "file {id}: the header of the wave is the original one");
            checked += 1;
        }
        assert!(checked >= 4);
    }
}
