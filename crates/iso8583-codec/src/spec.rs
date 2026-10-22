//! Message specifications: field layouts, encodings and length prefixes.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::Error;

/// How a field's characters or bytes are represented on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    /// One byte per character, normally ASCII.
    Ascii,
    /// Two decimal digits per byte, high nibble first.
    Bcd,
    /// Raw bytes with no character encoding.
    Binary,
}

/// The semantic kind of a field, used for validation and value conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKind {
    /// Digits only (`N` fields).
    Numeric,
    /// Printable ASCII text (`AN`, `ANS`, `Z` fields).
    Text,
    /// Opaque bytes (`B` fields).
    Binary,
}

/// The encoding of a variable-length field's length prefix.
///
/// The integer is the number of decimal digits in the prefix. For example,
/// `Prefix::Ascii(2)` is the usual `LL` prefix: two ASCII digits such as
/// `b"16"`. `Prefix::Bcd(2)` is the same length encoded in one BCD byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Prefix {
    /// ASCII digits.
    Ascii(u8),
    /// BCD digits, packed two per byte.
    Bcd(u8),
}

impl Prefix {
    /// Returns the number of digits in the prefix.
    pub const fn digits(self) -> u8 {
        match self {
            Self::Ascii(digits) | Self::Bcd(digits) => digits,
        }
    }

    /// Returns `true` when the digit count is supported (1 through 3).
    pub const fn is_valid(self) -> bool {
        self.digits() >= 1 && self.digits() <= 3
    }

    /// Returns the largest length this prefix can represent.
    pub const fn max_value(self) -> u16 {
        match self.digits() {
            1 => 9,
            2 => 99,
            3 => 999,
            _ => 0,
        }
    }

    /// Returns the number of bytes the prefix occupies on the wire.
    pub const fn byte_len(self) -> usize {
        match self {
            Self::Ascii(digits) => digits as usize,
            Self::Bcd(digits) => (digits as usize).div_ceil(2),
        }
    }
}

/// The length rule for a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Length {
    /// A fixed number of characters or bytes.
    Fixed(u16),
    /// A variable length with a maximum and a length prefix.
    Variable {
        /// Maximum number of characters or bytes.
        max: u16,
        /// Encoding of the length prefix.
        prefix: Prefix,
    },
}

impl Length {
    /// Creates a fixed length.
    pub const fn fixed(length: u16) -> Self {
        Self::Fixed(length)
    }

    /// Creates an `LLVAR` length with an ASCII `LL` prefix.
    pub const fn llvar(max: u16) -> Self {
        Self::Variable {
            max,
            prefix: Prefix::Ascii(2),
        }
    }

    /// Creates an `LLLVAR` length with an ASCII `LLL` prefix.
    pub const fn lllvar(max: u16) -> Self {
        Self::Variable {
            max,
            prefix: Prefix::Ascii(3),
        }
    }

    /// Creates a variable length with an explicit prefix encoding.
    pub const fn variable(max: u16, prefix: Prefix) -> Self {
        Self::Variable { max, prefix }
    }
}

/// Padding applied to a fixed-length field that is shorter than its declared
/// length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Padding {
    /// Padding is not allowed; the value must already have the exact length.
    None,
    /// Pad on the left with the given byte (usually `b'0'`).
    Left(u8),
    /// Pad on the right with the given byte (usually `b' '`).
    Right(u8),
}

/// The complete definition of one data field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldSpec {
    /// Field number (`2..=128`, excluding `65`).
    pub number: u8,
    /// Human-readable name used in diagnostics.
    pub name: &'static str,
    /// Semantic kind used for validation.
    pub kind: FieldKind,
    /// Length rule.
    pub length: Length,
    /// Wire encoding.
    pub encoding: Encoding,
    /// Padding rule for fixed-length fields.
    pub padding: Padding,
}

