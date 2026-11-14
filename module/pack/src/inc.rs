//! Reads the data files that the plugin had before the packs (`gen7_species.inc`, `gen8_species.inc`, `alolan_form.inc`) and
//! makes packs from them. The plugin stops using these files when it reads the table of the packs.
//!
//! The files are C++ initialisers: nested lists of numbers, with `u"..."` texts. A small reader is enough for them.

use crate::pack::{to_hex, Evolution, ModelRef, Move, Pack, PackForm, PackSpecies, Ref, DATA_SIZE, MODEL_FILES};
use std::collections::HashMap;
use std::path::PathBuf;
use crate::table::{FormRecord, SpeciesRecord, Table, FIRST_SPECIES};

/// A value of an initialiser.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(i64),
    Text(String),
    List(Vec<Value>),
}

impl Value {
    fn list(&self) -> Result<&[Value], String> {
        match self {
            Value::List(items) => Ok(items),
            other => Err(format!("A list is expected, not {other:?}.")),
        }
    }

    fn number(&self) -> Result<i64, String> {
        match self {
            Value::Number(n) => Ok(*n),
            other => Err(format!("A number is expected, not {other:?}.")),
        }
    }

    fn text(&self) -> Result<&str, String> {
        match self {
            Value::Text(t) => Ok(t),
            other => Err(format!("A text is expected, not {other:?}.")),
        }
    }
}

/// Takes the comments away.
fn strip_comments(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('/', Some('*')) => {
                chars.next();
                while let Some(d) = chars.next() {
                    if d == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        break;
                    }
                }
            }
            ('/', Some('/')) => {
                for d in chars.by_ref() {
                    if d == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// The list that follows `name[] =` in the text.
pub fn initialiser(text: &str, name: &str) -> Result<Vec<Value>, String> {
    let text = strip_comments(text);
    let start = text.find(&format!("{name}[]")).ok_or_else(|| format!("{name} is not in the file."))?;
    let rest = &text[start..];
    let open = rest.find('{').ok_or("The list does not start.")?;
    let chars: Vec<char> = rest[open..].chars().collect();
    let mut at = 0;
    match parse_value(&chars, &mut at)? {
        Value::List(items) => Ok(items),
        _ => Err("A list is expected.".into()),
    }
}

fn skip_blanks(chars: &[char], at: &mut usize) {
    while *at < chars.len() && (chars[*at].is_whitespace() || chars[*at] == ',') {
        *at += 1;
    }
}

fn parse_value(chars: &[char], at: &mut usize) -> Result<Value, String> {
    skip_blanks(chars, at);
    match chars.get(*at) {
        Some('{') => {
            *at += 1;
            let mut items = Vec::new();
            loop {
                skip_blanks(chars, at);
                match chars.get(*at) {
                    Some('}') => {
                        *at += 1;
                        return Ok(Value::List(items));
                    }
                    Some(_) => items.push(parse_value(chars, at)?),
                    None => return Err("A list does not end.".into()),
                }
            }
        }
        Some('u') if chars.get(*at + 1) == Some(&'"') => {
            *at += 2;
            let mut text = String::new();
            while let Some(&c) = chars.get(*at) {
                *at += 1;
                match c {
                    '"' => return Ok(Value::Text(text)),
                    '\\' => {
                        text.push(chars.get(*at).copied().ok_or("A text does not end.")?);
                        *at += 1;
                    }
                    _ => text.push(c),
                }
            }
            Err("A text does not end.".into())
        }
        Some(c) if c.is_ascii_digit() || *c == '-' => {
            let start = *at;
            while *at < chars.len() && (chars[*at].is_ascii_alphanumeric() || chars[*at] == '-') {
                *at += 1;
            }
            let word: String = chars[start..*at].iter().collect();
            let number = match word.strip_prefix("0x").or_else(|| word.strip_prefix("0X")) {
                Some(hex) => i64::from_str_radix(hex, 16),
                None => word.parse::<i64>(),
            };
            number.map(Value::Number).map_err(|_| format!("'{word}' is not a number."))
        }
        other => Err(format!("Unexpected {other:?} in a list.")),
    }
}

fn bytes(value: &Value) -> Result<Vec<u8>, String> {
    let data: Vec<u8> = value.list()?.iter().map(|v| v.number().map(|n| n as u8)).collect::<Result<_, _>>()?;
    if data.len() != DATA_SIZE {
        return Err(format!("The data has {} bytes, not {DATA_SIZE}.", data.len()));
    }
    Ok(data)
}

/// A species of `gen7_species.inc` or `gen8_species.inc`, as the plugin read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncSpecies {
    pub id: u16,
    pub model_form: u16,
    pub name: String,
    pub data: Vec<u8>,
    pub moves: Vec<(u16, u8)>,
    pub evolutions: Vec<(u8, u16, u16)>,
}

/// A form of `alolan_form.inc`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncForm {
    pub species: u16,
    pub form: u16,
    pub model_form: u16,
    pub data: Vec<u8>,
}

