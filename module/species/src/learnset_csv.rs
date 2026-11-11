//! The CSV export and import of the moves that the species learn.
//!
//! One file holds one row for each move of each species and form. It follows the layout of the species file
//! (see the species CSV module): a header, plain decimal numbers, commas between the fields, no quotes. The
//! import reads and checks the whole file, then writes the changes, or writes nothing.
//!
//! | Column | Content | Limits |
//! | --- | --- | --- |
//! | `species` | The number of the species. | 1 to 721 |
//! | `form` | The number of the form. 0 is the normal form. | the forms of the species |
//! | `name` | The name of the species. The import ignores it. | text without a comma |
//! | `group` | `level_up`, `egg`, `tm` or `tutor`. | one of the four |
//! | `level` | The level, for `level_up` only. Empty for the other groups. | 1 to 100 |
//! | `move` | The move number: 1 is Pound. 0 means "no move" (see below). | 0 to 621 |
//! | `move_name` | The name of the move. The import ignores it. | text without a comma |
//!
//! The groups:
//!
//! - `level_up`: the moves learned by leveling up, at most 26 for a species and form.
//! - `egg`: the egg moves. They belong to the species: only the form 0 has them.
//! - `tm`: the TMs and HMs that the species and form can learn. The move must be the one of a TM or an HM.
//! - `tutor`: the moves of the tutors (the special tutors and the four tutors that charge Battle Points).
//!
//! **What an import replaces.** For a species, a form and a group that the file mentions, the moves of the file
//! take the place of all the moves of that group. The groups that the file does not mention do not change.
//! To remove all the moves of a group, give one row of that group with the move 0.

use crate::data::SpeciesData;
use crate::learnset::{machine_is_set, move_name, set_machine, Slot};
use crate::machines::MACHINES;
use crate::move_names::MOVE;
use crate::{file_index, names, sd_path, SPECIES_COUNT};
use sango_core::learnset::{self, EggMoves, LevelUp, LevelUpMove, MAX_LEVEL_UP_MOVES};
use sango_core::{Garc, SdCard};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;

/// The header line of the file.
pub const HEADER: &str = "species,form,name,group,level,move,move_name";

/// The groups of moves, as the column `group` writes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    LevelUp,
    Egg,
    Machine,
    Tutor,
}

impl Group {
    fn name(self) -> &'static str {
        match self {
            Group::LevelUp => "level_up",
            Group::Egg => "egg",
            Group::Machine => "tm",
            Group::Tutor => "tutor",
        }
    }

    fn from_name(name: &str) -> Option<Group> {
        [Group::LevelUp, Group::Egg, Group::Machine, Group::Tutor].into_iter().find(|group| group.name() == name)
    }
}

/// One row of the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub species: usize,
    pub form: usize,
    pub group: Group,
    /// The level. 0 for the groups that have no level.
    pub level: u16,
    /// The move number, or 0 for "no move".
    pub move_id: u16,
}

/// What an import did: the number of files written, by kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Report {
    pub level_up_files: usize,
    pub egg_files: usize,
    pub data_files: usize,
}

impl Report {
    /// The number of files in all.
    pub fn total(&self) -> usize {
        self.level_up_files + self.egg_files + self.data_files
    }
}

/// The archives that the moves come from, and the SD card that has the changes.
pub struct Library<'a> {
    /// The species data (archive 195).
    pub species: &'a Garc,
    /// The level-up moves (archive 191).
    pub level_up: &'a Garc,
    /// The egg moves (archive 190).
    pub eggs: &'a Garc,
    pub sd: Option<&'a SdCard>,
}

impl Library<'_> {
    fn form_count(&self, species: usize) -> usize {
        self.species.first_file(species).and_then(|b| SpeciesData::from_bytes(b).ok()).map_or(1, |d| (d.total_form_count as usize).max(1))
    }

    /// The species data of a species and a form: the SD card first, then the romfs.
    fn data(&self, species: usize, form: usize) -> Option<SpeciesData> {
        let on_card = self.sd.and_then(|sd| sd.read(&sd_path(species, form))).and_then(|b| SpeciesData::from_bytes(&b).ok());
        on_card.or_else(|| self.species.first_file(file_index(self.species, species, form)).and_then(|b| SpeciesData::from_bytes(b).ok()))
    }

    fn level_up(&self, species: usize, form: usize) -> Option<LevelUp> {
        let on_card = self.sd.and_then(|sd| sd.read(&learnset::level_up_sd_path(species, form))).and_then(|b| LevelUp::from_bytes(&b).ok());
        on_card.or_else(|| self.level_up.first_file(file_index(self.species, species, form)).and_then(|b| LevelUp::from_bytes(b).ok()))
    }

    fn eggs(&self, species: usize) -> Option<EggMoves> {
        let on_card = self.sd.and_then(|sd| sd.read(&learnset::egg_sd_path(species))).and_then(|b| EggMoves::from_bytes(&b).ok());
        on_card.or_else(|| self.eggs.first_file(species).and_then(|b| EggMoves::from_bytes(b).ok()))
    }
}

