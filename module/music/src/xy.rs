//! The musics of Pokémon X and Y for Omega Ruby and Alpha Sapphire: a pack that replaces musics of the game.
//!
//! X and Y keep their musics as AAC files (`sound/bgm_xy_*.aac`), and their loops in the sound archive (`sound/xy_sound.bcsar`, see
//! [`crate::bcsar`]). The pack builder decodes each music, cuts it at the end of its loop, and encodes it as a stream of the game
//! with the stream that it replaces as model (see [`crate::bcstm::encode`]).
//!
//! A music of X and Y replaces the music of Omega Ruby and Alpha Sapphire that has the same name after `bgm_xy_` / `bgm_sg_` (the
//! musics of the two games have the same names for the same places: `c_01` the first town, `bicycle` the bicycle...). The
//! musics that Omega Ruby and Alpha Sapphire already take from X and Y (`bgm_xy_*.dspadpcm.bcstm`) are left out.

use crate::bcsar::{Archive, Kind, Place};
use crate::bcstm::{self, Music};
use std::collections::BTreeMap;
use std::path::Path;

/// The rate of the streams of the game.
pub const RATE: u32 = 32728;

/// The AAC frames without their check sums. The frames of X and Y have a header of 9 bytes: the 7 bytes of the header, then a check
/// sum of 2 bytes (bit 0 of byte 1 is 0). The decoder reads only the headers of 7 bytes: the check sum is removed, the bit is set, and
/// the length of the frame (13 bits from bit 1 of byte 3) loses 2.
fn without_check_sums(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at + 7 <= bytes.len() {
        let header = &bytes[at..at + 7];
        if header[0] != 0xFF || header[1] & 0xF0 != 0xF0 {
            return Err(format!("The AAC file has no frame at {at:#x}."));
        }
        let length = ((header[3] as usize & 3) << 11) | ((header[4] as usize) << 3) | (header[5] as usize >> 5);
        if length < 7 || at + length > bytes.len() {
            break;
        }
        if header[1] & 1 == 1 {
            out.extend_from_slice(&bytes[at..at + length]);
        } else {
            let new_length = length - 2;
            let mut fixed = header.to_vec();
            fixed[1] |= 1;
            fixed[3] = (fixed[3] & !3) | ((new_length >> 11) & 3) as u8;
            fixed[4] = ((new_length >> 3) & 0xFF) as u8;
            fixed[5] = (fixed[5] & 0x1F) | (((new_length & 7) << 5) as u8);
            out.extend_from_slice(&fixed);
            out.extend_from_slice(&bytes[at + 9..at + length]);
        }
        at += length;
    }
    Ok(out)
}

/// Decodes an AAC file (ADTS frames) to a sound without loop.
pub fn decode_aac(bytes: &[u8]) -> Result<Music, String> {
    let bytes = without_check_sums(bytes)?;
    let bytes = bytes.as_slice();
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let source = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("aac");
    let probed = symphonia::default::get_probe().format(&hint, source, &FormatOptions::default(), &MetadataOptions::default()).map_err(|e| format!("This is not an AAC file: {e}"))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("The AAC file has no track.")?.clone();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(|e| e.to_string())?;
    let mut rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut channels: Vec<Vec<i16>> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(_)) => break,
            Err(e) => return Err(e.to_string()),
        };
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A damaged frame is skipped, like a player does.
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        };
        let spec = *decoded.spec();
        rate = spec.rate;
        let count = spec.channels.count();
        let mut buffer = SampleBuffer::<i16>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        if channels.len() != count {
            channels = vec![Vec::new(); count];
        }
        for frame in buffer.samples().chunks(count) {
            for (channel, sample) in frame.iter().enumerate() {
                channels[channel].push(*sample);
            }
        }
    }
    if channels.is_empty() || channels[0].is_empty() {
        return Err("The AAC file has no sound.".into());
    }
    Ok(Music { rate, channels, loop_start: None })
}

/// A music of X and Y and the music of the game that it replaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    /// The AAC file of X and Y (`bgm_xy_bicycle.aac`).
    pub source: String,
    /// The name of the sound in the archive of X and Y.
    pub source_name: String,
    /// The loop of the music of X and Y, in frames of its file.
    pub loop_frames: Option<(u32, u32)>,
    /// The stream file of the game that it replaces (`bgm_sg_bicycle.dspadpcm.bcstm`).
    pub target: String,
    /// The names of the sounds of the game that play the replaced file.
    pub target_names: Vec<String>,
}

