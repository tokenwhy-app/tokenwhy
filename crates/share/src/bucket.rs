// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! The Worker holds the same edge tables (through `payload::schema`) and rejects any other value.

pub const COUNT: [u64; 32] = [
    0,
    1,
    2,
    5,
    10,
    25,
    50,
    100,
    250,
    500,
    1_000,
    2_500,
    5_000,
    10_000,
    25_000,
    50_000,
    100_000,
    250_000,
    500_000,
    1_000_000,
    2_500_000,
    5_000_000,
    10_000_000,
    25_000_000,
    50_000_000,
    100_000_000,
    250_000_000,
    500_000_000,
    1_000_000_000,
    2_500_000_000,
    5_000_000_000,
    10_000_000_000,
];

pub const SHARE: [u16; 8] = [0, 20, 50, 100, 250, 500, 750, 900];

pub const RATIO: [u16; 8] = [0, 100, 250, 500, 750, 1000, 1500, 2000];

pub fn count(v: u64) -> u64 {
    lower(&COUNT, v)
}

pub fn count_f(v: f64) -> u64 {
    count(if v.is_finite() && v > 0.0 {
        v as u64
    } else {
        0
    })
}

pub fn cents(usd: f64) -> u64 {
    count_f((usd * 100.0).floor())
}

/// `x` is compared to `edge / 1000` so 0.02 lands on 20, not on 0 through a rounding of `x * 1000`.
pub fn share(x: f64) -> u16 {
    lower_f(&SHARE, x)
}

pub fn ratio(x: f64) -> u16 {
    lower_f(&RATIO, x)
}

fn lower(edges: &[u64], v: u64) -> u64 {
    edges.iter().rev().copied().find(|e| *e <= v).unwrap_or(0)
}

fn lower_f(edges: &[u16], x: f64) -> u16 {
    if !x.is_finite() {
        return 0;
    }
    edges
        .iter()
        .rev()
        .copied()
        .find(|e| f64::from(*e) / 1000.0 <= x)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Edges and the $2.50 example from design section 2.1.
    #[test]
    fn values_land_on_the_lower_edge() {
        assert_eq!(count(0), 0);
        assert_eq!(count(4), 2);
        assert_eq!(count(2_499), 1_000);
        assert_eq!(count(2_500), 2_500);
        assert_eq!(count(9_999_999_999), 5_000_000_000);
        assert_eq!(count(u64::MAX), 10_000_000_000);
        assert_eq!(cents(2.50), 250);
        assert_eq!(cents(4.999), 250);
        assert_eq!(cents(5.0), 500);
        assert_eq!(cents(-1.0), 0);
        assert_eq!(share(0.0199), 0);
        assert_eq!(share(0.02), 20);
        assert_eq!(share(0.1), 100);
        assert_eq!(share(0.89), 750);
        assert_eq!(share(1.0), 900);
        assert_eq!(share(f64::NAN), 0);
        assert_eq!(ratio(0.099), 0);
        assert_eq!(ratio(1.0), 1000);
        assert_eq!(ratio(7.5), 2000);
    }
}
