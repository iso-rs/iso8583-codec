# Changelog

All notable changes to this project are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-10-23

Maintenance release. No wire-format or public API changes; this version is the
first one published through the OIDC trusted publishing workflow.

## [0.1.0] - 2026-10-22

### Added

- ISO 8583 message type indicator (`Mti`) parsing, construction and component
  accessors.
- Primary and optional secondary bitmaps for fields 2–128.
- Spec-driven field definitions with fixed, `LLVAR`, `LLLVAR` and custom
  variable lengths.
- ASCII, BCD and binary field encodings.
- ASCII and BCD length prefixes.
- Left and right padding for fixed-length fields.
- Built-in ISO 8583:1987 field table for fields 2–64.
- Parse and encode APIs with structured, field-aware errors.
- `no_std` support via `alloc`; `std` is an optional default feature.
- Unit, integration and property-based round-trip tests.
- Hex decoder example.
- Tag-driven crates.io publishing workflow with OIDC trusted publishing support.

[Unreleased]: https://github.com/iso-rs/iso8583-codec/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/iso-rs/iso8583-codec/releases/tag/v0.1.1
[0.1.0]: https://github.com/iso-rs/iso8583-codec/releases/tag/v0.1.0

