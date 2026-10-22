//! Message parsing and encoding.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bitmap::Bitmap;
use crate::error::Error;
use crate::field::FieldValue;
use crate::mti::Mti;
use crate::spec::{Encoding, FieldKind, FieldSpec, Length, Padding, Prefix, Spec};

/// A parsed ISO 8583 message.
///
/// The message stores the MTI and the data fields in ascending field-number
/// order. The bitmap is derived from the fields when encoding and is not kept
/// as a separate public field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    mti: Mti,
    fields: BTreeMap<u8, FieldValue>,
}

impl Message {
    /// Creates an empty message with the given MTI.
    pub fn new(mti: Mti) -> Self {
        Self {
            mti,
            fields: BTreeMap::new(),
        }
    }

    /// Parses a complete message body.
    ///
    /// `input` must contain exactly one message: the four-byte MTI, the bitmap
    /// and the fields. It must not contain a network length header (MLI).
    pub fn parse(spec: &Spec, input: &[u8]) -> Result<Self, Error> {
        spec.validate()?;

        let mut cursor = Cursor::new(input);
        let mti = Mti::parse(cursor.take(4)?)?;

        let primary = cursor.take(8)?;
        let has_secondary = primary[0] & 0x80 != 0;

        let bitmap = if has_secondary {
            let secondary = cursor.take(8)?;
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(primary);
            bytes[8..].copy_from_slice(secondary);
            Bitmap::from_slice(&bytes)?
        } else {
            Bitmap::from_slice(primary)?
        };

        let mut fields = BTreeMap::new();

        for number in bitmap.iter() {
            let field_spec = spec
                .field(number)
                .ok_or(Error::UnknownField { field: number })?;
            let value = parse_field(&mut cursor, field_spec)?;
            fields.insert(number, value);
        }

        if cursor.remaining() != 0 {
            return Err(Error::TrailingBytes {
                remaining: cursor.remaining(),
            });
        }

        Ok(Self { mti, fields })
    }

    /// Encodes the message body, including the MTI and bitmap.
    pub fn encode(&self, spec: &Spec) -> Result<Vec<u8>, Error> {
        spec.validate()?;

        let bitmap = self.bitmap()?;
        let bitmap_bytes = bitmap.to_vec();

        let mut out = Vec::with_capacity(4 + bitmap_bytes.len() + 64);
        out.extend_from_slice(self.mti.as_bytes());
        out.extend_from_slice(&bitmap_bytes);

        for (number, value) in &self.fields {
            let field_spec = spec
                .field(*number)
                .ok_or(Error::UnknownField { field: *number })?;
            encode_field(&mut out, field_spec, value)?;
        }

        Ok(out)
    }

    /// Encodes the message and appends it to `out`.
    ///
    /// The message is encoded into a temporary buffer first, so `out` is left
    /// untouched when encoding fails.
    pub fn encode_to(&self, spec: &Spec, out: &mut Vec<u8>) -> Result<(), Error> {
        let encoded = self.encode(spec)?;
        out.extend_from_slice(&encoded);
        Ok(())
    }

    /// Returns the message type indicator.
    pub const fn mti(&self) -> Mti {
        self.mti
    }

    /// Replaces the message type indicator.
    pub fn set_mti(&mut self, mti: Mti) {
        self.mti = mti;
    }

    /// Returns a field value by field number.
    pub fn field(&self, number: u8) -> Option<&FieldValue> {
        self.fields.get(&number)
    }

    /// Sets a field value.
    ///
    /// Field numbers `1` and `65` are reserved bitmap indicators and are
    /// rejected. Whether the value is compatible with the spec is checked when
    /// the message is encoded.
    pub fn set_field(&mut self, number: u8, value: FieldValue) -> Result<(), Error> {
        validate_data_field(number)?;
        self.fields.insert(number, value);
        Ok(())
    }

    /// Removes a field and returns its previous value.
    pub fn remove_field(&mut self, number: u8) -> Option<FieldValue> {
        self.fields.remove(&number)
    }

