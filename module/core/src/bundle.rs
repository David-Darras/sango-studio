//! The bundle format: several resources in one file.
//!
//! Many files of the game use it. It is the same layout as the `Bundle` of the plugin.
//!
//! | Offset | Size | Name | Meaning |
//! | --- | --- | --- | --- |
//! | 0 | 2 | signature | Two letters. They are not checked. |
//! | 2 | 2 | resource count | The number of resources. |
//! | 4 | 4 × (count + 1) | offsets | The start of each resource, from the start of the file. The last one is the end of the file. |
//!
//! All the numbers are little-endian.

/// A bundle: the bytes of the file, with the places of its resources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bundle {
    bytes: Vec<u8>,
    /// The start of each resource, and the end of the last one.
    offsets: Vec<usize>,
}

impl Bundle {
    /// Reads a bundle. Every resource must be inside the file.
    pub fn parse(bytes: Vec<u8>) -> Result<Bundle, String> {
        if bytes.len() < 4 {
            return Err("The bundle is too short.".into());
        }
        let count = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
        let table_end = 4 + 4 * (count + 1);
        if bytes.len() < table_end {
            return Err(format!("The bundle has {count} resources but its table is cut."));
        }
        let offsets: Vec<usize> = (0..=count)
            .map(|i| u32::from_le_bytes(bytes[4 + 4 * i..8 + 4 * i].try_into().unwrap()) as usize)
            .collect();
        if offsets[0] < table_end || offsets.windows(2).any(|w| w[0] > w[1]) || offsets[count] > bytes.len() {
            return Err("The offsets of the bundle are wrong.".into());
        }
        Ok(Bundle { bytes, offsets })
    }

    /// The number of resources.
    pub fn resource_count(&self) -> usize {
        self.offsets.len() - 1
    }

    /// The bytes of a resource.
    pub fn resource(&self, index: usize) -> Option<&[u8]> {
        (index < self.resource_count()).then(|| &self.bytes[self.offsets[index]..self.offsets[index + 1]])
    }

    /// The bytes of a resource, to change them. The size of the resource cannot change.
    pub fn resource_mut(&mut self, index: usize) -> Option<&mut [u8]> {
        (index < self.resource_count()).then(|| {
            let (start, end) = (self.offsets[index], self.offsets[index + 1]);
            &mut self.bytes[start..end]
        })
    }

    /// The bytes of the whole file.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn sample() -> Vec<u8> {
        // Two resources: 2 bytes and 3 bytes, after a table of 3 offsets (12 bytes) and a header of 4 bytes.
        let mut b = vec![b'P', b'C', 2, 0];
        for offset in [16u32, 18, 21] {
            b.extend(offset.to_le_bytes());
        }
        b.extend([1, 2, 3, 4, 5]);
        b
    }

    #[test]
    fn the_resources_are_cut_at_their_offsets() {
        let bundle = Bundle::parse(sample()).unwrap();
        assert_eq!(bundle.resource_count(), 2);
        assert_eq!(bundle.resource(0), Some(&[1u8, 2][..]));
        assert_eq!(bundle.resource(1), Some(&[3u8, 4, 5][..]));
        assert_eq!(bundle.resource(2), None);
    }

    #[test]
    fn a_change_keeps_the_layout() {
        let mut bundle = Bundle::parse(sample()).unwrap();
        bundle.resource_mut(1).unwrap()[0] = 9;
        assert_eq!(bundle.bytes()[18], 9);
        assert_eq!(bundle.bytes().len(), sample().len());
    }

    #[test]
    fn wrong_bundles_are_refused() {
        assert!(Bundle::parse(vec![0, 0]).is_err());
        let mut cut = sample();
        cut.truncate(20);
        assert!(Bundle::parse(cut).is_err());
        let mut backward = sample();
        backward[8..12].copy_from_slice(&30u32.to_le_bytes());
        assert!(Bundle::parse(backward).is_err());
    }
}
