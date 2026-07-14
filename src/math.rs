use crate::types::Error;

/// Fixed-point scaling factor: 1e12 for 12 decimal precision.
pub const SCALE: i128 = 1_000_000_000_000;

// ── Core arithmetic helpers ─────────────────────────────────────

/// Multiplies `a * b` then divides by SCALE, checking overflow at each step.
fn scale_mul_div(a: i128, b: i128) -> Result<i128, Error> {
    a.checked_mul(b)
        .and_then(|p| p.checked_div(SCALE))
        .ok_or(Error::ArithmeticOverflow)
}

/// Divides `a` by `b` then multiplies by SCALE, checking overflow at each step.
fn scale_div_mul(a: i128, b: i128) -> Result<i128, Error> {
    a.checked_mul(SCALE)
        .and_then(|p| p.checked_div(b))
        .ok_or(Error::ArithmeticOverflow)
}

// ── Domain-specific functions ─────────────────────────────────────

/// Calculates accumulated rewards for a user's shares at the current index.
pub fn accumulated(shares: i128, index: i128) -> Result<i128, Error> {
    scale_mul_div(shares, index)
}

/// Calculates pending rewards: accumulated rewards minus already-recorded debt.
pub fn pending(shares: i128, index: i128, debt: i128) -> Result<i128, Error> {
    accumulated(shares, index)?
        .checked_sub(debt)
        .ok_or(Error::ArithmeticOverflow)
}

/// Calculates the index increase per share for a reward distribution.
pub fn reward_increase(amount: i128, total_shares: i128) -> Result<i128, Error> {
    scale_div_mul(amount, total_shares)
}
