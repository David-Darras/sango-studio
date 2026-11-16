//! The names of the forms of the species of the game, for the menus of the pages.
//!
//! Each species lists the names of its forms in the order of the form numbers (the first name is the form 0). The names come from
//! the list that the plugin shows in its own menus.

/// The species of the game that have named forms, with the names of the forms.
const FORMS: [(u16, &[&str]); 83] = [
    (3, &["Normal", "Mega"]),
    (6, &["Normal", "Mega X", "Mega Y"]),
    (9, &["Normal", "Mega"]),
    (15, &["Normal", "Mega"]),
    (18, &["Normal", "Mega"]),
    (25, &["Normal", "Style", "Beautiful", "Cute", "Clever", "Strong", "Contest"]),
    (65, &["Normal", "Mega"]),
    (80, &["Normal", "Mega"]),
    (94, &["Normal", "Mega"]),
    (115, &["Normal", "Mega"]),
    (127, &["Normal", "Mega"]),
    (130, &["Normal", "Mega"]),
    (142, &["Normal", "Mega"]),
    (150, &["Normal", "Mega X", "Mega Y"]),
    (181, &["Normal", "Mega"]),
    (201, &["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "Exclamation", "Question"]),
    (208, &["Normal", "Mega"]),
    (212, &["Normal", "Mega"]),
    (214, &["Normal", "Mega"]),
    (229, &["Normal", "Mega"]),
    (248, &["Normal", "Mega"]),
    (254, &["Normal", "Mega"]),
    (257, &["Normal", "Mega"]),
    (260, &["Normal", "Mega"]),
    (282, &["Normal", "Mega"]),
    (302, &["Normal", "Mega"]),
    (303, &["Normal", "Mega"]),
    (306, &["Normal", "Mega"]),
    (308, &["Normal", "Mega"]),
    (310, &["Normal", "Mega"]),
    (319, &["Normal", "Mega"]),
    (323, &["Normal", "Mega"]),
    (334, &["Normal", "Mega"]),
    (351, &["Normal", "Sun", "Rain", "Snow"]),
    (354, &["Normal", "Mega"]),
    (359, &["Normal", "Mega"]),
    (362, &["Normal", "Mega"]),
    (373, &["Normal", "Mega"]),
    (376, &["Normal", "Mega"]),
    (380, &["Normal", "Mega"]),
    (381, &["Normal", "Mega"]),
    (382, &["Normal", "Alpha"]),
    (383, &["Normal", "Omega"]),
    (384, &["Normal", "Delta"]),
    (386, &["Normal", "Attack", "Defence", "Speed"]),
    (412, &["Plant", "Sand", "Trash"]),
    (413, &["Plant", "Ground", "Steel"]),
    (421, &["Overcast", "Sunshine"]),
    (422, &["West", "East"]),
    (423, &["West", "East"]),
    (428, &["Normal", "Mega"]),
    (445, &["Normal", "Mega"]),
    (448, &["Normal", "Mega"]),
    (460, &["Normal", "Mega"]),
    (475, &["Normal", "Mega"]),
    (479, &["Normal", "Heat", "Wash", "Frost", "Fan", "Mow"]),
    (487, &["Altered", "Origin"]),
    (492, &["Land", "Sky"]),
    (493, &["Normal", "Fighting", "Flying", "Poison", "Ground", "Rock", "Bug", "Ghost", "Steel", "Fire", "Water", "Grass", "Electric", "Psychic", "Ice", "Dragon", "Dark", "Fairy"]),
    (531, &["Normal", "Mega"]),
    (550, &["Black", "White"]),
    (555, &["Standard", "Zen"]),
    (585, &["Spring", "Summer", "Autumn", "Winter"]),
    (586, &["Spring", "Summer", "Autumn", "Winter"]),
    (641, &["Incarnate", "Therian"]),
    (642, &["Incarnate", "Therian"]),
    (645, &["Incarnate", "Therian"]),
    (646, &["Normal", "White", "Black"]),
    (647, &["Ordinary", "Resolute"]),
    (648, &["Aria", "Pirouette"]),
    (649, &["Normal", "Aqua", "Shock", "Blaze", "Freeze"]),
    (666, &["Area 1", "Area 2", "Area 3", "Area 4", "Area 5", "Area 6", "Area 7", "Area 8", "Area 9", "Area 10", "Area 11", "Area 12", "Area 13", "Area 14", "Area 15", "Area 16", "Area 17", "Area 18", "Area 19", "Area 20"]),
    (669, &["Red", "Yellow", "Orange", "Blue", "White"]),
    (670, &["Red", "Yellow", "Orange", "Blue", "White", "Eternal"]),
    (671, &["Red", "Yellow", "Orange", "Blue", "White"]),
    (676, &["Normal", "Heart", "Star", "Diamond", "Debutante", "Matron", "Dandy", "La Reine", "Kabuki", "Pharaoh"]),
    (678, &["Male", "Female"]),
    (681, &["Shield", "Blade"]),
    (710, &["Normal", "Small", "Large", "Super"]),
    (711, &["Normal", "Small", "Large", "Super"]),
    (716, &["Normal", "Active"]),
    (719, &["Normal", "Mega"]),
    (720, &["Confined", "Unbound"]),
];

/// The name of a form of a species of the game, when the list has it.
pub fn game_form_name(species: usize, form: usize) -> Option<&'static str> {
    let (_, names) = FORMS.iter().find(|(number, _)| *number as usize == species)?;
    names.get(form).copied()
}

/// The name of a form for the menus: the name of the list, "Mega" for a form of a pack that has an item, else "Form n".
pub fn form_label(species: usize, form: usize, pack_item: bool) -> String {
    match game_form_name(species, form) {
        Some(name) => name.to_string(),
        None if form == 0 => "Normal".to_string(),
        None if pack_item => format!("Mega (form {form})"),
        None => format!("Form {form}"),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_forms_have_the_names_of_the_list() {
        assert_eq!(game_form_name(6, 1), Some("Mega X"));
        assert_eq!(game_form_name(6, 2), Some("Mega Y"));
        assert_eq!(game_form_name(25, 3), Some("Cute"));
        assert_eq!(game_form_name(1, 0), None);
        assert_eq!(form_label(1, 0, false), "Normal");
        assert_eq!(form_label(6, 5, true), "Mega (form 5)");
        assert_eq!(form_label(6, 5, false), "Form 5");
    }
}
