# Storage Layout & Gas Optimizations

This contract is deliberate about **which storage tier** each value lives in and
about keeping per-operation cost **constant**. This document explains the
choices and why they save gas and rent.

## Soroban storage tiers

Soroban exposes three storage tiers, each with different rent and lifetime
characteristics:

| Tier | Lifetime | Best for |
|------|----------|----------|
| **Instance** | Tied to the contract instance; shares one rent bucket | Small, always-needed global config read on nearly every call. |
| **Persistent** | Independent entries, individually rent-managed | Per-user data whose count grows with the number of stakers. |
| **Temporary** | Cheap, can expire | Values safe to lose after a short window (unused here). |

The contract maps each piece of state to the tier that minimizes rent while
preserving correctness. Details for each follow in the next sections.

## Instance storage — global config

These keys are **fixed in count** (they don't grow with users) and are read on
almost every call, so they live in **instance** storage and share a single rent
bucket:

- `Admin`, `ShareToken`, `RewardToken`
- `AccRewardPerShare`, `TotalShares`
- `Initialized`, `Paused`
- `MinimumDeposit`, `LockupDuration`
- `ManagementFeeBps`, `FeeCollector`

**Why it saves gas:** bundling all singletons into the instance bucket means the
contract renews **one** rent entry instead of a dozen separate ones, and reads
of hot config values (e.g. `TotalShares` on every `distribute`) stay cheap. The
whole set is a handful of small scalars/addresses, so the instance footprint
stays tiny.