    /// Iterates over the fields in ascending field-number order.
    pub fn fields(&self) -> impl Iterator<Item = (u8, &FieldValue)> + '_ {
        self.fields.iter().map(|(number, value)| (*number, value))
    }

    /// Returns the number of populated fields.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Returns `true` when no data fields are populated.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Removes all data fields while keeping the MTI.
    pub fn clear(&mut self) {
        self.fields.clear();
    }

    /// Builds the bitmap that corresponds to the current set of fields.
    pub fn bitmap(&self) -> Result<Bitmap, Error> {
        let mut bitmap = Bitmap::new();

        for number in self.fields.keys() {
            bitmap.set(*number)?;
        }

        Ok(bitmap)
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

struct Cursor<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.input.len() - self.position
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        if self.remaining() < length {
            return Err(Error::UnexpectedEof {
                needed: length,
                remaining: self.remaining(),
            });
        }

        let start = self.position;
        self.position += length;
        Ok(&self.input[start..self.position])
    }
}

fn parse_field(cursor: &mut Cursor<'_>, spec: &FieldSpec) -> Result<FieldValue, Error> {
    let length = match spec.length {
        Length::Fixed(length) => length as usize,
        Length::Variable { max, prefix } => {
            let prefix_bytes = cursor.take(prefix.byte_len())?;
            let length = decode_prefix(prefix, prefix_bytes, spec.number)?;

            if length > max as usize {
                return Err(Error::FieldTooLong {
                    field: spec.number,
                    max: max as usize,
                    actual: length,
                });
            }

            length
        }
    };

    let byte_length = match spec.encoding {
        Encoding::Ascii => length,
        Encoding::Bcd => length.div_ceil(2),
        Encoding::Binary => length,
    };

    let raw = cursor.take(byte_length)?;

    match spec.kind {
        FieldKind::Numeric => match spec.encoding {
            Encoding::Ascii => Ok(FieldValue::Digits(decode_ascii_digits(raw, spec.number)?)),
            Encoding::Bcd => Ok(FieldValue::Digits(decode_bcd(raw, length, spec.number)?)),
            Encoding::Binary => Err(Error::UnsupportedEncoding { field: spec.number }),
        },
        FieldKind::Text => {
            if spec.encoding != Encoding::Ascii {
                return Err(Error::UnsupportedEncoding { field: spec.number });
            }

            for &byte in raw {
                if !(0x20..=0x7E).contains(&byte) {
                    return Err(Error::InvalidTextByte {
                        field: Some(spec.number),
                        byte,
                    });
                }
            }

            let text = core::str::from_utf8(raw).expect("validated printable ASCII");
            Ok(FieldValue::Text(String::from(text)))
        }
        FieldKind::Binary => {
            if spec.encoding != Encoding::Binary {
                return Err(Error::UnsupportedEncoding { field: spec.number });
            }

            Ok(FieldValue::Binary(raw.to_vec()))
        }
    }
}

fn decode_ascii_digits(raw: &[u8], field: u8) -> Result<String, Error> {
    let mut digits = String::with_capacity(raw.len());

    for &byte in raw {
        if !byte.is_ascii_digit() {
            return Err(Error::InvalidDigit {
                field: Some(field),
                byte,
            });
        }

        digits.push(byte as char);
    }

    Ok(digits)
}

fn decode_bcd(raw: &[u8], digits: usize, field: u8) -> Result<String, Error> {
    let mut out = String::with_capacity(digits);

    for index in 0..digits {
        let byte = raw[index / 2];
        let nibble = if index % 2 == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        };

        if nibble > 9 {
            return Err(Error::InvalidDigit {
                field: Some(field),
                byte: nibble,
            });
        }

        out.push((b'0' + nibble) as char);
    }

    if digits % 2 == 1 {
        let padding = raw[digits / 2] & 0x0F;

        if padding != 0 && padding != 0x0F {
            return Err(Error::InvalidDigit {
                field: Some(field),
                byte: padding,
            });
        }
    }

    Ok(out)
}

