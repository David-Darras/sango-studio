//! The cries of the Pokémon.
//!
//! The cries are in the archive 100. A cry is two files: a sequence (`CSEQ`, which says what to play) and a wave archive (`CWAR`, which
//! holds the sound). Every wave archive of the game holds one mono wave of 4-bit samples ("DSP-ADPCM"), without loop.
//!
//! - [`table`] says which files a species and a form use for each kind of cry,
//! - [`adpcm`] turns the 4-bit samples into 16-bit samples and back,
//! - [`archive`] reads and writes the wave archive,
//! - [`sound`] reads and writes WAV files and reads MP3 files.

pub mod adpcm;
pub mod archive;
pub mod sound;
pub mod table;

/// The number of the archive of the cries.
pub const ARCHIVE: usize = 100;

/// The biggest wave archive that the game loads, in bytes: it keeps a cry in a memory of this size, and a bigger file stays silent.
pub const MAX_ARCHIVE_BYTES: usize = 40_000;

/// The longest cry whose wave archive fits in [`MAX_ARCHIVE_BYTES`] at the rate of the game (32728 Hz), in seconds.
pub const MAX_SECONDS: f32 = 2.1;

/// The file of the wave archive of the first species of the packs. The archive of the cries of the game has fewer files: the plugin
/// reads the file of a new species from the SD card when it is there.
pub const FIRST_NEW_SPECIES_FILE: u32 = 5000;

/// A sound: mono samples of 16 bits at a rate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sound {
    /// The samples each second.
    pub rate: u32,
    pub samples: Vec<i16>,
}

impl Sound {
    /// The length in seconds.
    pub fn seconds(&self) -> f32 {
        self.samples.len() as f32 / self.rate.max(1) as f32
    }
}
