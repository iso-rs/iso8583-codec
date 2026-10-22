//! A safe, spec-driven codec for ISO 8583 financial transaction messages.
//!
//! This crate implements the wire format of ISO 8583:1987 messages: the message
//! type indicator (MTI), primary and secondary bitmaps, fixed and variable
//! length fields, and the common ASCII, BCD and binary field encodings.
//!
//! It deliberately does **not** implement transport concerns such as TCP,
//! length headers (MLI) or TLS. Those belong in a separate crate so that the
//! codec can be used with any transport, including `no_std` embedded links.
//!
//! # Quick start
//!
//! ```
//! use iso8583_codec::{FieldValue, Message, Mti, Spec};
//!
//! let spec = Spec::iso8583_1987();
//! let mut message = Message::new(Mti::parse(b"0100").unwrap());
//! message
//!     .set_field(2, FieldValue::digits("4242424242424242").unwrap())
//!     .unwrap();
//! message
//!     .set_field(4, FieldValue::digits("000000010000").unwrap())
//!     .unwrap();
//!
//! let bytes = message.encode(&spec).unwrap();
//! let parsed = Message::parse(&spec, &bytes).unwrap();
//!
//! assert_eq!(parsed.mti().as_str(), "0100");
//! assert_eq!(parsed.field(2).unwrap().as_digits(), Some("4242424242424242"));
//! ```
//!
//! # Field values
//!
//! ISO 8583 fields are not self-describing: the specification attached to a
//! message decides how many bytes a field occupies and how those bytes are
//! encoded. [`FieldValue`] keeps numeric fields as digit strings so that
//! leading zeros (for example in a PAN or an amount) are never lost.
//!
//! # `no_std`
//!
//! The crate only needs `alloc`. Disable the default `std` feature to use it in
//! `no_std` environments:
//!
//! ```toml
//! iso8583-codec = { version = "0.1", default-features = false }
//! ```
//!
//! # Specification coverage
//!
//! The built-in [`Spec::iso8583_1987`] table covers fields 2 through 64 of
//! ISO 8583:1987. Fields 43, 54 and 55 are exposed as text in this release;
//! structured subfield and EMV TLV decoding is planned for a later version.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

extern crate alloc;

#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
struct ReadmeDoctests;

pub mod bitmap;
pub mod error;
pub mod field;
pub mod message;
pub mod mti;
pub mod spec;

pub use bitmap::Bitmap;
pub use error::Error;
pub use field::FieldValue;
pub use message::Message;
pub use mti::Mti;
pub use spec::{Encoding, FieldKind, FieldSpec, Length, Padding, Prefix, Spec, SpecBuilder};
