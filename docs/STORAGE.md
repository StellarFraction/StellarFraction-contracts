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
