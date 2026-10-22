# iso8583-codec design

Status: accepted for `0.1.0` (2026-10-22)

## Goals

- A small, auditable ISO 8583 core codec with no transport or application
  logic.
- Correct handling of the wire format: MTI, primary/secondary bitmaps, fixed
  and variable fields, ASCII/BCD/binary encodings, and length prefixes.
- A spec model that can describe ISO 8583:1987 as well as private network
  profiles without forking the parser.
- `no_std + alloc`, zero runtime dependencies, no unsafe code.
- Errors that identify the field and the violated rule.

## Non-goals (0.1)

- TCP, TLS, MLI/length-header framing, connection management.
- Message matching, persistence, ISO 8583:1993 field semantics.
- EMV BER-TLV parsing, composite subfield decoding, PIN block cryptography.
- A derive macro. Specs are constructed from data at runtime.

These are deliberate boundaries. A codec that owns networking is harder to
test, audit and reuse, and every existing Rust crate that mixes the two pays for
it in dependency weight.

## Wire model

An ISO 8583 message body is:

```text
+--------+------------------+------------------+---------------------+
| MTI    | primary bitmap   | secondary bitmap | data fields 2..128  |
| 4 bytes| 8 bytes          | 8 bytes optional | variable            |
+--------+------------------+------------------+---------------------+
```

- The MTI is four ASCII digits. It is validated as digits but not restricted to
  known version/class/function/origin combinations.
- Bit 1 of the primary bitmap means "a secondary bitmap follows". Bit 1 of the
  secondary bitmap would mean a tertiary bitmap; this is rejected.
- Field numbers map to bits most-significant-bit first: field 1 is `0x80` of
  byte 0, field 2 is `0x40`, field 64 is `0x01` of byte 7, field 65 is `0x80` of
  secondary byte 0, and so on.
- A field has a length rule (`Fixed` or `Variable { max, prefix }`), a semantic
  kind (`Numeric`, `Text`, `Binary`), an encoding (`Ascii`, `Bcd`, `Binary`) and
  a padding rule for fixed lengths.
- For `Variable`, the prefix holds the number of characters (for ASCII/BCD) or
  bytes (for binary). ASCII prefixes are decimal digits; BCD prefixes pack
  decimal digits two per byte, with the final low nibble padded with zero.

## Architecture

```text
iso8583/
└── crates/
    └── iso8583-codec/
        └── src/
            ├── lib.rs       public exports and crate documentation
            ├── mti.rs       Mti: validation and component accessors
            ├── bitmap.rs    Bitmap: field-number to bit mapping
            ├── spec.rs      Encoding, FieldKind, Length, Prefix, FieldSpec,
            │                Spec, SpecBuilder, built-in 1987 table
            ├── field.rs     FieldValue: Text, Digits, Binary
            ├── message.rs   Message: parse/encode and the byte cursor
            └── error.rs     Error enum and Display/Error impls
```

The workspace exists so that follow-up crates (`iso8583-macros`,
`iso8583-tokio`) can share the version and dependency policy without changing
the core crate's location. The codec crate has no dependency on the workspace
beyond package metadata.

## Data model

`Message` owns a `BTreeMap<u8, FieldValue>`. The map gives deterministic
ascending field order, which the encoder needs for both the bitmap and the field
stream, without a separate ordering step.

`FieldValue` separates text, digits and raw bytes:

- `Text(String)` — printable ASCII (`0x20..=0x7E`).
- `Digits(String)` — ASCII digits, non-empty.
- `Binary(Vec<u8>)` — opaque bytes.

Digits are not stored as integers. ISO 8583 fields are fixed-width or
length-prefixed decimal strings; converting `000000010000` to `10000` and back
would silently change the wire representation. The string form also keeps field
width visible to callers.

The bitmap is not stored in `Message`. It is derived from the field keys on
encode and discarded on parse after the fields are read. This makes it
impossible for the bitmap and the field map to disagree.

## Parsing

1. Validate the spec (once per call; the built-in table is 63 entries).
2. Read the 4-byte MTI and validate its digits.
3. Read the 8-byte primary bitmap. If bit 1 is set, read the 8-byte secondary
   bitmap; reject tertiary bitmaps.
4. Iterate set bits in ascending order. For each field:
   - look up the field spec; an unknown field is an error (strict parsing);
   - read the length prefix if variable, validate it against `max`;
   - compute the byte count (`ceil(length / 2)` for BCD, `length` otherwise);
   - read exactly that many bytes;
   - decode to `Text`, `Digits` or `Binary`, validating digits and printable
     characters.
