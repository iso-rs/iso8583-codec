//! Message Type Indicator (MTI) parsing and formatting.

use core::fmt;
use core::str::FromStr;

use crate::error::Error;

/// A four-digit ISO 8583 Message Type Indicator.
///
/// The MTI is usually described as four independent digits:
///
/// | Position | Meaning | Examples |
/// |----------|---------|----------|
/// | 1 | Version | `0` = 1987, `2` = 1993, `3` = 2003 |
/// | 2 | Class | `1` = authorization, `2` = financial, `4` = reversal, `8` = network |
/// | 3 | Function | `0` = request, `1` = response, `2` = advice |
/// | 4 | Origin | `0` = acquirer, `1` = acquirer repeat, `2` = issuer |
///
/// The type validates the shape (four ASCII digits) but intentionally does not
/// reject combinations that a particular network does not use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mti([u8; 4]);

impl Mti {
    /// Builds an MTI from four ASCII digit bytes.
    ///
    /// ```
    /// let mti = iso8583_codec::Mti::from_digits(*b"0100").unwrap();
    /// assert_eq!(mti.as_str(), "0100");
    /// ```
    pub const fn from_digits(digits: [u8; 4]) -> Result<Self, Error> {
        let mut index = 0;
        while index < 4 {
            let byte = digits[index];
            if byte < b'0' || byte > b'9' {
                return Err(Error::InvalidMtiDigit { index, byte });
            }
            index += 1;
        }
        Ok(Self(digits))
    }

    /// Parses an MTI from a byte slice that must be exactly four bytes long.
    pub fn parse(input: &[u8]) -> Result<Self, Error> {
        if input.len() != 4 {
            return Err(Error::InvalidMtiLength {
                actual: input.len(),
            });
        }

        let mut digits = [0u8; 4];
        digits.copy_from_slice(input);
        Self::from_digits(digits)
    }

    /// Builds an MTI from its four numeric components.
    ///
    /// ```
    /// let mti = iso8583_codec::Mti::new(0, 1, 0, 0).unwrap();
    /// assert_eq!(mti.as_str(), "0100");
    /// ```
    pub fn new(version: u8, class: u8, function: u8, origin: u8) -> Result<Self, Error> {
        let components = [version, class, function, origin];
        let mut digits = [0u8; 4];

        for (index, component) in components.into_iter().enumerate() {
            if component > 9 {
                return Err(Error::InvalidMtiDigit {
                    index,
                    byte: component,
                });
            }
            digits[index] = b'0' + component;
        }

        Self::from_digits(digits)
    }

    /// Returns the MTI as a string slice.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).expect("MTI is validated ASCII")
    }

    /// Returns the four underlying bytes.
    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }

    /// Returns the version digit (`0` for 1987, `2` for 1993, `3` for 2003).
    pub const fn version(&self) -> u8 {
        self.0[0] - b'0'
    }

    /// Returns the message class digit.
    pub const fn class(&self) -> u8 {
        self.0[1] - b'0'
    }

    /// Returns the message function digit.
    pub const fn function(&self) -> u8 {
        self.0[2] - b'0'
    }

    /// Returns the message origin digit.
    pub const fn origin(&self) -> u8 {
        self.0[3] - b'0'
    }
}

impl fmt::Display for Mti {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Mti {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input.as_bytes())
    }
}

impl AsRef<str> for Mti {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mti() {
        let mti = Mti::parse(b"0100").unwrap();
        assert_eq!(mti.as_str(), "0100");
        assert_eq!(mti.version(), 0);
        assert_eq!(mti.class(), 1);
        assert_eq!(mti.function(), 0);
        assert_eq!(mti.origin(), 0);
    }

    #[test]
    fn builds_mti_from_components() {
        let mti = Mti::new(2, 2, 1, 0).unwrap();
        assert_eq!(mti.as_str(), "2210");
    }

    #[test]
    fn rejects_bad_length() {
        assert_eq!(
            Mti::parse(b"010").unwrap_err(),
            Error::InvalidMtiLength { actual: 3 }
        );
    }

    #[test]
    fn rejects_non_digits() {
        assert_eq!(
            Mti::parse(b"01A0").unwrap_err(),
            Error::InvalidMtiDigit {
                index: 2,
                byte: b'A'
            }
        );
    }

    #[test]
    fn from_str_and_display() {
        let mti: Mti = "0800".parse().unwrap();
        assert_eq!(mti.to_string(), "0800");
    }
}