/// Writes one row for each move, from the current data (the SD card first, then the romfs).
pub fn export(library: &Library) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    let line = |out: &mut String, species: usize, form: usize, group: Group, level: Option<u16>, id: u16| {
        let level = level.map_or(String::new(), |level| level.to_string());
        let _ = writeln!(out, "{species},{form},{},{},{level},{id},{}", names::SPECIES[species - 1], group.name(), move_name(id));
    };
    for species in 1..=SPECIES_COUNT {
        for form in 0..library.form_count(species) {
            if let Some(level_up) = library.level_up(species, form) {
                for m in &level_up.moves {
                    line(&mut out, species, form, Group::LevelUp, Some(m.level), m.id);
                }
            }
            if form == 0 {
                for id in library.eggs(species).map(|eggs| eggs.moves).unwrap_or_default() {
                    line(&mut out, species, form, Group::Egg, None, id);
                }
            }
            if let Some(data) = library.data(species, form) {
                for bit in (0..MACHINES.len()).filter(|&bit| machine_is_set(&data, bit)) {
                    line(&mut out, species, form, Group::Machine, None, MACHINES[bit]);
                }
                for (_, id) in Slot::all().into_iter().filter(|(slot, _)| slot.is_set(&data)) {
                    line(&mut out, species, form, Group::Tutor, None, id);
                }
            }
        }
    }
    out
}

/// Reads and checks the text of a file. Nothing is applied yet. The result holds every row, or the problems.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let mut lines = text.lines().enumerate();
    match lines.next() {
        Some((_, first)) if first.trim_end() == HEADER => {}
        _ => return Err("The first line is not the header of a learnset CSV file.".into()),
    }
    let columns = HEADER.split(',').count();
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for (place, line) in lines {
        let number = place + 1;
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        if fields.len() != columns {
            errors.push(format!("line {number}: {} fields, not {columns}", fields.len()));
            continue;
        }
        let mut field = |column: usize, limit: u32, what: &str| -> u32 {
            match fields[column].parse::<u32>() {
                Ok(value) if value <= limit => value,
                Ok(value) => {
                    errors.push(format!("line {number}: {what} is {value}, the limit is {limit}"));
                    0
                }
                Err(_) => {
                    errors.push(format!("line {number}: {what} is not a number"));
                    0
                }
            }
        };
        let species = field(0, SPECIES_COUNT as u32, "species") as usize;
        let form = field(1, 255, "form") as usize;
        let move_id = field(5, MOVE.len() as u32, "move") as u16;
        let Some(group) = Group::from_name(fields[3]) else {
            errors.push(format!("line {number}: the group `{}` is not level_up, egg, tm or tutor", fields[3]));
            continue;
        };
        let level = if group == Group::LevelUp && move_id != 0 {
            field(4, 100, "level") as u16
        } else if fields[4].is_empty() || fields[4] == "0" {
            0
        } else {
            errors.push(format!("line {number}: the level must be empty for the group {}", group.name()));
            0
        };
        if group == Group::LevelUp && move_id != 0 && level == 0 {
            errors.push(format!("line {number}: the level is 0, the first level is 1"));
        }
        if species == 0 {
            errors.push(format!("line {number}: the species is 0, the first species is 1"));
        }
        rows.push(Row { species, form, group, level, move_id });
    }
    if errors.is_empty() && rows.is_empty() {
        errors.push("The file has no rows.".into());
    }
    if !errors.is_empty() {
        let shown = errors.len().min(10);
        let mut message = errors[..shown].join("\n");
        if errors.len() > shown {
            let _ = write!(message, "\n... and {} more problems", errors.len() - shown);
        }
        return Err(message);
    }
    Ok(rows)
}

/// The new moves of one species and form, for the groups that the file mentions.
#[derive(Default)]
struct Changes {
    level_up: Option<Vec<LevelUpMove>>,
    machines: Option<Vec<usize>>,
    tutors: Option<Vec<Slot>>,
}

