//! The CSV files of the trainers: `trainers.csv` (the data) and `trainer_teams.csv` (one line for each Pokémon).

use crate::store::Store;
use sango_core::csv_text::{quote, records};
use sango_core::trainer::{self as data, Team, TrainerData, TrainerPokemon};
use sango_core::SdCard;
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::path::Path;

const TRAINERS_FILE: &str = "trainers.csv";
const TEAMS_FILE: &str = "trainer_teams.csv";
const TRAINERS_HEADER: &str = "trainer,name,class,fight_type,ai,item1,item2,item3,item4,money,heal_after,gift_item";
const TEAMS_HEADER: &str = "trainer,slot,species,form,level,iv,ability,gender,item,move1,move2,move3,move4";
/// The English language: the names of the file are the English ones.
const ENGLISH: usize = 2;

/// The state of the CSV card.
#[derive(Default)]
pub struct Card {
    status: Option<(bool, String)>,
}

/// The two files, as text, for all the trainers as the user has them now.
pub fn export(store: &mut Store, sd: Option<&SdCard>) -> Result<(String, String), String> {
    let (mut trainers, mut teams) = (format!("{TRAINERS_HEADER}\n"), format!("{TEAMS_HEADER}\n"));
    for id in 1..data::TRAINER_COUNT {
        let (trainer, team) = store.trainer(sd, id)?;
        let name = store.line(sd, ENGLISH, data::TRAINER_NAMES, id);
        let items = trainer.items.map(|item| item.to_string()).join(",");
        trainers.push_str(&format!("{id},{},{},{},{},{items},{},{},{}\n", quote(&name), trainer.class, trainer.fight_type, trainer.ai, trainer.money, u8::from(trainer.heal_after), trainer.gift_item));
        for (slot, pokemon) in team.pokemon.iter().enumerate() {
            let moves = pokemon.moves.map(|mv| mv.to_string()).join(",");
            teams.push_str(&format!("{id},{},{},{},{},{},{},{},{},{moves}\n", slot + 1, pokemon.species, pokemon.form, pokemon.level, pokemon.iv, pokemon.ability, pokemon.gender, pokemon.item));
        }
    }
    Ok((trainers, teams))
}

/// A number of a field, with the place of the error.
fn number<T: std::str::FromStr>(fields: &[String], at: usize, line: usize, column: &str, max: u64) -> Result<T, String> {
    let text = fields.get(at).map(|field| field.trim()).unwrap_or_default();
    let value: u64 = text.parse().map_err(|_| format!("Line {line}, column {column}: \"{text}\" is not a number."))?;
    if value > max {
        return Err(format!("Line {line}, column {column}: {value} is more than {max}."));
    }
    text.parse::<T>().map_err(|_| format!("Line {line}, column {column}: \"{text}\" is not a number."))
}

/// The trainers and the teams that the two files give, checked: the trainer, its name (English) and its team.
#[derive(Debug)]
pub struct Imported {
    pub trainers: Vec<(usize, String, TrainerData, Team)>,
}

