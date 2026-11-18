//! The choreography files of the moves (the archive 34): a header, then the steps one after the other, then an end mark.
//!
//! A step has a header of 14 bytes (the first and the last frame, the group of objects, the condition, the type) and the parameters of its
//! type (see `schema.rs` for the sizes and the meaning of the bytes). The file keeps what the Studio does not know (the header and the bytes
//! after the last step), so a file that is read and written again has the same bytes.

use crate::schema::{self, ParamKind, StepKind, STEP_SIZES};

/// The size of the header of the file.
pub const FILE_HEADER: usize = 16;
/// The size of the header of a step.
pub const STEP_HEADER: usize = 14;
/// Where the last frame of the choreography is in the header of the file.
const LAST_FRAME_AT: usize = 12;

/// A step of a choreography.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub start: i32,
    pub end: i32,
    /// The object that the step makes or moves (0 is the usual group).
    pub group: u16,
    /// When the step plays (a number of the list `Step Condition`; 0 always).
    pub condition: u16,
    pub kind: u16,
    /// The parameters, as many bytes as the type needs.
    pub data: Vec<u8>,
}

/// A choreography: the header of the file, the steps and the bytes after the end mark.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveChoreography {
    header: [u8; FILE_HEADER],
    pub steps: Vec<Step>,
    tail: Vec<u8>,
}

/// A value of a parameter.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i32),
    Bool(bool),
    Float(f32),
    Vec3([f32; 3]),
    Text(String),
    /// The bytes of a parameter that the page does not edit (curves, windows).
    Bytes(Vec<u8>),
}

/// The type of step of a number.
pub fn kind_of(number: u16) -> Option<&'static StepKind> {
    schema::KINDS.iter().find(|kind| kind.number == number)
}

fn int_at(bytes: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

impl MoveChoreography {
    /// Reads a file.
    pub fn parse(bytes: &[u8]) -> Result<MoveChoreography, String> {
        if bytes.len() < FILE_HEADER + 4 {
            return Err("The choreography file is too small.".to_string());
        }
        let mut header = [0u8; FILE_HEADER];
        header.copy_from_slice(&bytes[..FILE_HEADER]);
        let mut steps = Vec::new();
        let mut at = FILE_HEADER;
        loop {
            if at + 4 > bytes.len() {
                return Err("The choreography file has no end mark.".to_string());
            }
            if int_at(bytes, at) == -1 {
                break;
            }
            if at + STEP_HEADER > bytes.len() {
                return Err(format!("A step is cut at the byte {at}."));
            }
            let kind = u16::from_le_bytes([bytes[at + 12], bytes[at + 13]]);
            let size = *STEP_SIZES.get(kind as usize).ok_or_else(|| format!("The type of step {kind} is not known (byte {at})."))? as usize;
            let end = at + STEP_HEADER + size;
            if end > bytes.len() {
                return Err(format!("A step of the type {kind} is cut at the byte {at}."));
            }
            steps.push(Step { start: int_at(bytes, at), end: int_at(bytes, at + 4), group: u16::from_le_bytes([bytes[at + 8], bytes[at + 9]]), condition: u16::from_le_bytes([bytes[at + 10], bytes[at + 11]]), kind, data: bytes[at + STEP_HEADER..end].to_vec() });
            at = end;
        }
        Ok(MoveChoreography { header, steps, tail: bytes[at..].to_vec() })
    }

    /// The bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.header.to_vec();
        for step in &self.steps {
            out.extend_from_slice(&step.start.to_le_bytes());
            out.extend_from_slice(&step.end.to_le_bytes());
            out.extend_from_slice(&step.group.to_le_bytes());
            out.extend_from_slice(&step.condition.to_le_bytes());
            out.extend_from_slice(&step.kind.to_le_bytes());
            out.extend_from_slice(&step.data);
        }
        out.extend_from_slice(&self.tail);
        out
    }

    /// The last frame that the header gives.
    pub fn last_frame(&self) -> i32 {
        int_at(&self.header, LAST_FRAME_AT)
    }

    /// The last frame of the steps: the choreography is this long.
    pub fn length(&self) -> i32 {
        self.steps.iter().map(|step| step.end.max(step.start)).max().unwrap_or(0)
    }

    /// Puts the last frame of the header at the end of the last step when a step goes after it.
    pub fn fit_header(&mut self) {
        let length = self.length();
        if length > self.last_frame() {
            self.header[LAST_FRAME_AT..LAST_FRAME_AT + 4].copy_from_slice(&length.to_le_bytes());
        }
    }

    /// A new step of a type, with the parameters at 0 (a bool at false, a float at 0).
    pub fn new_step(kind: u16, start: i32, end: i32) -> Option<Step> {
        let size = *STEP_SIZES.get(kind as usize)? as usize;
        Some(Step { start, end, group: 0, condition: 0, kind, data: vec![0; size] })
    }
}