/// Checks the rows against the game, then writes each file that changes. Nothing is written when a row is wrong.
pub fn apply(rows: &[Row], library: &Library, sd: &SdCard) -> Result<Report, String> {
    let mut changes: BTreeMap<(usize, usize), Changes> = BTreeMap::new();
    let mut eggs: BTreeMap<usize, Vec<u16>> = BTreeMap::new();
    let mut errors = Vec::new();
    let slots = Slot::all();
    let mut seen = HashSet::new();
    for row in rows {
        let (species, form) = (row.species, row.form);
        if form >= library.form_count(species) {
            errors.push(format!("the species {species} has no form {form}"));
            continue;
        }
        let entry = changes.entry((species, form)).or_default();
        let id = row.move_id;
        match row.group {
            Group::LevelUp => {
                let moves = entry.level_up.get_or_insert_with(Vec::new);
                if id != 0 {
                    moves.push(LevelUpMove { id, level: row.level });
                }
            }
            Group::Egg => {
                if form != 0 {
                    errors.push(format!("the egg moves of the species {species} are in the form 0, not in the form {form}"));
                    continue;
                }
                let moves = eggs.entry(species).or_default();
                // The game itself lists an egg move two times for a few species: this is not refused.
                if id != 0 {
                    moves.push(id);
                }
            }
            Group::Machine => {
                let bits = entry.machines.get_or_insert_with(Vec::new);
                if id != 0 {
                    match MACHINES.iter().position(|&m| m == id) {
                        Some(bit) if seen.insert((species, form, row.group, id)) => bits.push(bit),
                        Some(_) => errors.push(format!("the species {species} form {form} has the TM or HM {} two times", move_name(id))),
                        None => errors.push(format!("{} is not the move of a TM or an HM", move_name(id))),
                    }
                }
            }
            Group::Tutor => {
                let list = entry.tutors.get_or_insert_with(Vec::new);
                if id != 0 {
                    match slots.iter().find(|(_, tutor_move)| *tutor_move == id) {
                        Some((slot, _)) if seen.insert((species, form, row.group, id)) => list.push(*slot),
                        Some(_) => errors.push(format!("the species {species} form {form} has the tutor move {} two times", move_name(id))),
                        None => errors.push(format!("{} is not the move of a tutor", move_name(id))),
                    }
                }
            }
        }
    }
    for (&(species, form), change) in &changes {
        if let Some(moves) = &change.level_up {
            if moves.len() > MAX_LEVEL_UP_MOVES {
                errors.push(format!("the species {species} form {form} has {} level-up moves, the most is {MAX_LEVEL_UP_MOVES}", moves.len()));
            }
        }
    }
    if !errors.is_empty() {
        let shown = errors.len().min(10);
        let mut message = errors[..shown].join("\n");
        if errors.len() > shown {
            let _ = write!(message, "\n... and {} more problems", errors.len() - shown);
        }
        return Err(message);
    }

    // Every row is good: the files that change are written.
    let mut report = Report::default();
    let write = |path: String, bytes: Vec<u8>| -> Result<(), String> { sd.write(&path, &bytes).and_then(|_| sd.mark_changed(&path)) };
    for (&(species, form), change) in &changes {
        if let Some(moves) = &change.level_up {
            let mut new = LevelUp { moves: moves.clone() };
            new.sort();
            if library.level_up(species, form) != Some(new.clone()) {
                write(learnset::level_up_sd_path(species, form), new.to_bytes())?;
                report.level_up_files += 1;
            }
        }
        if change.machines.is_some() || change.tutors.is_some() {
            let Some(old) = library.data(species, form) else { continue };
            let mut data = old;
            // Only the bits that the tables name are replaced: another bit that the data may have stays as it is.
            if let Some(bits) = &change.machines {
                for bit in 0..MACHINES.len() {
                    set_machine(&mut data, bit, false);
                }
                for &bit in bits {
                    set_machine(&mut data, bit, true);
                }
            }
            if let Some(list) = &change.tutors {
                for (slot, _) in &slots {
                    slot.set(&mut data, false);
                }
                for slot in list {
                    slot.set(&mut data, true);
                }
            }
            if data != old {
                write(sd_path(species, form), data.to_bytes())?;
                report.data_files += 1;
            }
        }
    }
    for (&species, moves) in &eggs {
        let mut new = library.eggs(species).unwrap_or_default();
        let same = new.moves == *moves;
        if !same {
            new.moves.clear();
            for &id in moves {
                new.push(id);
            }
            write(learnset::egg_sd_path(species), new.to_bytes())?;
            report.egg_files += 1;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_good_file_is_read() {
        let text = "species,form,name,group,level,move,move_name\n1,0,Bulbasaur,level_up,1,33,Tackle\n1,0,Bulbasaur,egg,,133,Amnesia\n1,0,Bulbasaur,tm,,92,Toxic\n1,0,Bulbasaur,tutor,,402,Seed Bomb\n2,0,Ivysaur,level_up,,0,\n";
        let rows = parse(text).unwrap();
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0], Row { species: 1, form: 0, group: Group::LevelUp, level: 1, move_id: 33 });
        assert_eq!(rows[1], Row { species: 1, form: 0, group: Group::Egg, level: 0, move_id: 133 });
        assert_eq!(rows[4].move_id, 0, "a row with the move 0 clears the group");
        assert!(parse(&text.replace('\n', "\r\n")).is_ok());
    }

    #[test]
    fn a_wrong_file_is_refused_with_its_line() {
        let head = format!("{HEADER}\n");
        let error = |line: &str| parse(&format!("{head}{line}\n")).unwrap_err();
        assert!(parse("species,form\n").unwrap_err().contains("header"));
        assert!(parse(&head).unwrap_err().contains("no rows"));
        assert!(error("1,0,Bulbasaur,level_up,1,33").contains("line 2: 6 fields"));
        assert!(error("0,0,x,level_up,1,33,x").contains("species is 0"));
        assert!(error("999,0,x,level_up,1,33,x").contains("species is 999"));
        assert!(error("1,0,x,moves,1,33,x").contains("`moves`"));
        assert!(error("1,0,x,level_up,101,33,x").contains("level is 101"));
        assert!(error("1,0,x,level_up,,33,x").contains("level is not a number"));
        assert!(error("1,0,x,level_up,0,33,x").contains("level is 0"));
        assert!(error("1,0,x,egg,5,33,x").contains("level must be empty"));
        assert!(error("1,0,x,tm,,9999,x").contains("move is 9999"));
        assert!(error("1,0,x,tm,,abc,x").contains("move is not a number"));
    }

    /// With the dump of the game: the file goes out and comes back, a change is written, and Bulbasaur has the
    /// moves of the Bulbapedia table.
    #[test]
    fn the_moves_of_the_dump_go_out_and_come_back() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let open = |n: usize| Garc::open(&dump.archive(n)).unwrap();
        let (species, level_up, eggs) = (open(crate::ARCHIVE), open(learnset::LEVEL_UP_ARCHIVE), open(learnset::EGG_ARCHIVE));
        let folder = std::env::temp_dir().join(format!("sango_learnset_csv_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let library = Library { species: &species, level_up: &level_up, eggs: &eggs, sd: Some(&sd) };

        let text = export(&library);
        let bulbasaur = |group: &str| text.lines().filter(|l| l.starts_with("1,0,") && l.split(',').nth(3) == Some(group)).count();
        assert_eq!((bulbasaur("level_up"), bulbasaur("egg"), bulbasaur("tm"), bulbasaur("tutor")), (14, 15, 30, 9));
        assert!(text.contains("\n1,0,Bulbasaur,level_up,1,33,Tackle\n"));
        assert!(text.contains("\n1,0,Bulbasaur,tm,,92,Toxic\n"));
        // The same file gives no change.
        let rows = parse(&text).unwrap();
        assert_eq!(apply(&rows, &library, &sd).unwrap(), Report::default());

        // A change: Bulbasaur loses Tackle and a TM, and gets a tutor move and an egg move.
        let edited: String = text
            .lines()
            .filter(|l| !l.starts_with("1,0,Bulbasaur,level_up,1,33,") && !l.starts_with("1,0,Bulbasaur,tm,,92,"))
            .chain(["1,0,Bulbasaur,tutor,,162,Super Fang", "1,0,Bulbasaur,egg,,1,Pound"])
            .map(|l| format!("{l}\n"))
            .collect();
        let report = apply(&parse(&edited).unwrap(), &library, &sd).unwrap();
        assert_eq!(report, Report { level_up_files: 1, egg_files: 1, data_files: 1 });
        let changed = export(&library);
        assert!(!changed.contains("\n1,0,Bulbasaur,level_up,1,33,"));
        assert!(changed.contains("\n1,0,Bulbasaur,tutor,,162,Super Fang\n"));
        assert!(changed.contains("\n1,0,Bulbasaur,egg,,1,Pound\n"));
        assert!(!changed.contains("\n1,0,Bulbasaur,tm,,92,"));
        // The other species did not change.
        assert_eq!(text.lines().filter(|l| l.starts_with("25,0,")).collect::<Vec<_>>(), changed.lines().filter(|l| l.starts_with("25,0,")).collect::<Vec<_>>());

        // Nothing is written when one row is wrong.
        let before = export(&library);
        let bad = format!("{edited}2,0,Ivysaur,tm,,33,Tackle\n1,0,Bulbasaur,tutor,,33,Tackle\n3,5,Venusaur,egg,,1,Pound\n");
        let message = apply(&parse(&bad).unwrap(), &library, &sd).unwrap_err();
        assert!(message.contains("Tackle is not the move of a TM"), "{message}");
        assert!(message.contains("no form 5"), "{message}");
        assert_eq!(export(&library), before);

        // A row with the move 0 clears a group.
        let clear = format!("{HEADER}\n1,0,Bulbasaur,egg,,0,\n");
        assert_eq!(apply(&parse(&clear).unwrap(), &library, &sd).unwrap().egg_files, 1);
        assert_eq!(export(&library).lines().filter(|l| l.starts_with("1,0,Bulbasaur,egg,")).count(), 0);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
