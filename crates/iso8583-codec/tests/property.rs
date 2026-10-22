//! Property-based round-trip tests.

use iso8583_codec::{Encoding, FieldSpec, FieldValue, Length, Message, Mti, Prefix, Spec};
use proptest::prelude::*;

proptest! {
    #[test]
    fn roundtrips_fixed_ascii_fields(
        pan in "[0-9]{16}",
        amount in "[0-9]{12}",
        stan in "[0-9]{6}",
        currency in "[0-9]{3}",
    ) {
        let spec = Spec::iso8583_1987();
        let mut message = Message::new(Mti::parse(b"0100").unwrap());

        message.set_field(2, FieldValue::digits(pan.clone()).unwrap()).unwrap();
        message.set_field(3, FieldValue::digits("000000").unwrap()).unwrap();
        message.set_field(4, FieldValue::digits(amount.clone()).unwrap()).unwrap();
        message.set_field(11, FieldValue::digits(stan.clone()).unwrap()).unwrap();
        message.set_field(49, FieldValue::digits(currency.clone()).unwrap()).unwrap();

        let encoded = message.encode(&spec).unwrap();
        let parsed = Message::parse(&spec, &encoded).unwrap();

        prop_assert_eq!(parsed.mti(), message.mti());
        prop_assert_eq!(parsed.field(2).unwrap().as_digits(), Some(pan.as_str()));
        prop_assert_eq!(parsed.field(4).unwrap().as_digits(), Some(amount.as_str()));
        prop_assert_eq!(parsed.field(11).unwrap().as_digits(), Some(stan.as_str()));
        prop_assert_eq!(parsed.encode(&spec).unwrap(), encoded);
    }

    #[test]
    fn roundtrips_text_field(value in "[ -~]{40}") {
        let spec = Spec::builder("text")
            .field(FieldSpec::text(43, "Card Acceptor Name and Location", Length::fixed(40)))
            .build()
            .unwrap();

        let mut message = Message::new(Mti::parse(b"0100").unwrap());
        message.set_field(43, FieldValue::text(value.clone()).unwrap()).unwrap();

        let encoded = message.encode(&spec).unwrap();
        let parsed = Message::parse(&spec, &encoded).unwrap();

        prop_assert_eq!(parsed.field(43).unwrap().as_str(), Some(value.as_str()));
    }

    #[test]
    fn roundtrips_bcd_variable_field(pan in "[0-9]{1,19}") {
        let spec = Spec::builder("bcd")
            .field(
                FieldSpec::numeric(2, "PAN", Length::variable(19, Prefix::Bcd(2)))
                    .with_encoding(Encoding::Bcd),
            )
            .build()
            .unwrap();

        let mut message = Message::new(Mti::parse(b"0100").unwrap());
        message.set_field(2, FieldValue::digits(pan.clone()).unwrap()).unwrap();

        let encoded = message.encode(&spec).unwrap();
        let parsed = Message::parse(&spec, &encoded).unwrap();

        prop_assert_eq!(parsed.field(2).unwrap().as_digits(), Some(pan.as_str()));
        prop_assert_eq!(parsed.encode(&spec).unwrap(), encoded);
    }

    #[test]
    fn roundtrips_variable_text(value in "[ -~]{0,76}") {
        let spec = Spec::iso8583_1987();
        let mut message = Message::new(Mti::parse(b"0100").unwrap());
        message.set_field(45, FieldValue::text(value.clone()).unwrap()).unwrap();

        let encoded = message.encode(&spec).unwrap();
        let parsed = Message::parse(&spec, &encoded).unwrap();

        prop_assert_eq!(parsed.field(45).unwrap().as_str(), Some(value.as_str()));
    }
}
