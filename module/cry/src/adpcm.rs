//! The 4-bit samples of the game (DSP-ADPCM): 8 bytes hold 14 samples. The first byte of a frame has the predictor (high 4 bits, 0 to 7)
//! and the scale (low 4 bits); the 7 other bytes have two samples each, the high half first. Each sample is predicted from the two
//! samples before it with two coefficients of the predictor (in 11 bits of fraction), and the 4-bit number corrects the prediction.

/// The samples of a frame.
pub const FRAME_SAMPLES: usize = 14;
/// The bytes of a frame.
pub const FRAME_BYTES: usize = 8;
/// The number of predictors.
pub const PREDICTORS: usize = 8;

/// The coefficients of the 8 predictors: two for each.
pub type Coefficients = [i16; 2 * PREDICTORS];

fn clamp16(value: i64) -> i16 {
    value.clamp(i16::MIN as i64, i16::MAX as i64) as i16
}

fn nibble(byte: u8, high: bool) -> i64 {
    let value = if high { byte >> 4 } else { byte & 0xF } as i64;
    if value >= 8 {
        value - 16
    } else {
        value
    }
}

/// The sample after two samples, for a predictor and a scale, when the 4-bit number is `delta`.
fn next_sample(coefficients: &Coefficients, predictor: usize, scale: u32, delta: i64, h1: i16, h2: i16) -> i16 {
    let (c1, c2) = (coefficients[2 * predictor] as i64, coefficients[2 * predictor + 1] as i64);
    clamp16((((delta * (1i64 << scale)) << 11) + 1024 + c1 * h1 as i64 + c2 * h2 as i64) >> 11)
}

/// Decodes `count` samples. `bytes` has the frames one after the other.
pub fn decode(bytes: &[u8], coefficients: &Coefficients, count: usize) -> Vec<i16> {
    let (mut h1, mut h2) = (0i16, 0i16);
    let mut out = Vec::with_capacity(count);
    for frame in bytes.chunks(FRAME_BYTES) {
        if frame.len() < FRAME_BYTES || out.len() >= count {
            break;
        }
        let (predictor, scale) = (((frame[0] >> 4) & 7) as usize, (frame[0] & 0xF) as u32);
        for i in 0..FRAME_SAMPLES {
            if out.len() >= count {
                break;
            }
            let delta = nibble(frame[1 + i / 2], i % 2 == 0);
            let sample = next_sample(coefficients, predictor, scale, delta, h1, h2);
            h2 = h1;
            h1 = sample;
            out.push(sample);
        }
    }
    out
}

/// The result of an encoding: the coefficients and the frames.
pub struct Encoded {
    pub coefficients: Coefficients,
    pub bytes: Vec<u8>,
}

/// The two coefficients that predict the samples of a frame best from the two before them.
fn frame_predictor(samples: &[f64], before: [f64; 2]) -> (f64, f64) {
    let at = |i: isize| -> f64 {
        if i >= 0 {
            samples[i as usize]
        } else {
            before[(-i - 1) as usize]
        }
    };
    let (mut r0, mut r1, mut r2) = (0.0, 0.0, 0.0);
    for i in 0..samples.len() as isize {
        r0 += at(i) * at(i);
        r1 += at(i) * at(i - 1);
        r2 += at(i) * at(i - 2);
    }
    if r0 < 1.0 {
        return (0.0, 0.0);
    }
    // The prediction of the second order.
    let k1 = r1 / r0;
    let error = r0 * (1.0 - k1 * k1);
    if error < 1e-9 {
        return (k1.clamp(-1.9, 1.9), 0.0);
    }
    let k2 = (r2 - k1 * r1) / error;
    ((k1 - k2 * k1).clamp(-1.9, 1.9), k2.clamp(-1.9, 1.9))
}