/// Reads the two texts and checks them against the trainers as they are now.
pub fn parse(store: &Store, sd: Option<&SdCard>, trainers_text: &str, teams_text: &str) -> Result<Imported, String> {
    let rows = records(trainers_text)?;
    match rows.first() {
        Some((_, header)) if header.join(",") == TRAINERS_HEADER => {}
        _ => return Err(format!("The first line of {TRAINERS_FILE} is not \"{TRAINERS_HEADER}\".")),
    }
    let team_rows = records(teams_text)?;
    match team_rows.first() {
        Some((_, header)) if header.join(",") == TEAMS_HEADER => {}
        _ => return Err(format!("The first line of {TEAMS_FILE} is not \"{TEAMS_HEADER}\".")),
    }
    let mut pokemon: std::collections::BTreeMap<usize, Vec<(usize, TrainerPokemon)>> = Default::default();
    for (line, fields) in team_rows.iter().skip(1).filter(|(_, fields)| !(fields.len() == 1 && fields[0].is_empty())) {
        if fields.len() != 13 {
            return Err(format!("Line {line} of {TEAMS_FILE} has {} columns, not 13.", fields.len()));
        }
        let id: usize = number(fields, 0, *line, "trainer", data::TRAINER_COUNT as u64 - 1)?;
        let slot: usize = number(fields, 1, *line, "slot", data::MAX_TEAM as u64)?;
        let entry = TrainerPokemon {
            species: number(fields, 2, *line, "species", 1100)?,
            form: number(fields, 3, *line, "form", 255)?,
            level: number(fields, 4, *line, "level", 100)?,
            iv: number(fields, 5, *line, "iv", 255)?,
            ability: number(fields, 6, *line, "ability", 3)?,
            gender: number(fields, 7, *line, "gender", 2)?,
            item: number(fields, 8, *line, "item", 65535)?,
            moves: [number(fields, 9, *line, "move1", 65535)?, number(fields, 10, *line, "move2", 65535)?, number(fields, 11, *line, "move3", 65535)?, number(fields, 12, *line, "move4", 65535)?],
        };
        if slot == 0 || entry.level == 0 || entry.species == 0 {
            return Err(format!("Line {line} of {TEAMS_FILE}: the slot, the species and the level start at 1."));
        }
        pokemon.entry(id).or_default().push((slot, entry));
    }
    let mut out = Vec::new();
    for (line, fields) in rows.iter().skip(1).filter(|(_, fields)| !(fields.len() == 1 && fields[0].is_empty())) {
        if fields.len() != 12 {
            return Err(format!("Line {line} of {TRAINERS_FILE} has {} columns, not 12.", fields.len()));
        }
        let id: usize = number(fields, 0, *line, "trainer", data::TRAINER_COUNT as u64 - 1)?;
        if id == 0 {
            return Err(format!("Line {line} of {TRAINERS_FILE}: the trainers start at 1."));
        }
        let (mut trainer, _) = store.trainer(sd, id)?;
        trainer.class = number(fields, 2, *line, "class", data::CLASS_COUNT as u64 - 1)?;
        trainer.fight_type = number(fields, 3, *line, "fight_type", 4)?;
        trainer.ai = number(fields, 4, *line, "ai", u32::MAX as u64)?;
        for (slot, name) in ["item1", "item2", "item3", "item4"].iter().enumerate() {
            trainer.items[slot] = number(fields, 5 + slot, *line, name, 65535)?;
        }
        trainer.money = number(fields, 9, *line, "money", 255)?;
        trainer.heal_after = number::<u8>(fields, 10, *line, "heal_after", 1)? == 1;
        trainer.gift_item = number(fields, 11, *line, "gift_item", 65535)?;
        let mut members = pokemon.remove(&id).unwrap_or_default();
        members.sort_by_key(|(slot, _)| *slot);
        if members.is_empty() || members.len() > data::MAX_TEAM {
            return Err(format!("The trainer {id} has {} Pokémon in {TEAMS_FILE}: a team has 1 to {}.", members.len(), data::MAX_TEAM));
        }
        let team = Team {
            moves: members.iter().any(|(_, entry)| entry.moves != [0; 4]),
            items: members.iter().any(|(_, entry)| entry.item != 0),
            pokemon: members.into_iter().map(|(_, entry)| entry).collect(),
        };
        out.push((id, fields[1].clone(), trainer, team));
    }
    if let Some(id) = pokemon.keys().next() {
        return Err(format!("{TEAMS_FILE} has Pokémon for the trainer {id}, which is not in {TRAINERS_FILE}."));
    }
    Ok(Imported { trainers: out })
}

/// Writes the checked trainers on the SD card. Gives the number of trainers.
pub fn write(store: &mut Store, sd: &SdCard, imported: Imported) -> Result<usize, String> {
    let count = imported.trainers.len();
    for (id, name, trainer, team) in imported.trainers {
        let (old, old_team) = store.trainer(Some(sd), id)?;
        // A team that only differs by a form flag that nothing uses is not a change.
        if old != trainer || old_team.pokemon != team.pokemon {
            store.save_trainer(sd, id, &trainer, &team)?;
        }
        if store.line(Some(sd), ENGLISH, data::TRAINER_NAMES, id) != name {
            store.set_line(sd, ENGLISH, data::TRAINER_NAMES, id, &name)?;
        }
    }
    Ok(count)
}

fn read(folder: &Path, name: &str) -> Result<String, String> {
    std::fs::read_to_string(folder.join(name)).map_err(|e| format!("{}: {e}", folder.join(name).display()))
}

