use crate::types::Error;

pub const SCALE_FACTOR: i128 = 1_000_000_000_000;

pub fn accumulated(shares: i128, index: i128) -> Result<i128, Error> {
    shares
        .checked_mul(index)
        .and_then(|value| value.checked_div(SCALE_FACTOR))
        .ok_or(Error::ArithmeticOverflow)
}

pub fn pending(shares: i128, index: i128, debt: i128) -> Result<i128, Error> {
    accumulated(shares, index)?
        .checked_sub(debt)
        .ok_or(Error::ArithmeticOverflow)
}

pub fn reward_increase(amount: i128, total_shares: i128) -> Result<i128, Error> {
    amount
        .checked_mul(SCALE_FACTOR)
        .and_then(|value| value.checked_div(total_shares))
        .ok_or(Error::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_scaled_rewards() {
        assert_eq!(reward_increase(4_000, 4_000), Ok(SCALE_FACTOR));
        assert_eq!(accumulated(250, SCALE_FACTOR * 2), Ok(500));
        assert_eq!(pending(250, SCALE_FACTOR * 2, 100), Ok(400));
    }

    #[test]
    fn rejects_overflow() {
        assert_eq!(accumulated(i128::MAX, 2), Err(Error::ArithmeticOverflow));
        assert_eq!(
            reward_increase(i128::MAX, 1),
            Err(Error::ArithmeticOverflow)
        );
    }
}
