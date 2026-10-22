//! End-to-end parsing and encoding tests.

use iso8583_codec::{Encoding, Error, FieldSpec, FieldValue, Length, Message, Mti, Prefix, Spec};

const FIELD_43: &str = "ACME STORE               NEW YORK     US";

fn sample_message() -> Vec<u8> {
    let mut bytes = Vec::new();

    bytes.extend_from_slice(b"0100");
    // Fields 2, 3, 4, 7, 11, 12, 13, 18, 22, 25, 32, 41, 42, 43 and 49.
    bytes.extend_from_slice(&[0x72, 0x38, 0x44, 0x81, 0x00, 0xE0, 0x80, 0x00]);
    bytes.extend_from_slice(b"164242424242424242");
    bytes.extend_from_slice(b"000000");
    bytes.extend_from_slice(b"000000010000");
    bytes.extend_from_slice(b"1022143000");
    bytes.extend_from_slice(b"000123");
    bytes.extend_from_slice(b"143000");
    bytes.extend_from_slice(b"1022");
    bytes.extend_from_slice(b"5411");
    bytes.extend_from_slice(b"051");
    bytes.extend_from_slice(b"00");
    bytes.extend_from_slice(b"1112345678901");
    bytes.extend_from_slice(b"TERM0001");
    bytes.extend_from_slice(b"MERCHANT0000001");
    bytes.extend_from_slice(FIELD_43.as_bytes());
    bytes.extend_from_slice(b"840");

    bytes
}

#[test]
fn field_43_fixture_has_fixed_length() {
    assert_eq!(FIELD_43.len(), 40);
}

#[test]
fn parses_1987_sample() {
    let spec = Spec::iso8583_1987();
    let bytes = sample_message();
    let message = Message::parse(&spec, &bytes).unwrap();

    assert_eq!(message.mti().as_str(), "0100");
    assert_eq!(message.field_count(), 15);
    assert_eq!(
        message.field(2).unwrap().as_digits(),
        Some("4242424242424242")
    );
    assert_eq!(message.field(3).unwrap().as_digits(), Some("000000"));
    assert_eq!(message.field(4).unwrap().as_digits(), Some("000000010000"));
    assert_eq!(message.field(7).unwrap().as_digits(), Some("1022143000"));
    assert_eq!(message.field(11).unwrap().as_digits(), Some("000123"));
    assert_eq!(message.field(12).unwrap().as_digits(), Some("143000"));
    assert_eq!(message.field(13).unwrap().as_digits(), Some("1022"));
    assert_eq!(message.field(18).unwrap().as_digits(), Some("5411"));
    assert_eq!(message.field(22).unwrap().as_digits(), Some("051"));
    assert_eq!(message.field(25).unwrap().as_digits(), Some("00"));
    assert_eq!(message.field(32).unwrap().as_digits(), Some("12345678901"));
    assert_eq!(message.field(41).unwrap().as_str(), Some("TERM0001"));
    assert_eq!(message.field(42).unwrap().as_str(), Some("MERCHANT0000001"));
    assert_eq!(message.field(43).unwrap().as_str(), Some(FIELD_43));
    assert_eq!(message.field(49).unwrap().as_digits(), Some("840"));
}

#[test]
fn roundtrips_1987_sample() {
    let spec = Spec::iso8583_1987();
    let bytes = sample_message();
    let message = Message::parse(&spec, &bytes).unwrap();

    assert_eq!(message.encode(&spec).unwrap(), bytes);
}

