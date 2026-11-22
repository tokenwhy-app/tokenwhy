// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! `client`, behind the `net` feature, is the only code in the workspace that links an HTTP client.

pub mod bucket;
pub mod payload;
pub mod week;

#[cfg(feature = "net")]
pub mod client;

pub use payload::Payload;
pub use week::Week;