impl Step {
    /// The value of a parameter of the type of the step.
    pub fn value(&self, param: &schema::Param) -> Value {
        let bytes = &self.data[param.offset..param.offset + param.size];
        match param.kind {
            ParamKind::Int | ParamKind::Choice(_) | ParamKind::Asset(_) => Value::Int(int_at(bytes, 0)),
            ParamKind::Bool => Value::Bool(int_at(bytes, 0) != 0),
            ParamKind::Float => Value::Float(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
            ParamKind::Vec3 => Value::Vec3([0, 4, 8].map(|k| f32::from_le_bytes([bytes[k], bytes[k + 1], bytes[k + 2], bytes[k + 3]]))),
            ParamKind::Text(_) => Value::Text(String::from_utf8_lossy(&bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())]).to_string()),
            ParamKind::Keys5 | ParamKind::Windows6 => Value::Bytes(bytes.to_vec()),
        }
    }

    /// Writes the value of a parameter.
    pub fn set_value(&mut self, param: &schema::Param, value: &Value) {
        let bytes = &mut self.data[param.offset..param.offset + param.size];
        match value {
            Value::Int(v) => bytes[..4].copy_from_slice(&v.to_le_bytes()),
            Value::Bool(v) => bytes[..4].copy_from_slice(&i32::from(*v).to_le_bytes()),
            Value::Float(v) => bytes[..4].copy_from_slice(&v.to_le_bytes()),
            Value::Vec3(v) => {
                for (k, part) in v.iter().enumerate() {
                    bytes[4 * k..4 * k + 4].copy_from_slice(&part.to_le_bytes());
                }
            }
            Value::Text(text) => {
                bytes.fill(0);
                let used = text.len().min(bytes.len().saturating_sub(1));
                bytes[..used].copy_from_slice(&text.as_bytes()[..used]);
            }
            Value::Bytes(v) => {
                let used = v.len().min(bytes.len());
                bytes[..used].copy_from_slice(&v[..used]);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_small_choreography_is_written_as_it_was_read() {
        let mut bytes = vec![0u8; FILE_HEADER];
        bytes[12..16].copy_from_slice(&60i32.to_le_bytes());
        let kind = schema::KINDS.iter().find(|k| k.name == "Effect Spawn").unwrap();
        let mut step = MoveChoreography::new_step(kind.number, 15, 60).unwrap();
        step.set_value(&kind.params[0], &Value::Int(123));
        let choreography = MoveChoreography { header: bytes.clone().try_into().unwrap(), steps: vec![step], tail: Vec::new() };
        let mut written = choreography.to_bytes();
        written.extend_from_slice(&(-1i32).to_le_bytes());
        let read = MoveChoreography::parse(&written).unwrap();
        assert_eq!(read.steps.len(), 1);
        assert_eq!(read.steps[0].value(&kind.params[0]), Value::Int(123));
        assert_eq!(read.to_bytes(), written);
        assert_eq!(read.length(), 60);
    }

    #[test]
    fn the_schema_sizes_add_up() {
        for kind in schema::KINDS {
            if !kind.params.is_empty() {
                assert_eq!(kind.params.iter().map(|p| p.size).sum::<usize>(), kind.size, "{}", kind.name);
            }
            assert_eq!(STEP_SIZES[kind.number as usize] as usize, kind.size, "{}", kind.name);
        }
    }

    /// With the dump: every choreography of the game is read and written again with the same bytes.
    #[test]
    fn the_choreographies_of_the_game_keep_their_bytes() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = sango_core::Garc::open(&dump.archive(34)).unwrap();
        let mut checked = 0;
        for id in 0..garc.file_count() {
            let Some(bytes) = garc.first_file_decompressed(id) else { continue };
            let choreography = MoveChoreography::parse(&bytes).unwrap_or_else(|e| panic!("file {id}: {e}"));
            assert_eq!(choreography.to_bytes(), bytes, "file {id}");
            checked += 1;
        }
        assert!(checked > 700, "{checked}");
    }
}
