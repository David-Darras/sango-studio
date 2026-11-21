//! The Layout page: the menus of the game.
//!
//! The screens of the game (the Bag, the party, the Summary, the PSS...) are layouts. A layout archive file is a bundle (see [`bundle`])
//! that holds a layout (see [`bclyt`]) and its images and animations in a DARC archive (see [`darc`]). The page draws a layout, and
//! moves, resizes, hides, copies and deletes its panes, and replaces its images.

pub mod bclan;
pub mod bclyt;
pub mod bundle;
pub mod darc;
mod page;
pub mod tex;

pub use page::LayoutPage;

#[cfg(test)]
mod tests {
    use super::*;
    use sango_core::{lz, Dump, Garc};

    /// Reads and writes again every layout of the dump: the bytes must be the same.
    #[test]
    fn every_layout_of_the_dump_is_written_back_the_same() {
        let Ok(path) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&path).unwrap();
        let (mut bundles, mut layouts, mut bad) = (0, 0, Vec::new());
        for number in 0..300 {
            let path = dump.archive(number);
            if !path.is_file() || std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > 60_000_000 {
                continue;
            }
            let Ok(garc) = Garc::open(&path) else { continue };
            for id in 0..garc.file_count() {
                for language in garc.languages(id) {
                    let Some(file) = garc.file(id, language) else { continue };
                    let Ok(bundle) = bundle::Bundle::parse(file) else { continue };
                    bundles += 1;
                    let raw = if lz::is_compressed(file) { lz::decompress(file).unwrap() } else { file.to_vec() };
                    if bundle.to_raw() != raw {
                        bad.push(format!("a/{}/{}/{} file {id}: the bundle changed", number / 100, number / 10 % 10, number % 10));
                        continue;
                    }
                    for f in &bundle.darc.files {
                        if f.path.ends_with(".bclyt") {
                            layouts += 1;
                            let parsed = bclyt::Bclyt::parse(&f.data).unwrap();
                            if parsed.to_bytes() != f.data {
                                bad.push(format!("archive {number} file {id}: {} changed", f.path));
                            }
                        }
                    }
                }
            }
        }
        println!("{bundles} bundles, {layouts} layouts");
        assert!(bad.is_empty(), "{} files changed:\n{}", bad.len(), bad.iter().take(10).cloned().collect::<Vec<_>>().join("\n"));
        assert!(bundles > 300 && layouts > 380);
    }
}

#[cfg(test)]
mod edit {
    use super::*;
    use sango_core::{Dump, Garc};

    /// Copies, adds and deletes panes, saves, and reads the result again.
    #[test]
    fn edited_layout_is_still_valid() {
        let Ok(path) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&path).unwrap();
        let garc = Garc::open(&dump.archive(61)).unwrap();
        let mut bundle = bundle::Bundle::parse(garc.file(0, 0).unwrap()).unwrap();
        let mut layout = bclyt::Bclyt::parse(&bundle.darc.find("bag_lower.bclyt").unwrap().data).unwrap();
        let before = layout.nodes().len();
        let panes_before = bundle.pane_names().len();
        // A picture that has no children: a copy adds one pane.
        let picture = layout.nodes().into_iter().find(|n| n.kind == bclyt::Kind::Picture && layout.subtree(n.block).len() == 1).unwrap().block;
        let (copy, names) = layout.duplicate(picture);
        assert_eq!(names.len(), 1);
        bundle.add_pane_name(&names[0]);
        assert_eq!(layout.nodes().len(), before + 1);
        let (empty, _) = layout.add_empty(copy);
        layout.set_position(empty, [10.0, 20.0, 0.0]);
        assert_eq!(layout.nodes().len(), before + 2);
        // The copy of a pane that has children copies the children too.
        let parent = layout.nodes().into_iter().find(|n| layout.subtree(n.block).len() > 3).unwrap().block;
        let count = layout.subtree(parent).len();
        let (_, names) = layout.duplicate(parent);
        assert!(names.len() >= 2, "the children are copied");
        assert_eq!(layout.nodes().len(), before + 2 + names.len());
        layout.delete(layout.nodes().last().unwrap().block);
        let _ = count;
        // Save and read again.
        bundle.darc.replace("bag_lower.bclyt", layout.to_bytes());
        let again = bundle::Bundle::parse(&bundle.to_bytes()).unwrap();
        let layout2 = bclyt::Bclyt::parse(&again.darc.find("bag_lower.bclyt").unwrap().data).unwrap();
        assert!(layout2.to_bytes() == layout.to_bytes());
        assert_eq!(again.pane_names().len(), panes_before + 1);
        // The other files did not change.
        assert_eq!(again.darc.find("back.bclim").map(|f| f.data.len()), bundle.darc.find("back.bclim").map(|f| f.data.len()));
    }
}

