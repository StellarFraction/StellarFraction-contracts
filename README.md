# StellarFraction Smart Contracts

This repository contains the high-performance Rust-based smart contracts for the **StellarFraction** micro-investment platform, running on the Soroban smart contract platform on the Stellar network.

---

## Staking & Distribution Contract

The core contract is a proportional yield distribution system that handles rental income payouts (in USDC) to stakers holding physical property deed tokens (issued as custom Stellar Classic Assets).

### $O(1)$ Staking Pool Algorithm
To ensure the contract can support millions of stakers within the CPU limit of a single on-chain transaction block, we implement a scalable reward accumulator with reward debt tracking:
- **Accumulation:** When rental yield is paid, a global accumulated reward per share factor increases:
  $$A_{new} = A_{old} + \frac{USDC \times 10^{12}}{Shares_{total}}$$
- **Staker Debt:** Staker's claims are calculated using:
  $$Pending = \frac{StakerShares \times A_{new}}{10^{12}} - StakerDebt$$

---

## Directory Structure
- `src/lib.rs` - Main distribution contract logic.
- `src/test.rs` - Precision verification unit tests.
- `Cargo.toml` - Dependencies and target WASM configurations.

---

## Getting Started

### Prerequisites
- **Rust** (v1.78+)
- **Stellar CLI** (v22+)
  ```bash
  cargo install --locked stellar-cli
  ```

### Build wasm target
Compile the contract into WebAssembly:
```bash
cargo build --target wasm32-unknown-unknown --release
```

### Run Tests
Execute the unit tests to verify mathematical correctness:
```bash
cargo test
```

## Contributing
Please refer to [CONTRIBUTING.md](./CONTRIBUTING.md) for details on code style, formatting (`cargo fmt`), linting (`cargo clippy`), and PR review flows.

## License
MIT License - see the [LICENSE](./LICENSE) file for details.
