//! Decode a hex-encoded ISO 8583 message with the built-in 1987 spec.
//!
//! The argument is the hex encoding of the raw message bytes, not of the
//! printable field text.
//!
//! ```text
//! cargo run -p iso8583-codec --example decode_hex -- 3038313000000000020000003030
//! ```

use std::env;

use iso8583_codec::{Message, Spec};

fn main() {
    let input = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: decode_hex <hex-encoded-message>");
        std::process::exit(2);
    });

    let bytes = hex::decode(input.trim()).unwrap_or_else(|error| {
        eprintln!("invalid hex: {error}");
        std::process::exit(2);
    });

    let spec = Spec::iso8583_1987();
    let message = Message::parse(&spec, &bytes).unwrap_or_else(|error| {
        eprintln!("invalid ISO 8583 message: {error}");
        std::process::exit(1);
    });

    println!("MTI: {}", message.mti());
    println!("Fields: {}", message.field_count());

    for (number, value) in message.fields() {
        let name = spec
            .field(number)
            .map(|field| field.name)
            .unwrap_or("<unknown>");

        println!("F{number:03} {name}: {value}");
    }
}
