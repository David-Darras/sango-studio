//! Which move each TM, HM and tutor bit teaches.
//!
//! The species data has one bit for each TM and HM (`tm_hm_moves`) and one bit for each move of the
//! tutors (`special_tutor_moves` and `bp_tutor_moves`). The bits do not say which move they mean: the game
//! has the tables. The numbers below are the move numbers (see `move_names::MOVE`: the move 1 is Pound).
//! They were read from the game and checked against the lists of Bulbapedia for Bulbasaur.

/// The move of each bit of `tm_hm_moves`: TM01 to TM100, then HM01 to HM07.
pub const MACHINES: [u16; 107] = [
    468, 337, 473, 347, 46, 92, 258, 339, 474, 237, // TM01 - TM10
    241, 269, 58, 59, 63, 113, 182, 240, 355, 219, // TM11 - TM20
    218, 76, 479, 85, 87, 89, 216, 91, 94, 247, // TM21 - TM30
    280, 104, 115, 482, 53, 188, 201, 126, 317, 332, // TM31 - TM40
    259, 263, 488, 156, 213, 168, 490, 496, 497, 315, // TM41 - TM50
    211, 411, 412, 206, 503, 374, 451, 507, 510, 511, // TM51 - TM60
    261, 512, 373, 153, 421, 371, 514, 416, 397, 148, // TM61 - TM70
    444, 521, 86, 360, 14, 522, 244, 523, 524, 157, // TM71 - TM80
    404, 525, 611, 398, 138, 447, 207, 214, 369, 164, // TM81 - TM90
    430, 433, 528, 290, 555, 267, 399, 612, 605, 590, // TM91 - TM100
    15, 19, 57, 70, 127, 249, 291, // HM01 - HM07
];

/// The number of TMs. The next bits are the HMs.
pub const TM_COUNT: usize = 100;

/// The name of the machine of a bit: "TM06", "HM01".
pub fn machine_name(bit: usize) -> String {
    if bit < TM_COUNT {
        format!("TM{:02}", bit + 1)
    } else {
        format!("HM{:02}", bit - TM_COUNT + 1)
    }
}

/// The moves of the four tutors that charge Battle Points, in the order of their bits.
pub const BP_TUTORS: [&[u16]; 4] = [
    &[450, 343, 162, 530, 324, 442, 402, 529, 340, 67, 441, 253, 9, 7, 8],
    &[277, 335, 414, 492, 356, 393, 334, 387, 276, 527, 196, 401, 399, 428, 406, 304, 231],
    &[20, 173, 282, 235, 257, 272, 215, 366, 143, 220, 202, 409, 355, 264, 351, 352],
    &[380, 388, 180, 495, 270, 271, 478, 472, 283, 200, 278, 289, 446, 214, 285],
];

#[cfg(test)]
mod test {
    use super::*;
    use crate::data::SpeciesData;
    use crate::move_names::MOVE;

    fn name(id: u16) -> &'static str {
        MOVE[id as usize - 1]
    }

    #[test]
    fn the_machines_have_the_names_of_bulbapedia() {
        assert_eq!(name(MACHINES[0]), "Hone Claws"); // TM01
        assert_eq!(name(MACHINES[5]), "Toxic"); // TM06
        assert_eq!(name(MACHINES[93]), "Secret Power"); // TM94
        assert_eq!(name(MACHINES[99]), "Confide"); // TM100
        assert_eq!(name(MACHINES[100]), "Cut"); // HM01
        assert_eq!(name(MACHINES[106]), "Dive"); // HM07
        assert_eq!(machine_name(5), "TM06");
        assert_eq!(machine_name(100), "HM01");
        assert_eq!(machine_name(106), "HM07");
        assert!(BP_TUTORS.iter().all(|moves| moves.len() <= 32));
    }

    /// With the dump of the game: the TMs, HMs and tutor moves of Bulbasaur are the ones of the Bulbapedia
    /// table of Generation VI.
    #[test]
    fn the_machines_and_tutors_of_bulbasaur_are_the_ones_of_bulbapedia() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let garc = sango_core::Garc::open(&std::path::Path::new(&root).join("a/1/9/5")).unwrap();
        let d = SpeciesData::from_bytes(garc.first_file(1).unwrap()).unwrap();
        let known: Vec<String> = (0..MACHINES.len())
            .filter(|bit| d.tm_hm_moves[bit / 32] >> (bit % 32) & 1 == 1)
            .map(|bit| format!("{} {}", machine_name(bit), name(MACHINES[bit])))
            .collect();
        let expected = [
            "TM06 Toxic", "TM09 Venoshock", "TM10 Hidden Power", "TM11 Sunny Day", "TM16 Light Screen", "TM17 Protect",
            "TM20 Safeguard", "TM21 Frustration", "TM22 Solar Beam", "TM27 Return", "TM32 Double Team", "TM36 Sludge Bomb",
            "TM42 Facade", "TM44 Rest", "TM45 Attract", "TM48 Round", "TM49 Echoed Voice", "TM53 Energy Ball", "TM70 Flash",
            "TM75 Swords Dance", "TM86 Grass Knot", "TM87 Swagger", "TM88 Sleep Talk", "TM90 Substitute", "TM94 Secret Power",
            "TM96 Nature Power", "TM100 Confide", "HM01 Cut", "HM04 Strength", "HM06 Rock Smash",
        ];
        assert_eq!(known, expected);
        // The tutors of Bulbapedia for Bulbasaur: Bind, Giga Drain, Knock Off, Seed Bomb, Snore, Synthesis, Worry Seed
        // (and Grass Pledge from the special tutor). The data also has Sleep Talk, which Bulbapedia lists as a TM.
        let mut tutors: Vec<&str> = Vec::new();
        for (tutor, moves) in BP_TUTORS.iter().enumerate() {
            for (bit, &id) in moves.iter().enumerate() {
                if d.bp_tutor_moves[tutor] >> bit & 1 == 1 {
                    tutors.push(name(id));
                }
            }
        }
        tutors.sort();
        assert_eq!(tutors, ["Bind", "Giga Drain", "Knock Off", "Seed Bomb", "Sleep Talk", "Snore", "Synthesis", "Worry Seed"]);
        // No bit is set above the last move of a tutor.
        for (tutor, moves) in BP_TUTORS.iter().enumerate() {
            assert_eq!(d.bp_tutor_moves[tutor] >> moves.len(), 0, "tutor {tutor}");
        }
    }
}