#[cfg(test)]
mod inspect {
    use super::*;
    use sango_core::{Dump, Garc};

    /// Saves the images of a bundle as PNG files (to look at them).
    #[test]
    fn dump_images() {
        let (Ok(path), Ok(out)) = (std::env::var("SANGO_DUMP"), std::env::var("LAYOUT_PREVIEW")) else { return };
        let dump = Dump::open(&path).unwrap();
        let garc = Garc::open(&dump.archive(103)).unwrap();
        let b = bundle::Bundle::parse(garc.file(0, 0).unwrap()).unwrap();
        for name in ["wave_pattern.bclim", "line_pattern.bclim", "box_bg2.bclim", "box_bg.bclim", "bar2.bclim", "box1.bclim", "text_base.bclim"] {
            let f = b.darc.find(name).unwrap();
            let p = tex::decode(&f.data).unwrap();
            // On a grey background, to see the transparency.
            let mut img = image::RgbaImage::from_pixel(p.width as u32, p.height as u32, image::Rgba([90, 0, 90, 255]));
            for y in 0..p.height {
                for x in 0..p.width {
                    let s = &p.rgba[(y * p.width + x) * 4..][..4];
                    let a = s[3] as f32 / 255.0;
                    let d = img.get_pixel_mut(x as u32, y as u32);
                    for k in 0..3 {
                        d.0[k] = (d.0[k] as f32 * (1.0 - a) + s[k] as f32 * a) as u8;
                    }
                }
            }
            img.save(format!("{out}/img_{}.png", name.trim_end_matches(".bclim"))).unwrap();
        }
    }
}


#[cfg(test)]
mod animations {
    use super::*;
    use sango_core::{Dump, Garc};

    /// Reads every animation of the dump, and checks the colors of the Bag.
    #[test]
    fn every_animation_is_read() {
        let Ok(path) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&path).unwrap();
        let (mut count, mut bad, mut green_found) = (0, Vec::new(), false);
        for number in 0..300 {
            let path = dump.archive(number);
            if !path.is_file() || std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > 60_000_000 {
                continue;
            }
            let Ok(garc) = Garc::open(&path) else { continue };
            for id in 0..garc.file_count() {
                for language in garc.languages(id) {
                    let Some(file) = garc.file(id, language) else { continue };
                    let Ok(bundle) = bundle::Bundle::parse(file) else { continue };
                    for f in bundle.darc.files.iter().filter(|f| f.path.ends_with(".bclan")) {
                        count += 1;
                        match bclan::Bclan::parse(&f.data) {
                            Ok(a) => assert!(a.to_bytes() == f.data),
                            Err(e) => bad.push(format!("a/{number} file {id} {}: {e}", f.path)),
                        }
                        if number == 61 && f.path.contains("/anim/") {
                            // The animations of the background of the top screen of the Bag give a color to the panel: one of them is green.
                            let a = bclan::Bclan::parse(&f.data).unwrap();
                            let values: Vec<f32> = a.tracks.iter().flat_map(|c| c.channels.iter()).filter(|t| t.curve == 2 && !t.keys.is_empty()).map(|t| a.value(&t.keys[0], t.curve)).collect();
                            if values == vec![60.0, 210.0, 24.0] {
                                green_found = true;
                            }
                        }
                    }
                }
            }
        }
        println!("{count} animations");
        assert!(bad.is_empty(), "{} failed:
{}", bad.len(), bad.iter().take(10).cloned().collect::<Vec<_>>().join("
"));
        assert!(count > 2900);
        assert!(green_found, "the green animation of the Bag is read");
    }
}