impl Card {
    /// Draws the card. Gives true when the files changed.
    pub fn ui(&mut self, ui: &mut egui::Ui, store: &mut Store, sd: &SdCard) -> bool {
        let mut changed = false;
        widget::card(ui, |ui| {
            ui.label(widget::title("CSV files of all the trainers"));
            ui.label(widget::hint("These buttons work on all the trainers, not only on the one that is open."));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if widget::button(ui, "Export CSV…") {
                    self.status = Some(match export_files(store, sd) {
                        Ok(Some(path)) => (true, format!("Wrote {TRAINERS_FILE} and {TEAMS_FILE} in {path}.")),
                        Ok(None) => (true, "Nothing was written.".to_string()),
                        Err(message) => (false, message),
                    });
                }
                if widget::button(ui, "Import CSV…") {
                    match import_files(store, sd) {
                        Ok(Some(count)) => {
                            changed = true;
                            self.status = Some((true, format!("Imported {count} trainers. Restart the game to see the change.")));
                        }
                        Ok(None) => {}
                        Err(message) => self.status = Some((false, message)),
                    }
                }
            });
            ui.label(widget::hint(&format!("Export writes {TRAINERS_FILE} (the data and the English name of each trainer) and {TEAMS_FILE} (one line for each Pokémon) in a folder that you choose. Import reads the two files from a folder, checks all of them, and writes the changes on the SD card.")));
            if let Some((ok, message)) = &self.status {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }));
            }
        });
        changed
    }
}

fn export_files(store: &mut Store, sd: &SdCard) -> Result<Option<String>, String> {
    let Some(folder) = rfd::FileDialog::new().set_title("Folder for the CSV files").pick_folder() else { return Ok(None) };
    let (trainers, teams) = export(store, Some(sd))?;
    std::fs::write(folder.join(TRAINERS_FILE), trainers).map_err(|e| format!("{TRAINERS_FILE}: {e}"))?;
    std::fs::write(folder.join(TEAMS_FILE), teams).map_err(|e| format!("{TEAMS_FILE}: {e}"))?;
    Ok(Some(folder.display().to_string()))
}

fn import_files(store: &mut Store, sd: &SdCard) -> Result<Option<usize>, String> {
    let Some(folder) = rfd::FileDialog::new().set_title("Folder of the CSV files").pick_folder() else { return Ok(None) };
    let imported = parse(store, Some(sd), &read(&folder, TRAINERS_FILE)?, &read(&folder, TEAMS_FILE)?)?;
    let count = write(store, sd, imported)?;
    store.forget_texts();
    Ok(Some(count))
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: the export of all the trainers is read back and changes nothing.
    #[test]
    fn the_export_of_the_game_is_imported_without_a_change() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let mut store = Store::open(&dump).unwrap();
        let (trainers, teams) = export(&mut store, None).unwrap();
        assert!(trainers.starts_with(TRAINERS_HEADER) && teams.starts_with(TEAMS_HEADER));
        let imported = parse(&store, None, &trainers, &teams).unwrap();
        assert_eq!(imported.trainers.len(), data::TRAINER_COUNT - 1);
        for (id, _, trainer, team) in &imported.trainers {
            let (game, game_team) = store.game_trainer(*id).unwrap();
            assert_eq!((trainer.class, trainer.ai, trainer.items, trainer.money), (game.class, game.ai, game.items, game.money), "trainer {id}");
            assert_eq!(team.pokemon, game_team.pokemon, "team {id}");
        }
        // A wrong number is refused with its place.
        let broken = trainers.replacen("\n1,Brendan,127,0", "\n1,Brendan,x,0", 1);
        assert!(parse(&store, None, &broken, &teams).unwrap_err().contains("column class"));
        // A folder with the files: the write on a card then reads back the same trainers.
        let folder = std::env::temp_dir().join(format!("sango_trainer_csv_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let changed_teams = teams.replacen("\n1,1,252,0,5,", "\n1,1,252,0,9,", 1);
        let imported = parse(&store, Some(&sd), &trainers, &changed_teams).unwrap();
        assert_eq!(write(&mut store, &sd, imported).unwrap(), data::TRAINER_COUNT - 1);
        let (_, team) = store.trainer(Some(&sd), 1).unwrap();
        assert_eq!(team.pokemon[0].level, 9);
        assert_eq!(Store::changed_trainers(Some(&sd)).len(), 1, "only the trainer that changed is written");
        let _ = std::fs::remove_dir_all(&folder);
    }
}
