//! The Music page: the musics, the jingles and the sounds of the game.
//!
//! The musics and the jingles are files of the folder `sound` of the game (`.bcstm`, 4 bits for each sample, with a loop). The
//! sound archive `sound/sango_sound.bcsar` gives the name of each one.

pub mod bcsar;
pub mod bcstm;
pub mod effects;
pub mod ids;
pub mod names;
pub mod mp3;
pub mod page;
pub mod wav;
pub mod waves;

pub use page::MusicPage;

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game (`SANGO_DUMP`): the sounds of the archive have their names, and each stream has a file of the folder.
    #[test]
    fn the_archive_lists_the_streams_with_their_files() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::path::Path::new(&root).join("sound");
        let archive = bcsar::Archive::parse(&std::fs::read(folder.join("sango_sound.bcsar")).unwrap()).unwrap();
        assert_eq!(archive.sounds.len(), 1709);
        let streams: Vec<_> = archive.sounds.iter().filter(|s| s.kind == bcsar::Kind::Stream).collect();
        assert_eq!(streams.len(), 256);
        assert!(matches!(&streams[1].place, bcsar::Place::External(file) if file.ends_with(".bcstm")));
        for stream in &streams {
            let bcsar::Place::External(file) = &stream.place else { panic!("{} has no file", stream.name) };
            assert!(folder.join(file).exists(), "{}: {file}", stream.name);
        }
        // Every music and jingle of the list of the ids is a stream of the archive.
        // Every stream of the list of the ids is a stream of the archive, and every sound effect is a sequence.
        assert_eq!(ids::STREAMS.len(), 256);
        for (index, _) in ids::STREAMS {
            assert_eq!(archive.sounds[index as usize].kind, bcsar::Kind::Stream, "{index}");
        }
        assert_eq!(ids::EFFECTS.len(), 1436);
        for (index, _) in ids::EFFECTS {
            assert_eq!(archive.sounds[index as usize].kind, bcsar::Kind::Sequence, "{index}");
        }
        // The first sound effect has its place in the file.
        assert!(matches!(archive.sounds[ids::EFFECTS[0].0 as usize].place, bcsar::Place::Internal { .. }));
    }

    /// With the dump of the game: the waves of a sound effect are found, read, and replaced in place without changing the archive size.
    #[test]
    fn the_waves_of_a_sound_effect_are_replaced_in_place() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let mut bytes = std::fs::read(std::path::Path::new(&root).join("sound/sango_sound.bcsar")).unwrap();
        let archive = bcsar::Archive::parse(&bytes).unwrap();
        assert!(archive.banks.len() > 1000 && archive.wave_archives.len() > 1000);
        let sound = &archive.sounds[ids::EFFECTS[0].0 as usize];
        assert!(!sound.banks.is_empty(), "the sequence has a bank");
        let slots = waves::waves_of(&bytes, &archive, sound).unwrap();
        assert!(!slots.is_empty(), "the bank has waves");
        let old = waves::read(&bytes, &slots[0]).unwrap();
        assert!(old.rate >= 8000 && !old.samples.is_empty());
        // A tone replaces it; the archive keeps its size and the wave reads back.
        let tone = sango_cry::Sound { rate: 32000, samples: (0..4000).map(|i| ((i as f32 * 0.1).sin() * 9000.0) as i16).collect() };
        let length = bytes.len();
        let written = waves::replace(&mut bytes, &slots[0], &tone).unwrap();
        assert_eq!(bytes.len(), length);
        let back = waves::read(&bytes, &slots[0]).unwrap();
        assert_eq!(back.rate, 32000);
        assert_eq!(back.samples.len(), written);
        // A sound that is too long is cut to the room.
        let long = sango_cry::Sound { rate: 32000, samples: vec![1000; 10_000_000] };
        let written = waves::replace(&mut bytes, &slots[0], &long).unwrap();
        assert!(written <= waves::room_samples(slots[0].size));
        // The other sounds are not touched: the archive still parses.
        assert_eq!(bcsar::Archive::parse(&bytes).unwrap().sounds.len(), 1709);
    }

    /// With the dump of the game: a stream is read, written again with the layout of its own file, and read again: the sound is the
    /// same, the file has the same size (to a few bytes), and the loop is kept.
    #[test]
    fn a_stream_is_read_and_written_again() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        // The second stream of the archive (a music with a long loop).
        let archive = bcsar::Archive::parse(&std::fs::read(std::path::Path::new(&root).join("sound/sango_sound.bcsar")).unwrap()).unwrap();
        let file = archive.sounds.iter().filter(|s| s.kind == bcsar::Kind::Stream).nth(1).and_then(|s| match &s.place { bcsar::Place::External(file) => Some(file.clone()), _ => None }).unwrap();
        let original = std::fs::read(std::path::Path::new(&root).join("sound").join(file)).unwrap();
        let music = bcstm::decode(&original).unwrap();
        assert_eq!(music.channels.len(), bcstm::channel_count(&original).unwrap());
        assert!(music.rate > 8000 && music.seconds() > 10.0, "{} Hz, {} s", music.rate, music.seconds());
        // The seek block of the game has the two samples before each block: the decoder finds nearly the same ones (the tool of the
        // game does not round like the decoder, the difference stays small).
        let seek = u32::from_le_bytes(original[0x14 + 12 + 4..0x14 + 12 + 8].try_into().unwrap()) as usize + 8;
        let channels = music.channels.len();
        for block in 1..4 {
            for (c, channel) in music.channels.iter().enumerate() {
                let at = seek + (block * channels + c) * 4;
                let (y1, y2) = (i16::from_le_bytes([original[at], original[at + 1]]), i16::from_le_bytes([original[at + 2], original[at + 3]]));
                let start = block * 14336;
                assert!((channel[start - 1] as i32 - y1 as i32).abs() < 100 && (channel[start - 2] as i32 - y2 as i32).abs() < 100, "block {block} channel {c}");
            }
        }
        // A short piece keeps the test fast.
        let short = bcstm::Music { rate: music.rate, channels: music.channels.iter().map(|c| c[..60_000].to_vec()).collect(), loop_start: Some(14 * 100) };
        let file = bcstm::encode(&original, &short).unwrap();
        assert_eq!(file.len() % 0x20, 0);
        let again = bcstm::decode(&file).unwrap();
        assert_eq!(again.rate, short.rate);
        assert_eq!(again.loop_start, Some(14 * 100));
        assert_eq!(again.frames(), 60_000);
        // The 4-bit coding loses a little: the sound stays close to the original (more than 25 dB between the sound and the noise).
        for (a, b) in again.channels.iter().zip(&short.channels) {
            let signal: f64 = b.iter().map(|&x| (x as f64).powi(2)).sum();
            let noise: f64 = a.iter().zip(b).map(|(&x, &y)| ((x as f64) - (y as f64)).powi(2)).sum();
            assert!(10.0 * (signal / noise.max(1.0)).log10() > 25.0, "noise");
        }
        // A mono sound fits a stereo stream and the other way round.
        let mono = bcstm::with_channels(&short, 1);
        assert_eq!(mono.channels.len(), 1);
        assert_eq!(bcstm::with_channels(&mono, 2).channels.len(), 2);
    }
}