5. Reject trailing bytes.

The cursor is a small bounds-checked slice reader. Every read returns a slice
borrowed from the input; all allocations happen when a field value is built.
There is no indexing arithmetic that can escape the input slice.

Strict parsing is the default because a codec should not guess at malformed
network input. Lenient modes (skip unknown fields, allow trailing MAC bytes) can
be added as explicit options later without changing the default.

## Encoding

1. Validate the spec.
2. Build the bitmap from the field keys. Setting a field above 64 allocates the
   secondary bitmap and sets bit 1 automatically.
3. Write the MTI and bitmap.
4. Iterate fields in ascending order:
   - check that the value variant matches the field kind;
   - validate digits/text bytes;
   - pad fixed fields according to the spec;
   - validate the value against `max` for variable fields;
   - write the prefix (ASCII or BCD) and the encoded value.
5. Return the bytes.

`encode_to` encodes into a temporary buffer first. Callers never receive a
partially written message when encoding fails.

## Spec validation

`Spec::validate` is the contract between user-defined specs and the parser. It
rejects:

- empty names and empty field tables;
- field numbers outside `2..=128` or equal to the reserved `65`;
- duplicate field numbers;
- zero fixed lengths and zero variable maxima;
- prefixes outside one to three digits;
- variable maxima that the prefix cannot represent;
- padding on variable-length fields;
- incompatible kind/encoding pairs (`Text` + `Bcd`, `Binary` + `Ascii`, etc.);
- non-digit padding for numeric fields and non-printable padding for text.

The built-in ISO 8583:1987 table is validated by a unit test. `Spec::new`
validates immediately; `Spec::from_static` is cheap and validates on first
parse/encode.

## Error handling

`Error` is a non-exhaustive enum with context-bearing variants, including:

- `UnexpectedEof { needed, remaining }`
- `InvalidMtiDigit { index, byte }`
- `MissingSecondaryBitmap` / `UnexpectedSecondaryBitmap`
- `UnknownField { field }`
- `FieldTooLong { field, max, actual }`
- `InvalidFixedLength { field, expected, actual }`
- `InvalidLengthPrefix { field }`
- `InvalidDigit { field, byte }` / `InvalidTextByte { field, byte }`
- `InvalidSpec { field, reason }`
- `FieldTypeMismatch { field }`

`Display` is implemented in `core`. `std::error::Error` is implemented only
with the default `std` feature. The enum is `#[non_exhaustive]` so new
diagnostics can be added without a breaking change.

## Testing strategy

- Unit tests per module: MTI digits, bitmap mapping/secondary handling, spec
  validation, field value constructors, BCD prefix encode/decode.
- Integration tests (`tests/roundtrip.rs`): a full 15-field 1987 message, a
  secondary-bitmap message, BCD field and prefix handling, and negative cases
  (truncation, invalid digits, type mismatch, unknown fields, reserved fields).
- Property tests (`tests/property.rs`): random PANs, amounts, STANs, currencies,
  40-character text fields, variable text and BCD variable fields must survive
  encode/parse.
- CI: format, clippy with `-D warnings`, all tests, `no_std` build, rustdoc with
  `-D warnings`, and an MSRV check for the library.

The `0.1` test vectors are constructed from the public field definitions, not
copied from a network capture. That keeps the tests reproducible and avoids
shipping customer data.

## Compatibility policy

- `0.1.x` is the initial API. Breaking changes may happen before `1.0`.
- The crate follows SemVer once `1.0` is reached.
- New error variants are allowed because `Error` is non-exhaustive.
- New spec fields, encodings and options are additive.
- The built-in 1987 table is data; correcting a field name or a documented
  maximum is not considered an API break, but changing a wire default is.

## Follow-up design questions

- Should `0.2` model composite fields as a first-class `FieldValue::Composite`
  or as a separate `CompositeSpec` layer? The former makes field 43/54 easier to
  inspect; the latter keeps the core value type small. Likely a separate layer
  with an explicit decode call.
- Should transport framing live in `iso8583-tokio` only, or should there be a
  transport-agnostic `iso8583-framing` crate for MLI? Likely the latter, with
  Tokio depending on it.
- How should `no_alloc` mode represent values? A borrowed `&[u8]` message view
  is the likely answer, but it interacts with BCD decoding and padding.
