//! Error types returned by the codec.

use core::fmt;

/// An error produced while parsing, validating or encoding an ISO 8583
/// message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The input ended before the requested number of bytes were available.
    UnexpectedEof {
        /// Number of bytes the decoder tried to read.
        needed: usize,
        /// Number of bytes that were actually left.
        remaining: usize,
    },

    /// The MTI was not exactly four bytes long.
    InvalidMtiLength {
        /// Actual length in bytes.
        actual: usize,
    },

    /// The MTI contained a byte that is not an ASCII digit.
    InvalidMtiDigit {
        /// Zero-based position of the offending byte.
        index: usize,
        /// The offending byte.
        byte: u8,
    },

    /// A bitmap was neither 8 nor 16 bytes long.
    InvalidBitmapLength {
        /// Actual length in bytes.
        actual: usize,
    },

    /// The primary bitmap announced a secondary bitmap, but none followed.
    MissingSecondaryBitmap,

    /// A secondary bitmap was present although the primary bitmap did not
    /// announce one.
    UnexpectedSecondaryBitmap,

    /// The secondary bitmap announced a tertiary bitmap, which is not
    /// supported by this crate.
    UnsupportedTertiaryBitmap,

    /// Field 1 and field 65 are bitmap indicators, not data fields.
    ReservedBitmapField {
        /// The reserved field number.
        field: u8,
    },

    /// A field number outside the range `2..=128` was used.
    InvalidFieldNumber {
        /// The invalid field number.
        field: u8,
    },

    /// The bitmap contains a field for which the specification has no
    /// definition.
    UnknownField {
        /// The unknown field number.
        field: u8,
    },

    /// A field value was longer than the maximum allowed by the specification.
    FieldTooLong {
        /// Field number.
        field: u8,
        /// Maximum length allowed by the specification.
        max: usize,
        /// Actual length of the value.
        actual: usize,
    },

    /// A fixed-length field had a different length and no padding rule that
    /// could make up the difference.
    InvalidFixedLength {
        /// Field number.
        field: u8,
        /// Expected length.
        expected: usize,
        /// Actual length.
        actual: usize,
    },

    /// A variable-length prefix could not be decoded.
    InvalidLengthPrefix {
        /// Field number.
        field: u8,
    },

    /// A numeric field contained a byte that is not an ASCII digit.
    InvalidDigit {
        /// Field number, when known.
        field: Option<u8>,
        /// The offending byte.
        byte: u8,
    },

    /// A text field contained a byte outside the printable ASCII range.
    InvalidTextByte {
        /// Field number, when known.
        field: Option<u8>,
        /// The offending byte.
        byte: u8,
    },

    /// An attempt was made to construct an empty numeric value.
    EmptyDigits,

    /// A specification is internally inconsistent.
    InvalidSpec {
        /// Field number, or zero for a specification-wide problem.
        field: u8,
        /// Human-readable reason.
        reason: &'static str,
    },

    /// The encoding is not supported for this field kind.
    UnsupportedEncoding {
        /// Field number.
        field: u8,
    },

    /// Bytes remained after the last field was decoded.
    TrailingBytes {
        /// Number of unconsumed bytes.
        remaining: usize,
    },

    /// The [`crate::FieldValue`] variant does not match the field kind declared
    /// by the specification.
    FieldTypeMismatch {
        /// Field number.
        field: u8,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnexpectedEof { needed, remaining } => write!(
                f,
                "unexpected end of input: needed {needed} more byte(s), {remaining} left"
            ),
            Error::InvalidMtiLength { actual } => {
                write!(f, "invalid MTI length {actual}: expected 4 bytes")
            }
            Error::InvalidMtiDigit { index, byte } => write!(
                f,
                "invalid MTI digit at position {index}: 0x{byte:02X} is not an ASCII digit"
            ),
            Error::InvalidBitmapLength { actual } => {
                write!(f, "invalid bitmap length {actual}: expected 8 or 16 bytes")
            }
            Error::MissingSecondaryBitmap => {
                write!(
                    f,
                    "primary bitmap announces a secondary bitmap, but none followed"
                )
            }
            Error::UnexpectedSecondaryBitmap => {
                write!(
                    f,
                    "secondary bitmap present, but primary bitmap bit 1 is not set"
                )
            }
            Error::UnsupportedTertiaryBitmap => {
                write!(f, "tertiary bitmaps (field 65) are not supported")
            }
            Error::ReservedBitmapField { field } => {
                write!(f, "field {field} is a reserved bitmap indicator")
            }
            Error::InvalidFieldNumber { field } => {
                write!(f, "invalid field number {field}: expected 2..=128")
            }
            Error::UnknownField { field } => {
                write!(
                    f,
                    "bitmap contains field {field}, but the spec has no definition for it"
                )
            }
            Error::FieldTooLong { field, max, actual } => write!(
                f,
                "field {field} is too long: maximum is {max}, got {actual}"
            ),
            Error::InvalidFixedLength {
                field,
                expected,
                actual,
            } => write!(
                f,
                "field {field} has fixed length {expected}, but the value is {actual} character(s)"
            ),
            Error::InvalidLengthPrefix { field } => {
                write!(f, "field {field} has an invalid length prefix")
            }
            Error::InvalidDigit { field, byte } => match field {
                Some(field) => write!(
                    f,
                    "field {field} contains 0x{byte:02X}, which is not an ASCII digit"
                ),
                None => write!(
                    f,
                    "value contains 0x{byte:02X}, which is not an ASCII digit"
                ),
            },
            Error::InvalidTextByte { field, byte } => match field {
                Some(field) => write!(
                    f,
                    "field {field} contains 0x{byte:02X}, which is not printable ASCII"
                ),
                None => write!(
                    f,
                    "value contains 0x{byte:02X}, which is not printable ASCII"
                ),
            },
            Error::EmptyDigits => write!(f, "numeric field value must not be empty"),
            Error::InvalidSpec { field, reason } => {
                if *field == 0 {
                    write!(f, "invalid spec: {reason}")
                } else {
                    write!(f, "invalid spec for field {field}: {reason}")
                }
            }
            Error::UnsupportedEncoding { field } => {
                write!(
                    f,
                    "field {field} uses an encoding that is not supported for its kind"
                )
            }
            Error::TrailingBytes { remaining } => {
                write!(f, "{remaining} trailing byte(s) after the last field")
            }
            Error::FieldTypeMismatch { field } => {
                write!(
                    f,
                    "field {field} value does not match its declared field kind"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
