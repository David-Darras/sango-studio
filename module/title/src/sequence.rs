//! A sequence file: a header of 16 bytes (`SESD`, the version, a 0, the last frame), then the steps one after the other, then the
//! mark -1.
//!
//! A step has 14 bytes (the first frame, the last frame, the group of objects, the condition, the type) and the values of its type
//! (see `commands.rs` for the sizes). The game reads the steps in the order of their first frame.

use crate::commands::SIZES;

const HEADER: usize = 16;
const STEP_HEADER: usize = 14;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub start: i32,
    pub end: i32,
    /// The steps of a group act on the same object.
    pub group: u16,
    /// When the step plays (see `commands::CONDITIONS`).
    pub condition: u16,
    pub kind: u16,
    pub data: Vec<u8>,
}

impl Step {
    /// A new step of a type, with its values at 0. `None` for a type that does not exist.
    pub fn new(kind: u16, start: i32, end: i32, group: u16) -> Option<Step> {
        Some(Step { start, end, group, condition: 0, kind, data: vec![0; *SIZES.get(kind as usize)?] })
    }

    /// The whole number at a place of the values (the place counts in values of 4 bytes).
    pub fn int(&self, place: usize) -> i32 {
        self.data.get(4 * place..4 * place + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))
    }

    pub fn set_int(&mut self, place: usize, value: i32) {
        if let Some(bytes) = self.data.get_mut(4 * place..4 * place + 4) {
            bytes.copy_from_slice(&value.to_le_bytes());
        }
    }

    pub fn float(&self, place: usize) -> f32 {
        f32::from_bits(self.int(place) as u32)
    }

    pub fn set_float(&mut self, place: usize, value: f32) {
        self.set_int(place, value.to_bits() as i32);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sequence {
    header: [u8; HEADER],
    pub steps: Vec<Step>,
    /// The end mark and what comes after it.
    tail: Vec<u8>,
}

fn int_at(bytes: &[u8], at: usize) -> Option<i32> {
    bytes.get(at..at + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()))
}

impl Sequence {
    pub fn parse(bytes: &[u8]) -> Result<Sequence, String> {
        if bytes.len() < HEADER + 4 || &bytes[..4] != b"SESD" {
            return Err("This is not a sequence file.".to_string());
        }
        let header: [u8; HEADER] = bytes[..HEADER].try_into().unwrap();
        let mut steps = Vec::new();
        let mut at = HEADER;
        while int_at(bytes, at).ok_or("The sequence file has no end mark.")? != -1 {
            let cut = || format!("A step is cut at the byte {at}.");
            let head = bytes.get(at..at + STEP_HEADER).ok_or_else(cut)?;
            let half = |place: usize| u16::from_le_bytes([head[place], head[place + 1]]);
            let kind = half(12);
            let size = *SIZES.get(kind as usize).ok_or_else(|| format!("The type of step {kind} is not known (byte {at})."))?;
            let data = bytes.get(at + STEP_HEADER..at + STEP_HEADER + size).ok_or_else(cut)?.to_vec();
            steps.push(Step { start: int_at(head, 0).unwrap(), end: int_at(head, 4).unwrap(), group: half(8), condition: half(10), kind, data });
            at += STEP_HEADER + size;
        }
        Ok(Sequence { header, steps, tail: bytes[at..].to_vec() })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.header.to_vec();
        for step in &self.steps {
            out.extend_from_slice(&step.start.to_le_bytes());
            out.extend_from_slice(&step.end.to_le_bytes());
            for half in [step.group, step.condition, step.kind] {
                out.extend_from_slice(&half.to_le_bytes());
            }
            out.extend_from_slice(&step.data);
        }
        out.extend_from_slice(&self.tail);
        out
    }

    /// The last frame of the sequence, as its header gives it.
    pub fn last_frame(&self) -> i32 {
        int_at(&self.header, 12).unwrap_or(0)
    }

    /// The last frame that the page shows: the header or the step that ends last.
    pub fn length(&self) -> i32 {
        self.steps.iter().map(|step| step.end.max(step.start)).chain([self.last_frame()]).max().unwrap_or(0)
    }

    /// Puts the steps in the order of their first frame (the ones of the same frame keep their order), and the header at the end of
    /// the last step when a step goes after it. Gives the new place of the step that was at `follow`.
    pub fn settle(&mut self, follow: Option<usize>) -> Option<usize> {
        let mut order: Vec<usize> = (0..self.steps.len()).collect();
        order.sort_by_key(|&place| self.steps[place].start);
        if order.iter().enumerate().any(|(new, &old)| new != old) {
            self.steps = order.iter().map(|&place| self.steps[place].clone()).collect();
        }
        let length = self.length();
        if length > self.last_frame() {
            self.header[12..16].copy_from_slice(&length.to_le_bytes());
        }
        follow.and_then(|old| order.iter().position(|&place| place == old))
    }

    /// Adds a step after the ones that start at the same frame or before. Gives its place.
    pub fn insert(&mut self, step: Step) -> usize {
        let place = self.steps.iter().position(|other| other.start > step.start).unwrap_or(self.steps.len());
        self.steps.insert(place, step);
        place
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_small_sequence_is_written_as_it_was_read() {
        let mut bytes = b"SESD\x02\0\0\0\0\0\0\0".to_vec();
        bytes.extend(100i32.to_le_bytes());
        bytes.extend((-1i32).to_le_bytes());
        let mut sequence = Sequence::parse(&bytes).unwrap();
        assert_eq!(sequence.to_bytes(), bytes);
        let mut late = Step::new(102, 50, 200, 1).unwrap();
        late.set_int(0, 2);
        assert_eq!(sequence.insert(late.clone()), 0);
        assert_eq!(sequence.insert(Step::new(14, 10, 10, 1).unwrap()), 0);
        sequence.steps[0].start = 60;
        assert_eq!(sequence.settle(Some(0)), Some(1));
        assert_eq!(sequence.last_frame(), 200);
        assert_eq!(Sequence::parse(&sequence.to_bytes()).unwrap(), sequence);
        assert_eq!(sequence.steps[0], late);
    }
}
