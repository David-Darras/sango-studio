//! The waves of the sound effects: a sequence plays the waves of its banks, and each bank uses wave archives (`CWAR`) that are files of
//! the sound archive.
//!
//! A wave archive has a header of 0x40 bytes, a block `INFO` (the number of waves, then the place and the size of each) and a block
//! `FILE` that holds the waves (`CWAV`). The places are counted from after the 8 first bytes of the block `FILE`. The reader of one wave
//! is in the crate of the cries.
//!
//! A wave is replaced in place: the new wave goes in the room of the old one, so the sound archive keeps its size and every offset.

use crate::bcsar::{Archive, Place, Sound as ArchiveSound};
use sango_cry::archive::{build_wave, read_wave};
use sango_cry::Sound;

/// A wave inside the sound archive.
#[derive(Clone, Debug)]
pub struct WaveSlot {
    /// The place of the wave archive in the list of the archive, and the place of the wave in it.
    pub archive: usize,
    pub index: usize,
    /// The offset of the wave in the archive file, and the room that it has.
    pub offset: usize,
    pub size: usize,
}

fn u32_at(bytes: &[u8], at: usize) -> Result<usize, String> {
    sango_core::bytes::u32_at(bytes, at).map(|value| value as usize)
}

/// The waves of a wave archive that is at `start` in the file.
fn waves_in(bytes: &[u8], archive: usize, start: usize) -> Result<Vec<WaveSlot>, String> {
    if bytes.get(start..start + 4) != Some(b"CWAR") {
        return Err("A wave archive of the sound archive is not where it should be.".into());
    }
    let info = start + u32_at(bytes, start + 0x18)?;
    let files = start + u32_at(bytes, start + 0x24)?;
    let count = u32_at(bytes, info + 8)?;
    (0..count)
        .map(|index| {
            let entry = info + 12 + index * 12;
            let (offset, size) = (u32_at(bytes, entry + 4)?, u32_at(bytes, entry + 8)?);
            Ok(WaveSlot { archive, index, offset: files + 8 + offset, size })
        })
        .collect()
}

/// The waves that a bank plays, from the table of its file: the wave archive (an item id: the type in the high 8 bits, the place in
/// the others) and the place of the wave in it.
fn bank_waves(bytes: &[u8], start: usize) -> Result<Vec<(usize, usize)>, String> {
    if bytes.get(start..start + 4) != Some(b"CBNK") {
        return Err("A bank of the sound archive is not where it should be.".into());
    }
    let blocks = u16_at(bytes, start + 0x10)?;
    for i in 0..blocks {
        let entry = start + 0x14 + i * 12;
        if sango_core::bytes::u16_at(bytes, entry)? == 0x5800 {
            let body = start + u32_at(bytes, entry + 4)? + 8;
            let table = body + u32_at(bytes, body + 4)?;
            return (0..u32_at(bytes, table)?)
                .map(|k| Ok((u32_at(bytes, table + 4 + k * 8)? & 0x00FF_FFFF, u32_at(bytes, table + 8 + k * 8)?)))
                .collect();
        }
    }
    Err("A bank has no information block.".into())
}

fn u16_at(bytes: &[u8], at: usize) -> Result<usize, String> {
    sango_core::bytes::u16_at(bytes, at).map(|value| value as usize)
}

/// The waves that a sound plays: the waves that the banks of its sequence list (a wave that two banks share is listed one time).
pub fn waves_of(bytes: &[u8], archive: &Archive, sound: &ArchiveSound) -> Result<Vec<WaveSlot>, String> {
    let mut out: Vec<WaveSlot> = Vec::new();
    for &bank in &sound.banks {
        let Some(bank) = archive.banks.get(bank) else { continue };
        let Place::Internal { offset, .. } = bank.place else { continue };
        for (place, index) in bank_waves(bytes, archive.file_block as usize + 8 + offset as usize)? {
            let Some(wave_archive) = archive.wave_archives.get(place) else { continue };
            let Place::Internal { offset, .. } = wave_archive.place else { continue };
            let slots = waves_in(bytes, place, archive.file_block as usize + 8 + offset as usize)?;
            if let Some(slot) = slots.get(index) {
                if !out.iter().any(|o| o.offset == slot.offset) {
                    out.push(slot.clone());
                }
            }
        }
    }
    Ok(out)
}

/// Reads a wave of the archive (a wave of one channel).
pub fn read(bytes: &[u8], slot: &WaveSlot) -> Result<Sound, String> {
    read_wave(bytes.get(slot.offset..slot.offset + slot.size).ok_or("The wave is out of the archive.")?)
}

/// The most samples that a wave of 4 bits can have in a room of `size` bytes.
pub fn room_samples(size: usize) -> usize {
    // The header of the wave is 0x40 bytes, its information 0x80, the head of its data 0x20 (see `build_wave`).
    size.saturating_sub(0x40 + 0x80 + 0x20 + 0x20) / 8 * 14
}

/// Puts a sound in the room of a wave. A sound that is too long is cut. Returns the number of samples that were written.
pub fn replace(bytes: &mut [u8], slot: &WaveSlot, sound: &Sound) -> Result<usize, String> {
    let limit = room_samples(slot.size);
    if limit < 14 {
        return Err("This wave has no room for a new sound.".into());
    }
    let cut = Sound { rate: sound.rate, samples: sound.samples[..sound.samples.len().min(limit)].to_vec() };
    let wave = build_wave(&cut);
    if wave.len() > slot.size {
        return Err(format!("The new wave takes {} bytes and the room has {}.", wave.len(), slot.size));
    }
    let room = bytes.get_mut(slot.offset..slot.offset + slot.size).ok_or("The wave is out of the archive.")?;
    room[..wave.len()].copy_from_slice(&wave);
    room[wave.len()..].fill(0);
    Ok(cut.samples.len())
}
