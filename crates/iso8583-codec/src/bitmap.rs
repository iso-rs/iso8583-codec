//! ISO 8583 primary and secondary bitmaps.

use alloc::vec::Vec;

use crate::error::Error;

/// A 64-bit primary bitmap with an optional 64-bit secondary bitmap.
///
/// ISO 8583 numbers fields from 1. Field 1 is the secondary-bitmap indicator,
/// field 65 is the tertiary-bitmap indicator, and the remaining bits map to
/// data fields. This type exposes that mapping directly:
///
/// ```
/// use iso8583_codec::Bitmap;
///
/// let mut bitmap = Bitmap::new();
/// bitmap.set(2).unwrap();
/// bitmap.set(70).unwrap();
/// assert!(bitmap.is_set(2));
/// assert!(bitmap.is_set(70));
/// assert!(bitmap.has_secondary());
/// ```
///
/// A bitmap is encoded most-significant-bit first: field 1 is `0x80` in the
/// first byte, field 2 is `0x40`, and so on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bitmap {
    primary: [u8; 8],
    secondary: Option<[u8; 8]>,
}

impl Bitmap {
    /// Creates an empty bitmap with no secondary bitmap.
    pub const fn new() -> Self {
        Self {
            primary: [0; 8],
            secondary: None,
        }
    }

    /// Decodes a bitmap from exactly 8 or 16 bytes.
    ///
    /// The secondary-bitmap indicator must agree with the input length:
    /// an 8-byte bitmap must not set bit 1, and a 16-byte bitmap must set it.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        match bytes.len() {
            8 => {
                let mut primary = [0u8; 8];
                primary.copy_from_slice(bytes);

                if primary[0] & 0x80 != 0 {
                    return Err(Error::MissingSecondaryBitmap);
                }

                Ok(Self {
                    primary,
                    secondary: None,
                })
            }
            16 => {
                let mut primary = [0u8; 8];
                primary.copy_from_slice(&bytes[..8]);

                let mut secondary = [0u8; 8];
                secondary.copy_from_slice(&bytes[8..]);

                if primary[0] & 0x80 == 0 {
                    return Err(Error::UnexpectedSecondaryBitmap);
                }

                if secondary[0] & 0x80 != 0 {
                    return Err(Error::UnsupportedTertiaryBitmap);
                }

                Ok(Self {
                    primary,
                    secondary: Some(secondary),
                })
            }
            actual => Err(Error::InvalidBitmapLength { actual }),
        }
    }

    /// Encodes the bitmap as 8 bytes, or 16 when a secondary bitmap is present.
    pub fn to_vec(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(if self.secondary.is_some() { 16 } else { 8 });
        out.extend_from_slice(&self.primary);

        if let Some(secondary) = &self.secondary {
            out.extend_from_slice(secondary);
        }

        out
    }

    /// Returns `true` when a secondary bitmap is present.
    pub const fn has_secondary(&self) -> bool {
        self.secondary.is_some()
    }

    /// Returns `true` when the given field bit is set.
    ///
    /// Returns `false` for field numbers outside `1..=128`.
    pub fn is_set(&self, field: u8) -> bool {
        match field {
            1 => self.primary[0] & 0x80 != 0,
            2..=64 => test_bit(&self.primary, field),
            65 => self
                .secondary
                .as_ref()
                .is_some_and(|secondary| secondary[0] & 0x80 != 0),
            66..=128 => self
                .secondary
                .as_ref()
                .is_some_and(|secondary| test_bit(secondary, field - 64)),
            _ => false,
        }
    }

    /// Sets the bit for a data field.
    ///
    /// Setting a field above 64 allocates the secondary bitmap and sets the
    /// primary bitmap's bit 1 automatically.
    pub fn set(&mut self, field: u8) -> Result<(), Error> {
        validate_data_field(field)?;

        if field <= 64 {
            set_bit(&mut self.primary, field);
        } else {
            let secondary = self.secondary.get_or_insert([0u8; 8]);
            set_bit(secondary, field - 64);
            self.primary[0] |= 0x80;
        }

        Ok(())
    }

    /// Clears the bit for a data field.
    ///
    /// Clearing the last field in the secondary bitmap also removes the
    /// secondary bitmap and its indicator.
    pub fn clear(&mut self, field: u8) -> Result<(), Error> {
        validate_data_field(field)?;

        if field <= 64 {
            clear_bit(&mut self.primary, field);
        } else if let Some(secondary) = &mut self.secondary {
            clear_bit(secondary, field - 64);

            if secondary.iter().all(|byte| *byte == 0) {
                self.secondary = None;
                self.primary[0] &= 0x7F;
            }
        }

        Ok(())
    }

    /// Iterates over the data fields whose bits are set, in ascending order.
    ///
    /// Field 1 and field 65 are bitmap indicators and are not yielded.
    pub fn iter(&self) -> impl Iterator<Item = u8> + '_ {
        (2..=128).filter(move |field| *field != 65 && self.is_set(*field))
    }
}