impl FieldSpec {
    /// Creates a field definition with explicit attributes.
    pub const fn new(
        number: u8,
        name: &'static str,
        kind: FieldKind,
        length: Length,
        encoding: Encoding,
        padding: Padding,
    ) -> Self {
        Self {
            number,
            name,
            kind,
            length,
            encoding,
            padding,
        }
    }

    /// Creates a numeric ASCII field, padding fixed-length values with zeros.
    pub const fn numeric(number: u8, name: &'static str, length: Length) -> Self {
        let padding = match length {
            Length::Fixed(_) => Padding::Left(b'0'),
            Length::Variable { .. } => Padding::None,
        };

        Self::new(
            number,
            name,
            FieldKind::Numeric,
            length,
            Encoding::Ascii,
            padding,
        )
    }

    /// Creates a text ASCII field, padding fixed-length values with spaces.
    pub const fn text(number: u8, name: &'static str, length: Length) -> Self {
        let padding = match length {
            Length::Fixed(_) => Padding::Right(b' '),
            Length::Variable { .. } => Padding::None,
        };

        Self::new(
            number,
            name,
            FieldKind::Text,
            length,
            Encoding::Ascii,
            padding,
        )
    }

    /// Creates a binary field.
    pub const fn binary(number: u8, name: &'static str, length: Length) -> Self {
        Self::new(
            number,
            name,
            FieldKind::Binary,
            length,
            Encoding::Binary,
            Padding::None,
        )
    }

    /// Returns a copy of the field with a different encoding.
    pub const fn with_encoding(mut self, encoding: Encoding) -> Self {
        self.encoding = encoding;
        self
    }

    /// Returns a copy of the field with a different padding rule.
    pub const fn with_padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }
}

/// A named collection of field definitions.
///
/// A spec only describes the wire layout. It does not encode a particular
/// network's business rules or private field usage.
#[derive(Debug, Clone)]
pub struct Spec {
    name: Cow<'static, str>,
    fields: Cow<'static, [FieldSpec]>,
}

