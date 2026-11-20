//! The streams of the game (`.bcstm` files): the musics and the jingles, with 4 bits for each sample (DSP-ADPCM) and a loop.
//!
//! All the numbers are little-endian. A file has a header of 0x40 bytes and three blocks.
//!
//! | Block | Content |
//! | --- | --- |
//! | `INFO` | three references: the stream information, the table of the tracks, the table of the channels |
//! | `SEEK` | for each block of samples and each channel, the two samples before the block (2 bytes each), so that the game can start in the middle |
//! | `DATA` | the samples: the blocks of the channels one after the other (a block of the first channel, a block of the second channel, then the next block) |
//!
//! The stream information: the coding (2 is DSP-ADPCM), the loop flag, the number of channels, the sample rate, the frame where the loop
//! starts, the number of frames (the loop ends there), the number and the sizes of the blocks, and the place of the samples in the
//! block `DATA`. Each channel has 16 coefficients, the first frame header and two history samples, and the same three values for the
//! start of the loop.

use sango_cry::adpcm::{self, Coefficients, FRAME_BYTES, FRAME_SAMPLES};

/// The bytes of a block of samples for each channel in the files that Studio writes (the files of the game use this size).
const BLOCK_BYTES: usize = 0x2000;
/// The samples of a block.
const BLOCK_SAMPLES: usize = BLOCK_BYTES / FRAME_BYTES * FRAME_SAMPLES;

/// A sound of one or more channels, with its loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Music {
    /// The samples each second.
    pub rate: u32,
    /// The samples of each channel (all the channels have the same length).
    pub channels: Vec<Vec<i16>>,
    /// The frame where the loop starts. The loop ends at the end of the sound. `None` for a sound that plays one time.
    pub loop_start: Option<u32>,
}

impl Music {
    /// The number of samples of each channel.
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }

    /// The length in seconds.
    pub fn seconds(&self) -> f32 {
        self.frames() as f32 / self.rate.max(1) as f32
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, String> {
    bytes.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| format!("The file stops at {at:#x}."))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| format!("The file stops at {at:#x}."))
}

fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

/// The layout of a file.
struct Layout {
    /// The offsets and the sizes of the three blocks.
    blocks: [(usize, usize); 3],
    /// The offset of the stream information.
    stream: usize,
    channels: usize,
    rate: u32,
    is_loop: bool,
    loop_start: u32,
    frames: u32,
    block_count: usize,
    block_bytes: usize,
    last_bytes: usize,
    last_padded: usize,
    /// The offset of the first sample in the file.
    samples: usize,
    /// The offsets of the DSP information of each channel.
    dsp: Vec<usize>,
}

fn layout(bytes: &[u8]) -> Result<Layout, String> {
    if bytes.get(..4) != Some(b"CSTM") {
        return Err("This is not a stream file.".into());
    }
    let mut blocks = [(0usize, 0usize); 3];
    let count = u16_at(bytes, 0x10)? as usize;
    for i in 0..count.min(3) {
        let kind = u16_at(bytes, 0x14 + i * 12)?;
        if (0x4000..0x4003).contains(&kind) {
            blocks[(kind - 0x4000) as usize] = (u32_at(bytes, 0x14 + i * 12 + 4)? as usize, u32_at(bytes, 0x14 + i * 12 + 8)? as usize);
        }
    }
    if blocks.iter().any(|b| b.0 == 0) {
        return Err("The stream has no information, seek or data block.".into());
    }
    let info = blocks[0].0 + 8;
    let stream = info + u32_at(bytes, info + 4)? as usize;
    if bytes.get(stream).copied() != Some(2) {
        return Err(format!("The coding {} is not supported: only DSP-ADPCM streams can be read.", bytes.get(stream).copied().unwrap_or(0)));
    }
    let channels = bytes[stream + 2] as usize;
    let table = info + u32_at(bytes, info + 20)? as usize;
    if u32_at(bytes, table)? as usize != channels {
        return Err("The table of the channels does not match.".into());
    }
    let mut dsp = Vec::new();
    for c in 0..channels {
        let entry = table + u32_at(bytes, table + 4 + c * 8 + 4)? as usize;
        dsp.push(entry + u32_at(bytes, entry + 4)? as usize);
    }
    Ok(Layout {
        blocks,
        stream,
        channels,
        rate: u32_at(bytes, stream + 4)?,
        is_loop: bytes[stream + 1] != 0,
        loop_start: u32_at(bytes, stream + 8)?,
        frames: u32_at(bytes, stream + 12)?,
        block_count: u32_at(bytes, stream + 16)? as usize,
        block_bytes: u32_at(bytes, stream + 20)? as usize,
        last_bytes: u32_at(bytes, stream + 28)? as usize,
        last_padded: u32_at(bytes, stream + 36)? as usize,
        samples: blocks[2].0 + 8 + u32_at(bytes, stream + 52)? as usize,
        dsp,
    })
}