fn decode_prefix(prefix: Prefix, raw: &[u8], field: u8) -> Result<usize, Error> {
    match prefix {
        Prefix::Ascii(digits) => {
            let mut value = 0usize;

            for &byte in &raw[..digits as usize] {
                if !byte.is_ascii_digit() {
                    return Err(Error::InvalidLengthPrefix { field });
                }

                value = value * 10 + (byte - b'0') as usize;
            }

            Ok(value)
        }
        Prefix::Bcd(digits) => {
            let mut value = 0usize;

            for index in 0..digits as usize {
                let byte = raw[index / 2];
                let nibble = if index % 2 == 0 {
                    byte >> 4
                } else {
                    byte & 0x0F
                };

                if nibble > 9 {
                    return Err(Error::InvalidLengthPrefix { field });
                }

                value = value * 10 + nibble as usize;
            }

            if digits % 2 == 1 {
                let padding = raw[digits as usize / 2] & 0x0F;

                if padding != 0 && padding != 0x0F {
                    return Err(Error::InvalidLengthPrefix { field });
                }
            }

            Ok(value)
        }
    }
}

fn encode_field(out: &mut Vec<u8>, spec: &FieldSpec, value: &FieldValue) -> Result<(), Error> {
    let mut data = match (spec.kind, value) {
        (FieldKind::Numeric, FieldValue::Digits(digits)) => {
            for &byte in digits.as_bytes() {
                if !byte.is_ascii_digit() {
                    return Err(Error::InvalidDigit {
                        field: Some(spec.number),
                        byte,
                    });
                }
            }

            digits.as_bytes().to_vec()
        }
        (FieldKind::Text, FieldValue::Text(text)) => {
            for &byte in text.as_bytes() {
                if !(0x20..=0x7E).contains(&byte) {
                    return Err(Error::InvalidTextByte {
                        field: Some(spec.number),
                        byte,
                    });
                }
            }

            text.as_bytes().to_vec()
        }
        (FieldKind::Binary, FieldValue::Binary(bytes)) => bytes.clone(),
        _ => return Err(Error::FieldTypeMismatch { field: spec.number }),
    };

    let length = data.len();

    match spec.length {
        Length::Fixed(expected) => {
            let expected = expected as usize;

            if length > expected {
                return Err(Error::FieldTooLong {
                    field: spec.number,
                    max: expected,
                    actual: length,
                });
            }

            if length < expected {
                match spec.padding {
                    Padding::Left(padding) => {
                        let mut padded = Vec::with_capacity(expected);
                        padded.resize(expected - length, padding);
                        padded.extend_from_slice(&data);
                        data = padded;
                    }
                    Padding::Right(padding) => {
                        data.resize(expected, padding);
                    }
                    Padding::None => {
                        return Err(Error::InvalidFixedLength {
                            field: spec.number,
                            expected,
                            actual: length,
                        });
                    }
                }
            }
        }
        Length::Variable { max, prefix } => {
            if length > max as usize {
                return Err(Error::FieldTooLong {
                    field: spec.number,
                    max: max as usize,
                    actual: length,
                });
            }

            encode_prefix(out, prefix, length);
        }
    }

    match spec.encoding {
        Encoding::Ascii => out.extend_from_slice(&data),
        Encoding::Bcd => encode_bcd(out, &data),
        Encoding::Binary => out.extend_from_slice(&data),
    }

    Ok(())
}

fn encode_prefix(out: &mut Vec<u8>, prefix: Prefix, length: usize) {
    debug_assert!(length <= prefix.max_value() as usize);

    match prefix {
        Prefix::Ascii(digits) => {
            let digits = digits as usize;
            let mut buffer = [b'0'; 3];
            let mut value = length;

            for index in (0..digits).rev() {
                buffer[index] = b'0' + (value % 10) as u8;
                value /= 10;
            }

            out.extend_from_slice(&buffer[..digits]);
        }
        Prefix::Bcd(digits) => {
            let digits = digits as usize;
            let mut nibbles = [0u8; 3];
            let mut value = length;

            for index in (0..digits).rev() {
                nibbles[index] = (value % 10) as u8;
                value /= 10;
            }

            let mut bytes = [0u8; 2];

            for (index, nibble) in nibbles[..digits].iter().enumerate() {
                if index % 2 == 0 {
                    bytes[index / 2] |= nibble << 4;
                } else {
                    bytes[index / 2] |= nibble;
                }
            }

            out.extend_from_slice(&bytes[..digits.div_ceil(2)]);
        }
    }
}