impl Spec {
    /// Creates a spec from owned field definitions and validates it.
    pub fn new(name: impl Into<String>, fields: Vec<FieldSpec>) -> Result<Self, Error> {
        let spec = Self {
            name: Cow::Owned(name.into()),
            fields: Cow::Owned(fields),
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Creates a spec that borrows static data.
    pub fn from_static(name: &'static str, fields: &'static [FieldSpec]) -> Self {
        Self {
            name: Cow::Borrowed(name),
            fields: Cow::Borrowed(fields),
        }
    }

    /// Starts building an owned spec.
    pub fn builder(name: impl Into<String>) -> SpecBuilder {
        SpecBuilder::new(name)
    }

    /// Returns the built-in ISO 8583:1987 field table (fields 2 through 64).
    pub fn iso8583_1987() -> Self {
        Self::from_static("ISO 8583:1987", &ISO8583_1987_FIELDS)
    }

    /// Returns the spec name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Looks up a field definition by number.
    pub fn field(&self, number: u8) -> Option<&FieldSpec> {
        self.fields.iter().find(|field| field.number == number)
    }

    /// Returns all field definitions in declaration order.
    pub fn fields(&self) -> &[FieldSpec] {
        &self.fields
    }

    /// Checks that the field table is internally consistent.
    pub fn validate(&self) -> Result<(), Error> {
        if self.name.is_empty() {
            return Err(Error::InvalidSpec {
                field: 0,
                reason: "spec name must not be empty",
            });
        }

        if self.fields.is_empty() {
            return Err(Error::InvalidSpec {
                field: 0,
                reason: "spec must contain at least one field",
            });
        }

        let mut seen = [false; 129];

        for field in self.fields.iter() {
            if field.number < 2 || field.number > 128 || field.number == 65 {
                return Err(Error::InvalidSpec {
                    field: field.number,
                    reason: "field number must be in 2..=128 and not 65",
                });
            }

            if seen[field.number as usize] {
                return Err(Error::InvalidSpec {
                    field: field.number,
                    reason: "duplicate field number",
                });
            }
            seen[field.number as usize] = true;

            if field.name.is_empty() {
                return Err(Error::InvalidSpec {
                    field: field.number,
                    reason: "field name must not be empty",
                });
            }

            match field.length {
                Length::Fixed(0) => {
                    return Err(Error::InvalidSpec {
                        field: field.number,
                        reason: "fixed length must be greater than zero",
                    });
                }
                Length::Fixed(_) => {}
                Length::Variable { max, prefix } => {
                    if !prefix.is_valid() {
                        return Err(Error::InvalidSpec {
                            field: field.number,
                            reason: "length prefix must contain 1 to 3 digits",
                        });
                    }

                    if max == 0 {
                        return Err(Error::InvalidSpec {
                            field: field.number,
                            reason: "variable maximum must be greater than zero",
                        });
                    }

                    if max > prefix.max_value() {
                        return Err(Error::InvalidSpec {
                            field: field.number,
                            reason: "variable maximum exceeds the prefix capacity",
                        });
                    }

                    if field.padding != Padding::None {
                        return Err(Error::InvalidSpec {
                            field: field.number,
                            reason: "variable-length fields cannot use padding",
                        });
                    }
                }
            }

            match (field.kind, field.encoding) {
                (FieldKind::Numeric, Encoding::Ascii | Encoding::Bcd) => {}
                (FieldKind::Text, Encoding::Ascii) => {}
                (FieldKind::Binary, Encoding::Binary) => {}
                _ => {
                    return Err(Error::InvalidSpec {
                        field: field.number,
                        reason: "incompatible field kind and encoding",
                    });
                }
            }

            match field.kind {
                FieldKind::Numeric => {
                    if let Padding::Left(byte) | Padding::Right(byte) = field.padding {
                        if !byte.is_ascii_digit() {
                            return Err(Error::InvalidSpec {
                                field: field.number,
                                reason: "numeric padding must be an ASCII digit",
                            });
                        }
                    }
                }
                FieldKind::Text => {
                    if let Padding::Left(byte) | Padding::Right(byte) = field.padding {
                        if !(0x20..=0x7E).contains(&byte) {
                            return Err(Error::InvalidSpec {
                                field: field.number,
                                reason: "text padding must be printable ASCII",
                            });
                        }
                    }
                }
                FieldKind::Binary => {
                    if field.padding != Padding::None {
                        return Err(Error::InvalidSpec {
                            field: field.number,
                            reason: "binary fields cannot use padding",
                        });
                    }
                }
            }
        }

        Ok(())
    }
}

/// Builder for an owned [`Spec`].
#[derive(Debug, Clone)]
pub struct SpecBuilder {
    name: String,
    fields: Vec<FieldSpec>,
}

impl SpecBuilder {
    /// Creates a builder with the given spec name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: Vec::new(),
        }
    }

    /// Adds a field definition.
    pub fn field(mut self, field: FieldSpec) -> Self {
        self.fields.push(field);
        self
    }

    /// Validates the field table and returns the spec.
    pub fn build(self) -> Result<Spec, Error> {
        Spec::new(self.name, self.fields)
    }
}

