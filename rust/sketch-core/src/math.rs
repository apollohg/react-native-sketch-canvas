//! Transcendental functions from the pure-Rust `libm`, so every platform computes the same bits.

pub(crate) use libm::{acos, atan2, cos, hypot, sin, tan};
