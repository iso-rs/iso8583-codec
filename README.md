# iso8583-codec

A safe, spec-driven codec for [ISO 8583](https://www.iso.org/standard/79451.html)
financial transaction messages, written in Rust.

[![Crates.io](https://img.shields.io/crates/v/iso8583-codec.svg)](https://crates.io/crates/iso8583-codec)
[![docs.rs](https://docs.rs/iso8583-codec/badge.svg)](https://docs.rs/iso8583-codec)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

## Why another ISO 8583 crate?

Most existing Rust implementations are either tied to a desktop GUI, carry a
large runtime stack, or have not been maintained for years. This crate is a
focused core codec:

- **Spec-driven.** Field layouts are data, not hard-coded business logic, so
  the same codec works for ISO 8583:1987 and private network profiles.
- **No hidden state.** `Message::parse` and `Message::encode` are explicit and
  deterministic. The bitmap is derived from the fields.
- **Leading zeros are preserved.** Numeric values are stored as digit strings,
  never as integers, so a PAN or an amount survives a round trip unchanged.
- **`no_std` friendly.** The crate only needs `alloc`; `std` is an optional
  default feature used for the `std::error::Error` implementation.
- **No unsafe code.** The crate is `#![forbid(unsafe_code)]`.
- **Zero runtime dependencies.**

## Quick start

```toml
[dependencies]
iso8583-codec = "0.1"
```

```rust
use iso8583_codec::{FieldValue, Message, Mti, Spec};

let spec = Spec::iso8583_1987();

let mut message = Message::new(Mti::parse(b"0100")?);
message.set_field(2, FieldValue::digits("4242424242424242")?)?;
message.set_field(3, FieldValue::digits("000000")?)?;
message.set_field(4, FieldValue::digits("000000010000")?)?;
message.set_field(11, FieldValue::digits("000123")?)?;
message.set_field(49, FieldValue::digits("840")?)?;

let wire = message.encode(&spec)?;
let parsed = Message::parse(&spec, &wire)?;

assert_eq!(parsed.mti().as_str(), "0100");
assert_eq!(parsed.field(2).unwrap().as_digits(), Some("4242424242424242"));
assert_eq!(parsed.field(4).unwrap().as_digits(), Some("000000010000"));
# Ok::<(), iso8583_codec::Error>(())
```

## What is implemented

| Area | Support |
|------|---------|
| MTI | Four ASCII digits, component accessors |
| Bitmap | 64-bit primary, optional 64-bit secondary (fields 2–128) |
| Field lengths | Fixed, `LLVAR`, `LLLVAR`, custom prefix digit counts |
| Length prefixes | ASCII digits or BCD |
| Field encodings | ASCII, BCD, raw binary |
| Field kinds | Numeric, text, binary |
| Padding | Left/right, configurable per field |
| Built-in spec | ISO 8583:1987, fields 2–64 |
| Errors | Structured, field-aware error enum |
| Tests | Unit, integration, property-based round trips |

Parsed values preserve the wire representation: numeric fields keep leading
zeros, and text fields keep their on-wire padding. Normalize those at the
application layer if your profile treats them as insignificant.

## Defining a custom spec

Networks routinely change field lengths or encodings. Build a spec instead of
patching the codec:

```rust
use iso8583_codec::{Encoding, FieldSpec, Length, Padding, Prefix, Spec};

let spec = Spec::builder("acme-2026")
    .field(FieldSpec::numeric(2, "PAN", Length::llvar(19)))
    .field(
        FieldSpec::numeric(4, "Amount", Length::fixed(12))
            .with_encoding(Encoding::Bcd),
    )
    .field(
        FieldSpec::numeric(32, "Acquiring Institution", Length::variable(11, Prefix::Bcd(2)))
            .with_encoding(Encoding::Bcd),
    )
    .field(
        FieldSpec::text(43, "Card Acceptor", Length::fixed(40))
            .with_padding(Padding::Right(b' ')),
    )
    .build()?;
# Ok::<(), iso8583_codec::Error>(())
```

## `no_std`

```toml
[dependencies]
iso8583-codec = { version = "0.1", default-features = false }
```

The crate uses `alloc::collections::BTreeMap`, `alloc::string::String` and
`alloc::vec::Vec`. A future release may add a fixed-capacity mode for embedded
targets without an allocator.

## Scope and roadmap

The codec deliberately stops at the message boundary. It does **not** implement
TCP, TLS, MLI/length-header framing, message matching, or persistence. Those are
transport or application concerns and belong in separate crates. The full
design rationale lives in [docs/design.md](docs/design.md).

Planned work:

- `0.2` — ISO 8583:1993 field table (fields 65–128), EBCDIC encoding,
  composite/subfield definitions for fields 43 and 54, BER-TLV decoding for
  field 55.
- `0.3` — derive macro for typed message structs.
- `0.4` — Tokio transport crate with MLI framing.
- Later — `no_alloc` mode, serde support, fuzzing corpus.

## Development

```text
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo build --no-default-features   # no_std check
cargo fmt --all -- --check
```

Decode a hex-encoded message from the command line:

```text
cargo run -p iso8583-codec --example decode_hex -- 3038313000000000020000003030
```

```text
MTI: 0810
Fields: 1
F039 Response Code: 00
```

## Security

ISO 8583 parsers process untrusted network input. This crate validates MTI
digits, bitmap/length consistency, field lengths, numeric characters, printable
text, BCD nibbles and trailing bytes. It never indexes past the input slice.
Fuzzing is on the roadmap; until then, run the parser behind your normal
transport-level limits.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