/// Field definitions from ISO 8583:1987, using ASCII encoding by default.
///
/// Fields 52, 53 and 64 are binary. Networks that encode them as hex ASCII
/// should provide their own spec instead.
static ISO8583_1987_FIELDS: [FieldSpec; 63] = [
    FieldSpec::numeric(2, "Primary Account Number", Length::llvar(19)),
    FieldSpec::numeric(3, "Processing Code", Length::fixed(6)),
    FieldSpec::numeric(4, "Transaction Amount", Length::fixed(12)),
    FieldSpec::numeric(5, "Settlement Amount", Length::fixed(12)),
    FieldSpec::numeric(6, "Cardholder Billing Amount", Length::fixed(12)),
    FieldSpec::numeric(7, "Transmission Date and Time", Length::fixed(10)),
    FieldSpec::numeric(8, "Cardholder Billing Fee Amount", Length::fixed(8)),
    FieldSpec::numeric(9, "Settlement Conversion Rate", Length::fixed(8)),
    FieldSpec::numeric(10, "Cardholder Billing Conversion Rate", Length::fixed(8)),
    FieldSpec::numeric(11, "System Trace Audit Number", Length::fixed(6)),
    FieldSpec::numeric(12, "Local Transaction Time", Length::fixed(6)),
    FieldSpec::numeric(13, "Local Transaction Date", Length::fixed(4)),
    FieldSpec::numeric(14, "Expiration Date", Length::fixed(4)),
    FieldSpec::numeric(15, "Settlement Date", Length::fixed(4)),
    FieldSpec::numeric(16, "Currency Conversion Date", Length::fixed(4)),
    FieldSpec::numeric(17, "Capture Date", Length::fixed(4)),
    FieldSpec::numeric(18, "Merchant Type", Length::fixed(4)),
    FieldSpec::numeric(19, "Acquiring Institution Country Code", Length::fixed(3)),
    FieldSpec::numeric(20, "PAN Extended Country Code", Length::fixed(3)),
    FieldSpec::numeric(21, "Forwarding Institution Country Code", Length::fixed(3)),
    FieldSpec::numeric(22, "Point of Service Entry Mode", Length::fixed(3)),
    FieldSpec::numeric(23, "Card Sequence Number", Length::fixed(3)),
    FieldSpec::numeric(24, "Function Code", Length::fixed(3)),
    FieldSpec::numeric(25, "Point of Service Condition Code", Length::fixed(2)),
    FieldSpec::numeric(26, "Point of Service PIN Capture Code", Length::fixed(2)),
    FieldSpec::numeric(
        27,
        "Authorization Identification Response Length",
        Length::fixed(1),
    ),
    FieldSpec::numeric(28, "Transaction Fee Amount", Length::fixed(9)),
    FieldSpec::numeric(29, "Settlement Fee Amount", Length::fixed(9)),
    FieldSpec::numeric(30, "Transaction Processing Fee Amount", Length::fixed(9)),
    FieldSpec::numeric(31, "Settlement Processing Fee Amount", Length::fixed(9)),
    FieldSpec::numeric(
        32,
        "Acquiring Institution Identification Code",
        Length::llvar(11),
    ),
    FieldSpec::numeric(
        33,
        "Forwarding Institution Identification Code",
        Length::llvar(11),
    ),
    FieldSpec::numeric(34, "Extended Primary Account Number", Length::llvar(28)),
    FieldSpec::text(35, "Track 2 Data", Length::llvar(37)),
    FieldSpec::text(36, "Track 3 Data", Length::lllvar(104)),
    FieldSpec::text(37, "Retrieval Reference Number", Length::fixed(12)),
    FieldSpec::text(
        38,
        "Authorization Identification Response",
        Length::fixed(6),
    ),
    FieldSpec::text(39, "Response Code", Length::fixed(2)),
    FieldSpec::text(40, "Service Restriction Code", Length::fixed(3)),
    FieldSpec::text(
        41,
        "Card Acceptor Terminal Identification",
        Length::fixed(8),
    ),
    FieldSpec::text(42, "Card Acceptor Identification Code", Length::fixed(15)),
    FieldSpec::text(43, "Card Acceptor Name and Location", Length::fixed(40)),
    FieldSpec::text(44, "Additional Response Data", Length::llvar(99)),
    FieldSpec::text(45, "Track 1 Data", Length::llvar(76)),
    FieldSpec::text(46, "Additional Data (ISO)", Length::lllvar(999)),
    FieldSpec::text(47, "Additional Data (National)", Length::lllvar(999)),
    FieldSpec::text(48, "Additional Data (Private)", Length::lllvar(999)),
    FieldSpec::numeric(49, "Transaction Currency Code", Length::fixed(3)),
    FieldSpec::numeric(50, "Settlement Currency Code", Length::fixed(3)),
    FieldSpec::numeric(51, "Cardholder Billing Currency Code", Length::fixed(3)),
    FieldSpec::binary(52, "PIN Data", Length::fixed(8)),
    FieldSpec::binary(
        53,
        "Security Related Control Information",
        Length::fixed(16),
    ),
    FieldSpec::text(54, "Additional Amounts", Length::lllvar(120)),
    FieldSpec::text(55, "ICC Data (EMV)", Length::lllvar(999)),
    FieldSpec::text(56, "Reserved (ISO)", Length::lllvar(999)),
    FieldSpec::text(57, "Reserved (National)", Length::lllvar(999)),
    FieldSpec::text(58, "Reserved (National)", Length::lllvar(999)),
    FieldSpec::text(59, "Reserved (National)", Length::lllvar(999)),
    FieldSpec::text(60, "Reserved (Private)", Length::lllvar(999)),
    FieldSpec::text(61, "Reserved (Private)", Length::lllvar(999)),
    FieldSpec::text(62, "Reserved (Private)", Length::lllvar(999)),
    FieldSpec::text(63, "Reserved (Private)", Length::lllvar(999)),
    FieldSpec::binary(64, "Message Authentication Code", Length::fixed(8)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_1987_spec_is_valid() {
        let spec = Spec::iso8583_1987();
        spec.validate().unwrap();
        assert_eq!(spec.name(), "ISO 8583:1987");
        assert_eq!(spec.fields().len(), 63);
        assert_eq!(spec.field(2).unwrap().length, Length::llvar(19));
        assert_eq!(spec.field(64).unwrap().encoding, Encoding::Binary);
        assert!(spec.field(1).is_none());
        assert!(spec.field(65).is_none());
    }

    #[test]
    fn rejects_duplicate_fields() {
        let result = Spec::builder("duplicate")
            .field(FieldSpec::numeric(2, "PAN", Length::llvar(19)))
            .field(FieldSpec::numeric(2, "PAN again", Length::llvar(19)))
            .build();

        assert!(matches!(
            result,
            Err(Error::InvalidSpec {
                field: 2,
                reason: "duplicate field number"
            })
        ));
    }

    #[test]
    fn rejects_prefix_overflow() {
        let result = Spec::builder("overflow")
            .field(FieldSpec::numeric(2, "PAN", Length::llvar(100)))
            .build();

        assert!(matches!(
            result,
            Err(Error::InvalidSpec {
                field: 2,
                reason: "variable maximum exceeds the prefix capacity"
            })
        ));
    }

    #[test]
    fn rejects_incompatible_encoding() {
        let result = Spec::builder("bad encoding")
            .field(FieldSpec::text(2, "Text", Length::fixed(4)).with_encoding(Encoding::Bcd))
            .build();

        assert!(matches!(
            result,
            Err(Error::InvalidSpec {
                field: 2,
                reason: "incompatible field kind and encoding"
            })
        ));
    }

    #[test]
    fn rejects_padding_on_variable_fields() {
        let result = Spec::builder("bad padding")
            .field(FieldSpec::numeric(2, "PAN", Length::llvar(19)))
            .build();

        assert!(result.is_ok());

        let result = Spec::builder("bad padding")
            .field(
                FieldSpec::numeric(2, "PAN", Length::llvar(19)).with_padding(Padding::Left(b'0')),
            )
            .build();

        assert!(matches!(
            result,
            Err(Error::InvalidSpec {
                field: 2,
                reason: "variable-length fields cannot use padding"
            })
        ));
    }

    #[test]
    fn rejects_reserved_field_numbers() {
        for number in [1, 65, 129] {
            let result = Spec::builder("reserved")
                .field(FieldSpec::numeric(number, "Reserved", Length::fixed(1)))
                .build();

            assert!(result.is_err(), "field {number} should be rejected");
        }
    }
}
