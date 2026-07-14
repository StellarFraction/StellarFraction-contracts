# StellarFraction Distribution — Contract API

The `DistributionContract` lets holders of a fractional real-estate **share
token** stake their shares and earn a proportional cut of rental yield paid in
a **reward token** (e.g. USDC). Reward accounting is **O(1) per operation**: a
running `acc_reward_per_share` accumulator means a distribution costs the same
whether one staker or ten thousand are in the pool.

- **Contract version:** `0.1.0`
- **SDK:** `soroban-sdk 22`
- **Precision:** reward-per-share is scaled by `1e12` (`SCALE_FACTOR`); all
  integer division **rounds down**, so the pool can never pay out more than was
  deposited.

## Roles

| Role | Capability |
|------|------------|
| **Admin** | Configures the contract: pause, fees, lockups, stake limits, admin transfer, token recovery. Set once at `initialize`. |
| **Staker** | Deposits/withdraws share tokens and claims accrued dividends. |
| **Fee collector** | Optional address that receives skimmed management fees. |

---

## Initialization

### `initialize(admin, share_token, reward_token) -> Result<(), Error>`

Sets the admin and the two token addresses, and zeroes the accumulators.

- Requires `admin.require_auth()` so a third party cannot front-run deployment
  and appoint themselves admin.
- Fails with `AlreadyInitialized` if called twice.

```rust
client.initialize(&admin, &share_token_id, &reward_token_id);
```

---

## Staking

### `deposit(user, amount) -> Result<(), Error>`

Stakes `amount` share tokens, transferring them into the contract's custody.

- Requires `user.require_auth()`.
- Rejected when the contract is **paused** (`ContractPaused`).
- Enforces `MinimumDeposit` (`BelowMinimumDeposit`) and the per-user stake cap
  (`ExceedsMaxStake`).
- **Auto-claims** any pending rewards first, so an existing position is never
  silently overwritten.
- If a lockup is configured, each deposit **refreshes** the position's unlock
  timestamp to `now + lockup_duration`.

### `withdraw(user, amount) -> Result<(), Error>`

Unstakes `amount` share tokens back to the user.

- Requires `user.require_auth()`.
- Fails with `InsufficientShares` if `amount` exceeds the staked balance.
- Fails with `StillLocked` while the position's lockup window has not elapsed.
- **Auto-claims** pending rewards as part of the withdrawal.
- A full withdrawal clears the position's share and debt storage entries.

