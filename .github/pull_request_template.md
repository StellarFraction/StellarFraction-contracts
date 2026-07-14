## Proposed Changes
Describe the contract modifications:
- What functions were added or modified in the Soroban contract?
- What issue does this PR resolve? (E.g., Closes #22)

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Security fix / Audit refinement
- [ ] Gas optimization modification

## Mathematical Model / Security Analysis
If modifying staking yield or fee logic, please explain the mathematical equations or safety guards implemented:
- **Math formula:** E.g., `Debt = Shares * AccReward`
- **Reentrancy / Overflow checks:** List security steps taken.

## Gas & Resource Footprint
Did this change impact CPU instructions or memory consumption?
- **Before:** [cycles/bytes]
- **After:** [cycles/bytes]

## Checklist
- [ ] I have formatted the code with `cargo fmt`.
- [ ] I have run `cargo clippy` and resolved all warnings.
- [ ] I have written unit tests in `src/test.rs` covering all new logic paths.
- [ ] All tests pass successfully when executing `cargo test`.