/// Reads a stream file: the samples of all the channels, the rate and the loop.
pub fn decode(bytes: &[u8]) -> Result<Music, String> {
    let l = layout(bytes)?;
    if l.channels == 0 || l.block_count == 0 {
        return Err("The stream is empty.".into());
    }
    let mut channels = Vec::new();
    for c in 0..l.channels {
        let mut coefficients: Coefficients = [0; 16];
        for (i, value) in coefficients.iter_mut().enumerate() {
            *value = u16_at(bytes, l.dsp[c] + i * 2)? as i16;
        }
        let mut data = Vec::new();
        let mut at = l.samples;
        for block in 0..l.block_count {
            let (size, used) = if block + 1 == l.block_count { (l.last_padded, l.last_bytes) } else { (l.block_bytes, l.block_bytes) };
            let from = at + c * size;
            data.extend_from_slice(bytes.get(from..from + used).ok_or("The samples of the stream are cut.")?);
            at += size * l.channels;
        }
        channels.push(adpcm::decode(&data, &coefficients, l.frames as usize));
    }
    Ok(Music { rate: l.rate, channels, loop_start: l.is_loop.then_some(l.loop_start) })
}

/// Makes a stream file from a sound. `template` is a file of the game: the new file has its layout and the same number of channels, so
/// the sound must have the same number of channels (see [`with_channels`]).
pub fn encode(template: &[u8], music: &Music) -> Result<Vec<u8>, String> {
    let l = layout(template)?;
    if music.channels.len() != l.channels {
        return Err(format!("The stream has {} channels and the sound has {}.", l.channels, music.channels.len()));
    }
    let frames = music.frames();
    if frames == 0 || music.channels.iter().any(|c| c.len() != frames) {
        return Err("The sound is empty.".into());
    }
    let loop_start = music.loop_start.map(|s| (s as usize / FRAME_SAMPLES * FRAME_SAMPLES).min(frames.saturating_sub(FRAME_SAMPLES)));

    // The 4-bit samples of each channel.
    let encoded: Vec<adpcm::Encoded> = music.channels.iter().map(|samples| adpcm::encode(samples)).collect();
    // What the game will hear: the history at the start of the loop and of each block comes from it.
    let decoded: Vec<Vec<i16>> = encoded.iter().map(|e| adpcm::decode(&e.bytes, &e.coefficients, frames)).collect();
    let per_channel = encoded[0].bytes.len();
    let block_count = per_channel.div_ceil(BLOCK_BYTES);
    let last_bytes = per_channel - (block_count - 1) * BLOCK_BYTES;
    let last_padded = last_bytes.div_ceil(0x20) * 0x20;
    let last_samples = frames - (block_count - 1) * BLOCK_SAMPLES;

    // The information block comes from the template, with the new numbers.
    let mut info = template[l.blocks[0].0..l.blocks[0].0 + l.blocks[0].1].to_vec();
    let stream = l.stream - l.blocks[0].0;
    info[stream + 1] = u8::from(loop_start.is_some());
    put_u32(&mut info, stream + 4, music.rate);
    put_u32(&mut info, stream + 8, loop_start.unwrap_or(0) as u32);
    put_u32(&mut info, stream + 12, frames as u32);
    put_u32(&mut info, stream + 16, block_count as u32);
    put_u32(&mut info, stream + 20, BLOCK_BYTES as u32);
    put_u32(&mut info, stream + 24, BLOCK_SAMPLES as u32);
    put_u32(&mut info, stream + 28, last_bytes as u32);
    put_u32(&mut info, stream + 32, last_samples as u32);
    put_u32(&mut info, stream + 36, last_padded as u32);
    put_u32(&mut info, stream + 44, BLOCK_SAMPLES as u32);
    for (c, channel) in encoded.iter().enumerate() {
        let at = l.dsp[c] - l.blocks[0].0;
        for (i, coefficient) in channel.coefficients.iter().enumerate() {
            put_u16(&mut info, at + i * 2, *coefficient as u16);
        }
        // The first frame, and the history before it.
        put_u16(&mut info, at + 32, u16::from(channel.bytes[0]));
        put_u16(&mut info, at + 34, 0);
        put_u16(&mut info, at + 36, 0);
        // The start of the loop: the header of its frame and the two samples before it.
        let (loop_ps, y1, y2) = match loop_start {
            Some(start) => {
                let history = |back: usize| if start >= back { decoded[c][start - back] as u16 } else { 0 };
                (u16::from(channel.bytes[start / FRAME_SAMPLES * FRAME_BYTES]), history(1), history(2))
            }
            None => (0, 0, 0),
        };
        put_u16(&mut info, at + 38, loop_ps);
        put_u16(&mut info, at + 40, y1);
        put_u16(&mut info, at + 42, y2);
    }

    // The seek block: the two samples before each block, for each channel.
    let mut seek_body = Vec::new();
    for block in 0..block_count {
        for channel in &decoded {
            let start = block * BLOCK_SAMPLES;
            let history = |back: usize| if start >= back { channel[start - back] } else { 0 };
            seek_body.extend_from_slice(&(history(1) as u16).to_le_bytes());
            seek_body.extend_from_slice(&(history(2) as u16).to_le_bytes());
        }
    }
    let mut seek = b"SEEK".to_vec();
    seek.extend_from_slice(&0u32.to_le_bytes());
    seek.extend_from_slice(&seek_body);
    seek.resize(seek.len().div_ceil(0x20) * 0x20, 0);
    let seek_size = seek.len() as u32;
    seek[4..8].copy_from_slice(&seek_size.to_le_bytes());

    // The data block.
    let sample_offset = u32_at(template, l.stream + 52)? as usize;
    let mut data = b"DATA".to_vec();
    data.extend_from_slice(&0u32.to_le_bytes());
    data.resize(8 + sample_offset, 0);
    for block in 0..block_count {
        let size = if block + 1 == block_count { last_padded } else { BLOCK_BYTES };
        for channel in &encoded {
            let from = block * BLOCK_BYTES;
            let to = (from + BLOCK_BYTES).min(channel.bytes.len());
            let mut part = channel.bytes[from..to].to_vec();
            part.resize(size, 0);
            data.extend_from_slice(&part);
        }
    }
    data.resize(data.len().div_ceil(0x20) * 0x20, 0);
    let data_size = data.len() as u32;
    data[4..8].copy_from_slice(&data_size.to_le_bytes());

    // The file: the header of the template with the new sizes, then the three blocks.
    let mut out = template[..0x40.min(l.blocks[0].0)].to_vec();
    let info_at = out.len();
    let seek_at = info_at + info.len();
    let data_at = seek_at + seek.len();
    for (i, (offset, size)) in [(info_at, info.len()), (seek_at, seek.len()), (data_at, data.len())].into_iter().enumerate() {
        put_u32(&mut out, 0x14 + i * 12 + 4, offset as u32);
        put_u32(&mut out, 0x14 + i * 12 + 8, size as u32);
    }
    out.extend_from_slice(&info);
    out.extend_from_slice(&seek);
    out.extend_from_slice(&data);
    let total = out.len() as u32;
    put_u32(&mut out, 0x0C, total);
    Ok(out)
}

/// The number of channels of a stream file.
pub fn channel_count(bytes: &[u8]) -> Result<usize, String> {
    Ok(layout(bytes)?.channels)
}

/// Gives a sound the number of channels that a stream has: a mono sound is copied, a stereo sound is mixed down to mono.
pub fn with_channels(music: &Music, count: usize) -> Music {
    let mut channels = music.channels.clone();
    if channels.len() > count {
        if count == 1 {
            let frames = music.frames();
            channels = vec![(0..frames).map(|i| (music.channels.iter().map(|c| c[i] as i32).sum::<i32>() / music.channels.len() as i32) as i16).collect()];
        } else {
            channels.truncate(count);
        }
    }
    while channels.len() < count {
        let last = channels.last().cloned().unwrap_or_default();
        channels.push(last);
    }
    Music { channels, ..music.clone() }
}
