//! The new abilities: the Ability page lists them with the abilities of the game and edits their texts with the same menus. Only the
//! reactions have their own tab (see the crate `sango-rules`). The page writes one file for each ability on the SD card (see
//! [`crate::rules`]).

use crate::rules::{self, Context, CustomAbility};
use sango_core::text::LANGUAGES;
use sango_core::SdCard;
use sango_ui::theme::color;
use sango_ui::{egui, widget};

/// The folder of the files on the SD card.
const FOLDER: &str = "ability";

/// The new abilities of the user.
#[derive(Default)]
pub struct Store {
    /// The abilities, as the user has them now.
    list: Vec<CustomAbility>,
    /// The abilities as they are on the SD card: to know what changed.
    saved: Vec<CustomAbility>,
    loaded: bool,
    root: Option<std::path::PathBuf>,
}

impl Store {
    /// Reads the files of the SD card the first time, and when the folder of the card changes.
    pub fn ensure(&mut self, sd: Option<&SdCard>) {
        let root = sd.map(|sd| sd.root().to_path_buf());
        if self.loaded && self.root == root {
            return;
        }
        self.root = root;
        self.reload(sd);
    }

    /// Reads the files of the SD card again.
    pub fn reload(&mut self, sd: Option<&SdCard>) {
        self.list.clear();
        if let Some(sd) = sd {
            for path in sd.files_in(FOLDER) {
                if let Some(ability) = sd.read(&path).and_then(|bytes| CustomAbility::from_bytes(&bytes).ok()) {
                    self.list.push(ability);
                }
            }
        }
        self.list.sort_by_key(|a| a.id);
        self.saved = self.list.clone();
        self.loaded = true;
    }

    /// The numbers of the new abilities.
    pub fn ids(&self) -> Vec<usize> {
        self.list.iter().map(|a| a.id as usize).collect()
    }

    pub fn find(&self, id: usize) -> Option<&CustomAbility> {
        self.list.iter().find(|a| a.id as usize == id)
    }

    pub fn find_mut(&mut self, id: usize) -> Option<&mut CustomAbility> {
        self.list.iter_mut().find(|a| a.id as usize == id)
    }

    /// True when a new ability changed since it was read or saved.
    pub fn changed(&self) -> bool {
        self.list != self.saved
    }

    /// Makes a new ability with the next free number. It starts with the texts and the reactions of `template` (the new ability
    /// that is open), or with the texts "New ability" and one reaction that raises the Speed of the Pokémon when it enters the
    /// battle. Gives its number.
    pub fn add(&mut self, template: Option<usize>) -> Result<usize, String> {
        if self.list.len() >= rules::MAX_ABILITIES {
            return Err(format!("The plugin keeps {} new abilities at most: delete one first.", rules::MAX_ABILITIES));
        }
        let used: Vec<u8> = self.list.iter().map(|a| a.id).collect();
        let id = CustomAbility::free_id(&used).ok_or("All the ids from 192 to 255 are used.")?;
        let mut ability = CustomAbility::new(id, "New ability", "");
        if let Some(from) = template.and_then(|t| self.find(t)) {
            ability.names = from.names.clone();
            ability.descriptions = from.descriptions.clone();
            ability.rules = from.rules.clone();
        }
        self.list.push(ability);
        self.list.sort_by_key(|a| a.id);
        Ok(id as usize)
    }