/// The species of a file: `array` is `kGen7Species` or `kGen8Species`.
pub fn read_species(text: &str, array: &str) -> Result<Vec<IncSpecies>, String> {
    initialiser(text, array)?
        .iter()
        .map(|entry| {
            let fields = entry.list()?;
            if fields.len() != 6 {
                return Err(format!("An entry of {array} has {} fields, not 6.", fields.len()));
            }
            let moves = fields[4].list()?.iter().map(|m| Ok((m.list()?[0].number()? as u16, m.list()?[1].number()? as u8))).collect::<Result<Vec<_>, String>>()?;
            let evolutions = fields[5]
                .list()?
                .iter()
                .map(|e| {
                    let e = e.list()?;
                    Ok((e[0].number()? as u8, e[2].number()? as u16, e[3].number()? as u16))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(IncSpecies { id: fields[0].number()? as u16, model_form: fields[1].number()? as u16, name: fields[2].text()?.to_string(), data: bytes(&fields[3])?, moves, evolutions })
        })
        .collect()
}

/// The forms of `alolan_form.inc`.
pub fn read_forms(text: &str) -> Result<Vec<IncForm>, String> {
    initialiser(text, "kAlolanForms")?
        .iter()
        .map(|entry| {
            let fields = entry.list()?;
            if fields.len() != 4 {
                return Err(format!("A form has {} fields, not 4.", fields.len()));
            }
            Ok(IncForm { species: fields[0].number()? as u16, form: fields[1].number()? as u16, model_form: fields[2].number()? as u16, data: bytes(&fields[3])? })
        })
        .collect()
}

/// What the plugin built from these files, as a table: the reference to compare with the table of the packs. The species of
/// the first file come first, then the species of the second file, then the forms.
pub fn table_from_inc(species: &[IncSpecies], forms: &[IncForm]) -> Result<Table, String> {
    let mut table = Table::default();
    for (place, entry) in species.iter().enumerate() {
        if entry.id != FIRST_SPECIES + place as u16 {
            return Err(format!("The species {} is not at the place {place}.", entry.id));
        }
        table.species.push(SpeciesRecord {
            id: entry.id,
            model_form: entry.model_form,
            name: entry.name.clone(),
            data: entry.data.clone(),
            moves: entry.moves.iter().copied().filter(|(id, _)| *id != 0).collect(),
            evolutions: entry.evolutions.iter().copied().filter(|(_, _, target)| *target != 0).collect(),
        });
    }
    for form in forms {
        table.forms.push(FormRecord { species: form.species, form: form.form, model_form: form.model_form, data: form.data.clone() });
    }
    Ok(table)
}

fn reference(number: u16, names: &std::collections::HashMap<u16, String>) -> Result<Ref, String> {
    if (1..=721).contains(&number) {
        Ok(Ref::Game(number))
    } else {
        names.get(&number).map(|name| Ref::Key(name.clone())).ok_or_else(|| format!("The species {number} is not in the files."))
    }
}

/// The models that the plugin read before the packs: the folder `sango/pokemodel` (or `sango_off/pokemodel`) of the SD card,
/// with `0.raw`, `index.bin` and the files `<file>.pc`. A *model form* (the number that the data files give) is a slot of a
/// carrier species.
pub struct OldModels {
    folder: PathBuf,
    /// The list of the old files (`0.raw` without its size), and the list of the game (the file 0 of the archive 8).
    old_list: Vec<u8>,
    base_list: Vec<u8>,
}

/// The first slot and the number of slots of a species in a list of the models.
fn list_entry(list: &[u8], species: usize) -> Result<(usize, usize), String> {
    let at = (species - 1) * 4;
    let entry = list.get(at..at + 4).ok_or("A list of the models is too short.")?;
    Ok((u16::from_le_bytes([entry[0], entry[1]]) as usize, entry[2] as usize))
}

impl OldModels {
    /// `base_list` is the file 0 of the archive 8 of the game.
    pub fn open(folder: &std::path::Path, base_list: &[u8]) -> Result<OldModels, String> {
        let raw = std::fs::read(folder.join("0.raw")).map_err(|e| format!("{}/0.raw: {e}", folder.display()))?;
        let old_list = raw.get(4..).ok_or("0.raw is too short.")?.to_vec();
        Ok(OldModels { folder: folder.to_path_buf(), old_list, base_list: base_list.to_vec() })
    }

    /// The slot of a model form: a form of a carrier species.
    pub fn slot_of(&self, model_form: u16) -> Result<usize, String> {
        let (carrier, form) = ((model_form / 256) as usize, (model_form % 256) as usize);
        let species = *[494usize, 721].get(carrier).ok_or_else(|| format!("The model form {model_form} is not a carrier form."))?;
        Ok(list_entry(&self.old_list, species)?.0 + form)
    }

    /// The slots that the old files added at the end of the list of a species of the game: the first one and the number.
    pub fn extension(&self, species: u16) -> Result<(usize, usize), String> {
        let (first, count) = list_entry(&self.old_list, species as usize)?;
        let (_, base_count) = list_entry(&self.base_list, species as usize)?;
        let extra = count.checked_sub(base_count).ok_or_else(|| format!("The species {species} has fewer models than the game."))?;
        Ok((first + base_count, extra))
    }

    /// The 8 files of the model of a model form (the data of each file, without the size in front).
    pub fn files(&self, model_form: u16) -> Result<Vec<Vec<u8>>, String> {
        self.files_of_slot(self.slot_of(model_form)?)
    }

    /// The 8 files of a slot.
    pub fn files_of_slot(&self, slot: usize) -> Result<Vec<Vec<u8>>, String> {
        (0..MODEL_FILES)
            .map(|file| {
                let path = self.folder.join(format!("{}.pc", slot * 8 + 3 + file));
                let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                bytes.get(4..).map(|data| data.to_vec()).ok_or_else(|| format!("{} is too short.", path.display()))
            })
            .collect()
    }
}

/// A pack made from the old files, with what is needed to find the files of its models.
pub struct Migrated {
    pub pack: Pack,
    /// For each model of the pack that is in a carrier species, the model form that the old files gave it.
    pub models: HashMap<String, u16>,
    /// For each model of a form of a species of the game, the slot that the old files gave it.
    pub slots: HashMap<String, usize>,
}

fn model_ref(model_form: u16, name: &str, models: &mut HashMap<String, u16>) -> Option<ModelRef> {
    if model_form & 0x8000 != 0 {
        return Some(ModelRef::Native { native: model_form & 0x7FFF });
    }
    // Two entries with the same old model share one model of the pack.
    let file = models.iter().find(|(_, form)| **form == model_form).map(|(file, _)| file.clone()).unwrap_or_else(|| {
        models.insert(name.to_string(), model_form);
        name.to_string()
    });
    Some(ModelRef::File { file })
}

fn pack_species(entries: &[IncSpecies], names: &HashMap<u16, String>, models: &mut HashMap<String, u16>) -> Result<Vec<PackSpecies>, String> {
    entries
        .iter()
        .map(|entry| {
            Ok(PackSpecies {
                name: entry.name.clone(),
                model: model_ref(entry.model_form, &entry.name, models).ok_or("A species has no model.")?,
                data: to_hex(&entry.data),
                moves: entry.moves.iter().filter(|(id, _)| *id != 0).map(|&(id, level)| Move { id, level }).collect(),
                evolutions: entry
                    .evolutions
                    .iter()
                    .filter(|(_, _, target)| *target != 0)
                    .map(|&(method, arg, target)| Ok(Evolution { method, arg, species: reference(target, names)? }))
                    .collect::<Result<_, String>>()?,
            })
        })
        .collect()
}

/// The three packs of the files that the plugin had: `gen7`, `gen8` and `forms` (the regional forms and the new Mega Evolutions).
/// Install them in this order to get the numbers that the plugin gave. A model that two packs use is in both.
pub fn packs_from_inc(gen7: &[IncSpecies], gen8: &[IncSpecies], forms: &[IncForm], old: &OldModels) -> Result<Vec<Migrated>, String> {
    let names: HashMap<u16, String> = gen7.iter().chain(gen8).map(|s| (s.id, s.name.clone())).collect();
    let credit = "Data of the species and models, made for Sango Studio by ZettaD.".to_string();
    let base = |id: &str, name: &str| Pack { id: id.into(), name: name.into(), version: "1.0".into(), author: "ZettaD".into(), license: String::new(), credit: credit.clone(), ..Pack::default() };
    let mut seven = Migrated { pack: base("gen7", "Generation VII species"), models: HashMap::new(), slots: HashMap::new() };
    seven.pack.species = pack_species(gen7, &names, &mut seven.models)?;
    let mut eight = Migrated { pack: base("gen8", "Generation VIII and IX species"), models: HashMap::new(), slots: HashMap::new() };
    eight.pack.species = pack_species(gen8, &names, &mut eight.models)?;
    let mut extra = Migrated { pack: base("forms", "Regional forms and new Mega Evolutions"), models: HashMap::new(), slots: HashMap::new() };
    let mut made = Vec::new();
    // The forms of a species of the game that have no model form have their model at the end of the list of the models of the
    // species: the old files added one slot for each form (two for a species that has a female model that the form needs).
    let mut forms_of: HashMap<u16, usize> = HashMap::new();
    for form in forms.iter().filter(|f| f.model_form == 0 && (1..=721).contains(&f.species)) {
        *forms_of.entry(form.species).or_default() += 1;
    }
    let mut taken: HashMap<u16, usize> = HashMap::new();
    for form in forms {
        let label = match reference(form.species, &names)? {
            Ref::Game(number) => format!("{number}"),
            Ref::Key(name) => name,
        };
        let name = format!("{label}#{}", form.form);
        let (mut model, mut female_model) = (None, None);
        if form.model_form != 0 {
            model = model_ref(form.model_form, &name, &mut extra.models);
        } else if (1..=721).contains(&form.species) {
            let (start, slots) = old.extension(form.species)?;
            let count = forms_of[&form.species];
            if slots > 0 {
                let own = if slots == count { 1 } else if count == 1 && slots <= 2 { slots } else { return Err(format!("The species {} has {slots} new slots for {count} forms.", form.species)) };
                let offset = taken.entry(form.species).or_default();
                extra.slots.insert(name.clone(), start + *offset);
                model = Some(ModelRef::File { file: name.clone() });
                if own == 2 {
                    let second = format!("{name} 2");
                    extra.slots.insert(second.clone(), start + *offset + 1);
                    female_model = Some(ModelRef::File { file: second });
                }
                *offset += own;
            }
        }
        made.push(PackForm { species: reference(form.species, &names)?, form: form.form, name: String::new(), model, female_model, data: to_hex(&form.data) });
    }
    extra.pack.forms = made;
    Ok(vec![seven, eight, extra])
}