impl Default for Bitmap {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_data_field(field: u8) -> Result<(), Error> {
    if !(2..=128).contains(&field) {
        return Err(Error::InvalidFieldNumber { field });
    }

    if field == 65 {
        return Err(Error::ReservedBitmapField { field });
    }

    Ok(())
}

fn set_bit(bytes: &mut [u8; 8], field: u8) {
    let index = ((field - 1) / 8) as usize;
    let mask = 0x80 >> ((field - 1) % 8);
    bytes[index] |= mask;
}

fn clear_bit(bytes: &mut [u8; 8], field: u8) {
    let index = ((field - 1) / 8) as usize;
    let mask = 0x80 >> ((field - 1) % 8);
    bytes[index] &= !mask;
}

fn test_bit(bytes: &[u8; 8], field: u8) -> bool {
    let index = ((field - 1) / 8) as usize;
    let mask = 0x80 >> ((field - 1) % 8);
    bytes[index] & mask != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_field_numbers_to_bits() {
        let mut bitmap = Bitmap::new();
        bitmap.set(2).unwrap();
        bitmap.set(64).unwrap();

        assert_eq!(bitmap.to_vec(), vec![0x40, 0, 0, 0, 0, 0, 0, 0x01]);
    }

    #[test]
    fn allocates_secondary_bitmap() {
        let mut bitmap = Bitmap::new();
        bitmap.set(70).unwrap();

        assert!(bitmap.has_secondary());
        assert!(bitmap.is_set(1));
        assert!(bitmap.is_set(70));
        assert_eq!(bitmap.to_vec().len(), 16);
    }

    #[test]
    fn clears_secondary_bitmap_when_empty() {
        let mut bitmap = Bitmap::new();
        bitmap.set(70).unwrap();
        bitmap.clear(70).unwrap();

        assert!(!bitmap.has_secondary());
        assert!(!bitmap.is_set(1));
        assert_eq!(bitmap.to_vec().len(), 8);
    }

    #[test]
    fn decodes_and_encodes() {
        let bytes = [0x72, 0x38, 0x44, 0x81, 0x00, 0xE0, 0x80, 0x00];
        let bitmap = Bitmap::from_slice(&bytes).unwrap();

        assert_eq!(
            bitmap.iter().collect::<Vec<_>>(),
            vec![2, 3, 4, 7, 11, 12, 13, 18, 22, 25, 32, 41, 42, 43, 49]
        );
        assert_eq!(bitmap.to_vec(), bytes);
    }

    #[test]
    fn rejects_missing_secondary_bitmap() {
        let mut bytes = [0u8; 8];
        bytes[0] = 0x80;
        assert_eq!(
            Bitmap::from_slice(&bytes).unwrap_err(),
            Error::MissingSecondaryBitmap
        );
    }

    #[test]
    fn rejects_unexpected_secondary_bitmap() {
        assert_eq!(
            Bitmap::from_slice(&[0u8; 16]).unwrap_err(),
            Error::UnexpectedSecondaryBitmap
        );
    }

    #[test]
    fn rejects_tertiary_bitmap() {
        let mut bytes = [0u8; 16];
        bytes[0] = 0x80;
        bytes[8] = 0x80;

        assert_eq!(
            Bitmap::from_slice(&bytes).unwrap_err(),
            Error::UnsupportedTertiaryBitmap
        );
    }

    #[test]
    fn rejects_reserved_fields() {
        let mut bitmap = Bitmap::new();
        assert_eq!(
            bitmap.set(1).unwrap_err(),
            Error::InvalidFieldNumber { field: 1 }
        );
        assert_eq!(
            bitmap.set(65).unwrap_err(),
            Error::ReservedBitmapField { field: 65 }
        );
        assert_eq!(
            bitmap.set(129).unwrap_err(),
            Error::InvalidFieldNumber { field: 129 }
        );
    }

    #[test]
    fn ignores_out_of_range_queries() {
        let bitmap = Bitmap::new();
        assert!(!bitmap.is_set(0));
        assert!(!bitmap.is_set(129));
    }
}
