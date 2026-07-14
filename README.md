# StellarFraction Smart Contracts

Part of the **StellarFraction** ecosystem: The Soroban Rust smart contracts that power decentralized, gas-efficient USDC dividend distribution to fractional property shareholders.

---

## 🌐 StellarFraction Ecosystem Architecture

StellarFraction utilizes a multi-layered structure where physical real estate assets are tokenized, tracked, and rewarded using a combination of Stellar Classic Assets and Soroban Smart Contracts.

```
       +-------------------------------------------------+
       |             Client Browser (React UI)           |
       +-------+--------------------+----------------+---+
               |                    |                |
   (Wallet Connection)       (API Requests)    (SDK Triggers)
               |                    |                |
               v                    v                v
       +-------+-------+     +------+------+   +-----+------+
       |   Freighter   |     |   Node.js   |   |   Stellar  |
       |  / Albedo     |     |   Backend   |   |  Horizon/  |
       |  Wallet       |     |   API       |   |  Soroban   |
       +-------+-------+     +------+------+   +-----+------+
               |                    |                |
         (Signs Tx)            (DB Queries)     (Dividend Dist)
               |                    |                |
               v                    +--------------> |
   +-----------+-------------------------------------+-----------+
   |                       Stellar Network                       |
   |   - Property Deed Tokens (Classic Asset HORZ/OAKT/OMNI)     |
   |   - USDC Rental Dividend Distribution (Soroban Contract)    |
   +-------------------------------------------------------------+
```

---

## 💻 Role of this Repository

This repository hosts the **Rust-based smart contract** deployed to the Stellar Futurenet/Testnet.

### Key Objectives:
1. **Mathematical Yield Precision:** Distribute USDC rental payouts to stakers proportionally without rounding leakage.
2. **Gas-Efficient Execution:** Implement an **$O(1)$ reward distribution algorithm** to allow millions of stakers to claim rewards without causing transaction block timeouts.
3. **Multi-Property Isolation:** Maintain independent tokens, balances, reward indices, managers, and lifecycle controls for every property pool.

### 📐 The $O(1)$ Staking Pool Algorithm
To avoid looping through thousands of investor accounts in a single transaction (which would fail due to block gas limits), the contract maintains global accumulator indices:
- **Rent Accumulation:** When USDC rent is paid, the global index is updated:
  $$AccRewardPerShare_{new} = AccRewardPerShare_{old} + \frac{USDC_{amount} \times 10^{12}}{TotalShares_{staked}}$$
- **Investor Claims:** When an investor stakes, withdraws, or claims, the contract updates their personal ledger record:
  $$PendingAmount = \frac{UserShares \times AccRewardPerShare_{new}}{10^{12}} - UserDebt$$
- **Debt Adjustment:** To prevent double-claiming historical distributions, the investor's `UserDebt` is synchronized:
  $$UserDebt_{new} = \frac{UserShares \times AccRewardPerShare_{new}}{10^{12}}$$

### Multi-property pools

Initialization creates pool `0` from the original share and reward token arguments. The admin can then create additional isolated pools:

```text
create_pool(manager, share_token, reward_token) -> PoolId
deposit_into(pool_id, user, amount)
distribute_to(pool_id, sender, amount)
claim_from(pool_id, user) -> amount
withdraw_from(pool_id, user, amount)
claim_many(user, pool_ids) -> total_amount
```

Pool managers can rotate their manager address and pause or resume deposits and distributions. Claims and withdrawals remain available while paused so investors are never locked in. `claim_many` accepts at most 20 pool IDs to keep resource use bounded.

Read APIs include `get_pool`, `get_pool_count`, `get_position`, and `get_pool_pending`. Pool lifecycle and accounting operations emit `pool_new`, `manager`, `paused`, `deposit`, `distrib`, `claim`, and `withdraw` events for indexers.

### Compatibility and migration

The original `deposit`, `withdraw`, `distribute`, `claim`, `get_shares`, `get_debt`, and `get_pending` methods remain available and now delegate to pool `0`. Existing clients can therefore keep their original calls while new clients use the pool-scoped APIs. Deployments upgrading from storage created before multi-pool support should migrate their legacy totals and investor records into pool `0` before switching WASM; fresh deployments require no migration.

---

## 🛠️ Build and Testing Instructions

### Prerequisites
* **Rust Toolchain** (Stable v1.78+)
* **WASM Target Support**: `rustup target add wasm32-unknown-unknown`
* **Stellar CLI** (v22.0.0 or higher)

### Setup & Execution Commands

1. **Clone and navigate to the directory:**
   ```bash
   cd StellarFraction-contracts
   ```

2. **Execute smart contract unit tests:**
   ```bash
   cargo test
   ```
   *Runs precision, overflow, and multi-user distribution scenarios.*

3. **Compile the WASM bytecode target:**
   ```bash
   cargo build --target wasm32-unknown-unknown --release
   ```
   *Compiles target bytecode file to `/target/wasm32-unknown-unknown/release/stellarfraction_distribution.wasm`.*

4. **Verify WASM size optimization:**
   The output WASM is fully optimized to fit within the Soroban transaction fee bounds (configured with dynamic dead-code stripping inside `Cargo.toml`).

---

## 🤝 Contributing & Audits
Please consult [CONTRIBUTING.md](./CONTRIBUTING.md) for clippy guidelines, cargo formatting rules, and branch structures. Submit Pull Requests using the provided templates.

## 📄 License
This project is open-source under the terms of the MIT License. See [LICENSE](./LICENSE) for details.