/// Eight predictors for the samples: the groups of the predictors of the frames.
fn choose_coefficients(samples: &[i16]) -> Coefficients {
    let floats: Vec<f64> = samples.iter().map(|&s| s as f64).collect();
    let mut points: Vec<(f64, f64)> = Vec::new();
    let mut before = [0.0, 0.0];
    for frame in floats.chunks(FRAME_SAMPLES) {
        points.push(frame_predictor(frame, before));
        let n = frame.len();
        before = [frame[n - 1], if n > 1 { frame[n - 2] } else { before[0] }];
    }
    // Eight groups: the first one stays at zero (a frame of noise or of silence is predicted by nothing).
    let mut centers = [(0.0, 0.0); PREDICTORS];
    if !points.is_empty() {
        let mut sorted = points.clone();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for (k, center) in centers.iter_mut().enumerate().skip(1) {
            *center = sorted[(sorted.len() - 1) * k / (PREDICTORS - 1)];
        }
    }
    for _ in 0..12 {
        let mut sums = [(0.0, 0.0, 0usize); PREDICTORS];
        for point in &points {
            let distance = |k: usize| (point.0 - centers[k].0).powi(2) + (point.1 - centers[k].1).powi(2);
            let nearest = (0..PREDICTORS).min_by(|&a, &b| distance(a).partial_cmp(&distance(b)).unwrap()).unwrap();
            sums[nearest].0 += point.0;
            sums[nearest].1 += point.1;
            sums[nearest].2 += 1;
        }
        for k in 1..PREDICTORS {
            if sums[k].2 > 0 {
                centers[k] = (sums[k].0 / sums[k].2 as f64, sums[k].1 / sums[k].2 as f64);
            }
        }
    }
    let mut coefficients = [0i16; 2 * PREDICTORS];
    for (k, center) in centers.iter().enumerate() {
        coefficients[2 * k] = (center.0 * 2048.0).round().clamp(-32768.0, 32767.0) as i16;
        coefficients[2 * k + 1] = (center.1 * 2048.0).round().clamp(-32768.0, 32767.0) as i16;
    }
    coefficients
}

/// Encodes samples: chooses the predictors, then for each frame the predictor and the scale that leave the smallest error.
pub fn encode(samples: &[i16]) -> Encoded {
    let coefficients = choose_coefficients(samples);
    let mut bytes = Vec::with_capacity(samples.len().div_ceil(FRAME_SAMPLES) * FRAME_BYTES);
    let (mut h1, mut h2) = (0i16, 0i16);
    for frame in samples.chunks(FRAME_SAMPLES) {
        let mut best: Option<(i64, [u8; FRAME_BYTES], i16, i16)> = None;
        for predictor in 0..PREDICTORS {
            for scale in 0..=12u32 {
                let (mut a1, mut a2) = (h1, h2);
                let mut error = 0i64;
                let mut out = [0u8; FRAME_BYTES];
                out[0] = ((predictor as u8) << 4) | scale as u8;
                for (i, &target) in frame.iter().enumerate() {
                    // The 4-bit number that brings the prediction nearest to the sample.
                    let base = next_sample(&coefficients, predictor, scale, 0, a1, a2) as i64;
                    let unit = (1i64 << scale).max(1);
                    let wanted = ((target as i64 - base) as f64 / unit as f64).round() as i64;
                    let delta = wanted.clamp(-8, 7);
                    let sample = next_sample(&coefficients, predictor, scale, delta, a1, a2);
                    error += (sample as i64 - target as i64).pow(2);
                    let code = (delta & 0xF) as u8;
                    if i % 2 == 0 {
                        out[1 + i / 2] |= code << 4;
                    } else {
                        out[1 + i / 2] |= code;
                    }
                    a2 = a1;
                    a1 = sample;
                }
                if best.as_ref().is_none_or(|b| error < b.0) {
                    best = Some((error, out, a1, a2));
                }
            }
        }
        let (_, out, a1, a2) = best.unwrap();
        bytes.extend_from_slice(&out);
        (h1, h2) = (a1, a2);
    }
    Encoded { coefficients, bytes }
}

#[cfg(test)]
mod test {
    use super::*;

    fn tone(count: usize, rate: f64) -> Vec<i16> {
        (0..count).map(|i| ((i as f64 / rate * 440.0 * std::f64::consts::TAU).sin() * 12000.0 + (i as f64 / rate * 1250.0 * std::f64::consts::TAU).sin() * 3000.0) as i16).collect()
    }

    fn signal_to_noise(a: &[i16], b: &[i16]) -> f64 {
        let (signal, noise) = a.iter().zip(b).fold((0.0, 0.0), |(s, n), (&x, &y)| (s + (x as f64).powi(2), n + (x as f64 - y as f64).powi(2)));
        10.0 * (signal / noise.max(1.0)).log10()
    }

    #[test]
    fn a_tone_is_encoded_and_decoded_with_little_noise() {
        let samples = tone(10_000, 32728.0);
        let encoded = encode(&samples);
        assert_eq!(encoded.bytes.len(), samples.len().div_ceil(FRAME_SAMPLES) * FRAME_BYTES);
        let decoded = decode(&encoded.bytes, &encoded.coefficients, samples.len());
        assert_eq!(decoded.len(), samples.len());
        let snr = signal_to_noise(&samples, &decoded);
        assert!(snr > 28.0, "the signal is {snr} dB above the noise");
    }

    #[test]
    fn a_silence_stays_a_silence() {
        let samples = vec![0i16; 300];
        let encoded = encode(&samples);
        assert!(decode(&encoded.bytes, &encoded.coefficients, 300).iter().all(|&s| s == 0));
    }
}