fn encode_bcd(out: &mut Vec<u8>, data: &[u8]) {
    let mut index = 0;

    while index + 1 < data.len() {
        let high = data[index] - b'0';
        let low = data[index + 1] - b'0';
        out.push((high << 4) | low);
        index += 2;
    }

    if index < data.len() {
        out.push((data[index] - b'0') << 4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::Prefix;

    #[test]
    fn parses_single_field() {
        let spec = Spec::builder("test")
            .field(FieldSpec::numeric(2, "PAN", Length::llvar(19)))
            .build()
            .unwrap();

        let mut input = Vec::new();
        input.extend_from_slice(b"0100");
        input.extend_from_slice(&[0x40, 0, 0, 0, 0, 0, 0, 0]);
        input.extend_from_slice(b"16");
        input.extend_from_slice(b"4242424242424242");

        let message = Message::parse(&spec, &input).unwrap();

        assert_eq!(message.mti().as_str(), "0100");
        assert_eq!(
            message.field(2).unwrap().as_digits(),
            Some("4242424242424242")
        );
    }

    #[test]
    fn parses_bcd_field() {
        let spec = Spec::builder("bcd")
            .field(
                FieldSpec::numeric(2, "PAN", Length::variable(19, Prefix::Bcd(2)))
                    .with_encoding(Encoding::Bcd),
            )
            .build()
            .unwrap();

        // MTI 0100, bitmap with field 2, BCD LL prefix 0x16, BCD PAN.
        let mut input = Vec::new();
        input.extend_from_slice(b"0100");
        input.extend_from_slice(&[0x40, 0, 0, 0, 0, 0, 0, 0]);
        input.push(0x16);
        input.extend_from_slice(&[0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42]);

        let message = Message::parse(&spec, &input).unwrap();
        assert_eq!(
            message.field(2).unwrap().as_digits(),
            Some("4242424242424242")
        );
    }

    #[test]
    fn encodes_bcd_field() {
        let spec = Spec::builder("bcd")
            .field(
                FieldSpec::numeric(2, "PAN", Length::variable(19, Prefix::Bcd(2)))
                    .with_encoding(Encoding::Bcd),
            )
            .build()
            .unwrap();

        let mut message = Message::new(Mti::parse(b"0100").unwrap());
        message
            .set_field(2, FieldValue::digits("4242424242424242").unwrap())
            .unwrap();

        let encoded = message.encode(&spec).unwrap();
        assert_eq!(&encoded[..5], &b"0100\x40"[..]);
        assert_eq!(encoded[12], 0x16);
        assert_eq!(
            &encoded[13..],
            &[0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42][..]
        );
    }

    #[test]
    fn rejects_trailing_bytes() {
        let spec = Spec::builder("test")
            .field(FieldSpec::numeric(2, "PAN", Length::fixed(1)))
            .build()
            .unwrap();

        let mut input = Vec::new();
        input.extend_from_slice(b"0100");
        input.extend_from_slice(&[0x40, 0, 0, 0, 0, 0, 0, 0]);
        input.extend_from_slice(b"30");

        assert_eq!(
            Message::parse(&spec, &input).unwrap_err(),
            Error::TrailingBytes { remaining: 1 }
        );
    }

    #[test]
    fn rejects_unknown_field() {
        let spec = Spec::builder("test")
            .field(FieldSpec::numeric(2, "PAN", Length::fixed(1)))
            .build()
            .unwrap();

        let mut input = Vec::new();
        input.extend_from_slice(b"0100");
        input.extend_from_slice(&[0x60, 0, 0, 0, 0, 0, 0, 0]);
        input.extend_from_slice(b"3");

        assert_eq!(
            Message::parse(&spec, &input).unwrap_err(),
            Error::UnknownField { field: 3 }
        );
    }

    #[test]
    fn decodes_bcd_length_prefix() {
        assert_eq!(decode_prefix(Prefix::Bcd(2), &[0x16], 2).unwrap(), 16);
        assert_eq!(
            decode_prefix(Prefix::Bcd(3), &[0x12, 0x30], 2).unwrap(),
            123
        );
    }

    #[test]
    fn encodes_bcd_length_prefix() {
        let mut out = Vec::new();
        encode_prefix(&mut out, Prefix::Bcd(2), 16);
        assert_eq!(out, vec![0x16]);

        out.clear();
        encode_prefix(&mut out, Prefix::Bcd(3), 123);
        assert_eq!(out, vec![0x12, 0x30]);
    }
}
