//! Field values.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::error::Error;

/// The value of an ISO 8583 data field.
///
/// Numeric fields are stored as digit strings rather than integers so that
/// fixed-width values such as `000000010000` keep their leading zeros. Binary
/// fields keep their raw bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldValue {
    /// Printable ASCII text (`AN`, `ANS`, `Z` and similar fields).
    Text(String),
    /// ASCII digits (`N` fields, including BCD-encoded fields).
    Digits(String),
    /// Raw bytes (`B` fields such as PIN data or a MAC).
    Binary(Vec<u8>),
}

impl FieldValue {
    /// Creates a text value, rejecting bytes outside printable ASCII.
    pub fn text(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();

        for &byte in value.as_bytes() {
            if !(0x20..=0x7E).contains(&byte) {
                return Err(Error::InvalidTextByte { field: None, byte });
            }
        }

        Ok(Self::Text(value))
    }

    /// Creates a numeric value, rejecting empty strings and non-digits.
    pub fn digits(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();

        if value.is_empty() {
            return Err(Error::EmptyDigits);
        }

        for &byte in value.as_bytes() {
            if !byte.is_ascii_digit() {
                return Err(Error::InvalidDigit { field: None, byte });
            }
        }

        Ok(Self::Digits(value))
    }

    /// Creates a binary value from raw bytes.
    pub fn binary(value: impl Into<Vec<u8>>) -> Self {
        Self::Binary(value.into())
    }

    /// Returns the value as a string slice for text and numeric fields.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) | Self::Digits(value) => Some(value),
            Self::Binary(_) => None,
        }
    }

    /// Returns the value as a digit string for numeric fields.
    pub fn as_digits(&self) -> Option<&str> {
        match self {
            Self::Digits(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the raw bytes of the value.
    ///
    /// For text and numeric values this is the ASCII representation, not the
    /// on-wire BCD representation.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Text(value) | Self::Digits(value) => value.as_bytes(),
            Self::Binary(value) => value,
        }
    }

    /// Returns the length in characters for text/numeric values and in bytes
    /// for binary values.
    pub fn len(&self) -> usize {
        match self {
            Self::Text(value) | Self::Digits(value) => value.len(),
            Self::Binary(value) => value.len(),
        }
    }

    /// Returns `true` when the value has zero length.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(value) | Self::Digits(value) => f.write_str(value),
            Self::Binary(value) => {
                f.write_str("0x")?;

                for byte in value {
                    write!(f, "{byte:02X}")?;
                }

                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_leading_zeros() {
        let value = FieldValue::digits("000000010000").unwrap();
        assert_eq!(value.as_digits(), Some("000000010000"));
        assert_eq!(value.len(), 12);
    }

    #[test]
    fn rejects_empty_digits() {
        assert_eq!(FieldValue::digits("").unwrap_err(), Error::EmptyDigits);
    }

    #[test]
    fn rejects_non_digits() {
        assert!(matches!(
            FieldValue::digits("12A4"),
            Err(Error::InvalidDigit { field: None, .. })
        ));
    }

    #[test]
    fn rejects_non_printable_text() {
        assert!(matches!(
            FieldValue::text("hello\n"),
            Err(Error::InvalidTextByte { field: None, .. })
        ));
    }

    #[test]
    fn formats_binary_as_hex() {
        let value = FieldValue::binary([0x12, 0xAB]);
        assert_eq!(value.to_string(), "0x12AB");
    }
}