/// The musics of X and Y that have a music of the same place in Omega Ruby and Alpha Sapphire. `oras` and `xy` are the folders of the
/// files of the two games (with the folder `sound`).
pub fn pairings(oras: &Path, xy: &Path) -> Result<Vec<Pairing>, String> {
    let read = |path: &Path| std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()));
    let oras_archive = Archive::parse(&read(&oras.join("sound/sango_sound.bcsar"))?)?;
    let xy_archive = Archive::parse(&read(&xy.join("sound/xy_sound.bcsar"))?)?;
    // The names of the sounds of the game, by file.
    let mut oras_names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for sound in oras_archive.sounds.iter().filter(|s| s.kind == Kind::Stream) {
        if let Place::External(file) = &sound.place {
            oras_names.entry(file.clone()).or_default().push(sound.name.clone());
        }
    }
    let mut out: Vec<Pairing> = Vec::new();
    for sound in xy_archive.sounds.iter().filter(|s| s.kind == Kind::Stream) {
        let Place::External(source) = &sound.place else { continue };
        if out.iter().any(|p| &p.source == source) {
            continue;
        }
        let Some(stem) = source.strip_suffix(".aac") else { continue };
        let Some((prefix, place)) = ["bgm_xy_", "me_xy_"].iter().find_map(|prefix| stem.strip_prefix(prefix).map(|place| (*prefix, place))) else { continue };
        // Omega Ruby and Alpha Sapphire already have this music of X and Y.
        if oras_names.contains_key(&format!("{prefix}{place}.dspadpcm.bcstm")) {
            continue;
        }
        let target = format!("{}{place}.dspadpcm.bcstm", prefix.replace("_xy_", "_sg_"));
        let Some(names) = oras_names.get(&target) else { continue };
        if !oras.join("sound").join(&target).is_file() || !xy.join("sound").join(source).is_file() {
            continue;
        }
        out.push(Pairing { source: source.clone(), source_name: sound.name.clone(), loop_frames: sound.loop_frames, target, target_names: names.clone() });
    }
    Ok(out)
}

/// Makes the stream file of a pairing: the music of X and Y, cut at the end of its loop, in the layout of the stream that it replaces.
pub fn convert(oras: &Path, xy: &Path, pairing: &Pairing) -> Result<Vec<u8>, String> {
    let read = |path: &Path| std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()));
    let template = read(&oras.join("sound").join(&pairing.target))?;
    let mut music = decode_aac(&read(&xy.join("sound").join(&pairing.source))?)?;
    if let Some((start, end)) = pairing.loop_frames {
        let end = (end as usize).min(music.frames());
        if (start as usize) < end {
            for channel in &mut music.channels {
                channel.truncate(end);
            }
            music.loop_start = Some(start);
        }
    }
    let source_rate = music.rate.max(1);
    let shaped = crate::wav::resample(&bcstm::with_channels(&music, bcstm::channel_count(&template)?), RATE);
    let loop_start = music.loop_start.map(|s| (s as u64 * RATE as u64 / source_rate as u64) as u32);
    bcstm::encode(&template, &Music { loop_start, ..shaped })
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dumps of the two games (SANGO_DUMP for Omega Ruby and Alpha Sapphire, SANGO_XY_DUMP for X or Y): the musics of the
    /// same places are paired, and one is converted to a stream that the decoder of the game reads back with its loop.
    #[test]
    fn a_music_of_x_and_y_becomes_a_stream_of_the_game() {
        let (Ok(oras), Ok(xy)) = (std::env::var("SANGO_DUMP"), std::env::var("SANGO_XY_DUMP")) else { return };
        let (oras, xy) = (Path::new(&oras), Path::new(&xy));
        let pairs = pairings(oras, xy).unwrap();
        println!("{} pairings", pairs.len());
        assert!(pairs.len() > 40);
        let bicycle = pairs.iter().find(|p| p.source == "bgm_xy_bicycle.aac").expect("the bicycle music is paired");
        assert_eq!(bicycle.target, "bgm_sg_bicycle.dspadpcm.bcstm");
        let bytes = convert(oras, xy, bicycle).unwrap();
        let back = bcstm::decode(&bytes).unwrap();
        assert_eq!(back.rate, RATE);
        assert!(back.seconds() > 20.0);
        assert_eq!(back.loop_start.is_some(), bicycle.loop_frames.is_some());
    }
}
