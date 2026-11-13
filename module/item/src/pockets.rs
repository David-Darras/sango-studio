//! The battle pockets of the items: the part of the battle Bag where an item shows.
//!
//! The game reads them in the archive 198, not in the item files: one byte for each item, with one bit for each pocket
//! of the battle Bag. The 4 bits, from the data of the game: bit 0 the Balls, bit 1 the battle items (X Attack...), bit 2
//! the HP and PP recovery items, bit 3 the status recovery items. An item can be in two pockets (Full Restore is in both
//! recovery pockets). The table has 776 bytes, the item 0 first.
//!
//! The changed table goes on the SD card as one file of 776 bytes.

use sango_core::{Garc, SdCard};

/// The archive of the battle pockets.
pub const ARCHIVE: usize = 198;

/// Where the table is on the SD card.
pub const SD_PATH: &str = "item/battle_pocket.bin";

/// The size of the table: one byte for each item and the item 0.
pub const SIZE: usize = crate::data::COUNT + 1;

/// The pockets: the name, and what the game shows in it.
pub const BITS: [(&str, &str); 4] = [
    ("Balls", "The pocket of the Poké Balls in the battle Bag."),
    ("Battle items", "The pocket of the items that help in battle (X Attack, Guard Spec., Dire Hit...)."),
    ("HP/PP recovery", "The pocket of the items that restore HP or PP."),
    ("Status recovery", "The pocket of the items that cure a status condition."),
];

/// The table as the romfs has it.
pub fn romfs(garc: Option<&Garc>) -> Vec<u8> {
    let mut table = garc.and_then(|g| g.first_file(0)).map(|b| b.to_vec()).unwrap_or_default();
    table.resize(SIZE, 0);
    table
}

/// The table as the user has it: the SD card first, then the romfs.
pub fn current(garc: Option<&Garc>, sd: Option<&SdCard>) -> Vec<u8> {
    match sd.and_then(|sd| sd.read(SD_PATH)).filter(|bytes| bytes.len() == SIZE) {
        Some(table) => table,
        None => romfs(garc),
    }
}

/// Writes the table on the SD card and tells the game.
pub fn write(sd: &SdCard, table: &[u8]) -> Result<(), String> {
    sd.write(SD_PATH, table).and_then(|_| sd.mark_changed(SD_PATH))
}

/// Gives an item the pockets of the romfs again in the table of the SD card (when it is there). The file goes away when
/// the table is the table of the romfs again.
pub fn reset_item(sd: &SdCard, garc: Option<&Garc>, item: usize) -> Result<(), String> {
    let Some(mut table) = sd.read(SD_PATH).filter(|bytes| bytes.len() == SIZE) else { return Ok(()) };
    let rom = romfs(garc);
    if let (Some(byte), Some(original)) = (table.get_mut(item), rom.get(item)) {
        *byte = *original;
    }
    if table == rom {
        sd.remove(SD_PATH)?;
        Ok(())
    } else {
        write(sd, &table)
    }
}