    /// Writes the new abilities that changed, and tells the plugin. Gives the number of files that were written.
    pub fn save(&mut self, sd: &SdCard) -> Result<usize, String> {
        let mut written = 0;
        for ability in &self.list {
            if self.saved.iter().any(|saved| saved == ability) {
                continue;
            }
            if let Some(first) = ability.problems().first() {
                return Err(format!("#{} {}: {first}", ability.id, ability.names[rules::ENGLISH]));
            }
            let path = CustomAbility::path(ability.id);
            sd.write(&path, &ability.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
            written += 1;
        }
        self.saved = self.list.clone();
        Ok(written)
    }

    /// Deletes a new ability: its file too.
    pub fn delete(&mut self, sd: &SdCard, id: usize) -> Result<(), String> {
        let id8 = id as u8;
        let path = CustomAbility::path(id8);
        sd.remove(&path).and_then(|_| sd.mark_changed(&path))?;
        self.list.retain(|a| a.id != id8);
        self.saved.retain(|a| a.id != id8);
        Ok(())
    }
}

/// The name and the description of a new ability in a language: the Text tab of a new ability.
pub fn texts_ui(ui: &mut egui::Ui, ability: &mut CustomAbility, language: &mut usize) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for (i, entry) in LANGUAGES.iter().enumerate() {
            if widget::tab_button(ui, entry.name, *language == i, false).clicked() {
                *language = i;
            }
        }
    });
    ui.add_space(8.0);
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::hint("A new ability starts with the same text in every language: write the text of this language."));
        ui.add_space(6.0);
        let language = (*language).min(7);
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut ability.names[language]).desired_width(300.0));
        });
        ui.label("Description");
        ui.add(egui::TextEdit::multiline(&mut ability.descriptions[language]).desired_rows(3).desired_width(520.0));
    });
}

/// What a new ability does: its reactions. The Reactions tab of a new ability.
pub fn reactions_ui(ui: &mut egui::Ui, ability: &mut CustomAbility) {
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title("What the ability does"));
        ui.label(widget::hint("Each reaction says: when this happens, if all these are true, do these things. An ability can have several reactions, for different moments."));
        sango_rules::editor::rules_editor(ui, &format!("ability_{}", ability.id), &mut ability.rules, Context::Ability);
    });
    ui.add_space(10.0);
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title("Summary"));
        for sentence in ability.preview() {
            ui.label(sentence);
        }
        let problems = ability.problems();
        if !problems.is_empty() {
            ui.add_space(6.0);
            for problem in problems {
                ui.label(egui::RichText::new(problem).color(color::ROSE).size(13.5));
            }
        }
    });
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rules::{Action, ActionKind, Condition, ConditionKind, Rule};

    fn folder(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("sango_custom_{name}_{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn adding_saving_copying_and_deleting_an_ability() {
        let path = folder("store");
        let sd = SdCard::open(path.to_str().unwrap()).unwrap();
        let mut store = Store::default();
        store.ensure(Some(&sd));
        assert!(store.ids().is_empty());
        let id = store.add(None).unwrap();
        assert_eq!(id, 192);
        assert_eq!(store.find(192).unwrap().names[rules::ENGLISH], "New ability");
        assert!(store.changed());
        // A copy of an ability keeps its texts and its reactions.
        store.find_mut(192).unwrap().names[rules::ENGLISH] = "Quick Start".into();
        store.find_mut(192).unwrap().rules.push(Rule { moment: 148, conditions: vec![Condition { kind: ConditionKind::OnlyOnce, arg: 0 }], actions: vec![Action::new(ActionKind::SetTerrain)] });
        assert_eq!(store.add(Some(192)).unwrap(), 193);
        assert_eq!(store.find(193).unwrap().rules, store.find(192).unwrap().rules);
        store.save(&sd).unwrap();
        assert!(!store.changed() && sd.read("ability/192.bin").is_some() && sd.read("ability/193.bin").is_some());
        let mut other = Store::default();
        other.ensure(Some(&sd));
        assert_eq!(other.ids(), vec![192, 193]);
        other.delete(&sd, 192).unwrap();
        assert_eq!(other.ids(), vec![193]);
        assert!(sd.read("ability/192.bin").is_none());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn the_limit_of_abilities_is_kept() {
        let mut store = Store::default();
        for _ in 0..rules::MAX_ABILITIES {
            store.add(None).unwrap();
        }
        assert!(store.add(None).unwrap_err().contains("at most"));
    }

    #[test]
    fn the_cards_draw() {
        let mut ability = CustomAbility::new(192, "Test", "");
        let mut language = 2;
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    texts_ui(ui, &mut ability, &mut language);
                    reactions_ui(ui, &mut ability);
                });
            });
        }
    }
}