#[test]
fn roundtrips_secondary_bitmap() {
    let spec = Spec::builder("test")
        .field(FieldSpec::numeric(2, "PAN", Length::llvar(19)))
        .field(FieldSpec::numeric(
            70,
            "Network Management Information Code",
            Length::fixed(3),
        ))
        .build()
        .unwrap();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"0800");
    bytes.extend_from_slice(&[0xC0, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&[0x04, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(b"164242424242424242");
    bytes.extend_from_slice(b"301");

    let message = Message::parse(&spec, &bytes).unwrap();
    assert_eq!(message.mti().as_str(), "0800");
    assert_eq!(
        message.field(2).unwrap().as_digits(),
        Some("4242424242424242")
    );
    assert_eq!(message.field(70).unwrap().as_digits(), Some("301"));
    assert!(message.bitmap().unwrap().has_secondary());
    assert_eq!(message.encode(&spec).unwrap(), bytes);
}

#[test]
fn parses_bcd_prefix_and_value() {
    let spec = Spec::builder("bcd")
        .field(
            FieldSpec::numeric(2, "PAN", Length::variable(19, Prefix::Bcd(2)))
                .with_encoding(Encoding::Bcd),
        )
        .build()
        .unwrap();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"0100");
    bytes.extend_from_slice(&[0x40, 0, 0, 0, 0, 0, 0, 0]);
    bytes.push(0x16);
    bytes.extend_from_slice(&[0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42]);

    let message = Message::parse(&spec, &bytes).unwrap();
    assert_eq!(
        message.field(2).unwrap().as_digits(),
        Some("4242424242424242")
    );
    assert_eq!(message.encode(&spec).unwrap(), bytes);
}

#[test]
fn rejects_truncated_input() {
    let spec = Spec::iso8583_1987();

    assert_eq!(
        Message::parse(&spec, b"010").unwrap_err(),
        Error::UnexpectedEof {
            needed: 4,
            remaining: 3
        }
    );
}

#[test]
fn rejects_invalid_numeric_field() {
    let spec = Spec::builder("test")
        .field(FieldSpec::numeric(2, "PAN", Length::fixed(2)))
        .build()
        .unwrap();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"0100");
    bytes.extend_from_slice(&[0x40, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(b"AB");

    assert_eq!(
        Message::parse(&spec, &bytes).unwrap_err(),
        Error::InvalidDigit {
            field: Some(2),
            byte: b'A'
        }
    );
}

#[test]
fn rejects_value_that_does_not_fit_fixed_field() {
    let spec = Spec::builder("test")
        .field(FieldSpec::numeric(3, "Processing Code", Length::fixed(6)))
        .build()
        .unwrap();

    let mut message = Message::new(Mti::parse(b"0100").unwrap());
    message
        .set_field(3, FieldValue::digits("1234567").unwrap())
        .unwrap();

    assert_eq!(
        message.encode(&spec).unwrap_err(),
        Error::FieldTooLong {
            field: 3,
            max: 6,
            actual: 7
        }
    );
}

#[test]
fn rejects_field_type_mismatch() {
    let spec = Spec::builder("test")
        .field(FieldSpec::numeric(3, "Processing Code", Length::fixed(6)))
        .build()
        .unwrap();

    let mut message = Message::new(Mti::parse(b"0100").unwrap());
    message
        .set_field(3, FieldValue::text("000000").unwrap())
        .unwrap();

    assert_eq!(
        message.encode(&spec).unwrap_err(),
        Error::FieldTypeMismatch { field: 3 }
    );
}

#[test]
fn rejects_reserved_fields() {
    let mut message = Message::new(Mti::parse(b"0100").unwrap());

    assert_eq!(
        message
            .set_field(1, FieldValue::digits("0").unwrap())
            .unwrap_err(),
        Error::InvalidFieldNumber { field: 1 }
    );
    assert_eq!(
        message
            .set_field(65, FieldValue::digits("0").unwrap())
            .unwrap_err(),
        Error::ReservedBitmapField { field: 65 }
    );
}

#[test]
fn rejects_unknown_field_when_encoding() {
    let spec = Spec::builder("test")
        .field(FieldSpec::numeric(2, "PAN", Length::fixed(1)))
        .build()
        .unwrap();

    let mut message = Message::new(Mti::parse(b"0100").unwrap());
    message
        .set_field(3, FieldValue::digits("1").unwrap())
        .unwrap();

    assert_eq!(
        message.encode(&spec).unwrap_err(),
        Error::UnknownField { field: 3 }
    );
}
