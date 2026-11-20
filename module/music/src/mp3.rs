//! MP3 files: Studio writes them with the encoder LAME (the reader is in `wav.rs`).
//!
//! LAME is under the LGPL licence: the program links it. The encoder only writes the sound that the user exports.

use crate::bcstm::Music;
use mp3lame_encoder::{Bitrate, Builder, DualPcm, FlushNoGap, MonoPcm, Quality};

/// The rates that the MP3 layer writes: the sound is brought to the nearest one.
const RATES: [u32; 3] = [32000, 44100, 48000];

/// Writes a sound as an MP3 file (one or two channels). The rate becomes 32000, 44100 or 48000 Hz, whichever is the nearest.
pub fn write_mp3(music: &Music) -> Result<Vec<u8>, String> {
    if music.frames() == 0 {
        return Err("The sound is empty.".into());
    }
    let channels = music.channels.len().clamp(1, 2);
    let rate = *RATES.iter().min_by_key(|&&r| (r as i64 - music.rate as i64).abs()).unwrap_or(&44100);
    let music = crate::wav::resample(music, rate);
    let failed = |what: &str| format!("The MP3 encoder failed ({what}).");
    let mut builder = Builder::new().ok_or_else(|| failed("start"))?;
    builder.set_num_channels(channels as u8).map_err(|_| failed("channels"))?;
    builder.set_sample_rate(rate).map_err(|_| failed("rate"))?;
    builder.set_brate(if channels == 2 { Bitrate::Kbps192 } else { Bitrate::Kbps128 }).map_err(|_| failed("bitrate"))?;
    builder.set_quality(Quality::Best).map_err(|_| failed("quality"))?;
    let mut encoder = builder.build().map_err(|_| failed("settings"))?;
    let mut out: Vec<u8> = Vec::new();
    out.reserve(mp3lame_encoder::max_required_buffer_size(music.frames()));
    let written = if channels == 2 {
        encoder.encode(DualPcm { left: &music.channels[0], right: &music.channels[1] }, out.spare_capacity_mut())
    } else {
        encoder.encode(MonoPcm(&music.channels[0]), out.spare_capacity_mut())
    }
    .map_err(|_| failed("sound"))?;
    // The encoder wrote `written` bytes in the spare room of the buffer.
    unsafe { out.set_len(out.len() + written) };
    out.reserve(7200);
    let flushed = encoder.flush::<FlushNoGap>(out.spare_capacity_mut()).map_err(|_| failed("end"))?;
    unsafe { out.set_len(out.len() + flushed) };
    Ok(out)
}

#[cfg(test)]
mod test {
    use super::*;

    fn tone(seconds: f32, channels: usize) -> Music {
        let frames = (32728.0 * seconds) as usize;
        let channel = |frequency: f32| (0..frames).map(|i| ((i as f32 / 32728.0 * frequency * std::f32::consts::TAU).sin() * 8000.0) as i16).collect::<Vec<i16>>();
        Music { rate: 32728, channels: (0..channels).map(|c| channel(440.0 * (c + 1) as f32)).collect(), loop_start: None }
    }

    #[test]
    fn an_mp3_file_comes_back_with_the_same_length_and_energy() {
        for channels in [1usize, 2] {
            let music = tone(1.5, channels);
            let mp3 = write_mp3(&music).unwrap();
            let back = crate::wav::read_mp3(&mp3).unwrap();
            assert_eq!(back.channels.len(), channels);
            assert!((back.seconds() - 1.5).abs() < 0.15, "{} s", back.seconds());
            let energy = |samples: &[i16]| samples.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / samples.len() as f64;
            let ratio = energy(&back.channels[0]) / energy(&music.channels[0]);
            assert!(ratio > 0.5 && ratio < 1.5, "energy ratio {ratio}");
        }
        assert!(write_mp3(&Music { rate: 32728, channels: vec![vec![]], loop_start: None }).is_err());
    }
}
