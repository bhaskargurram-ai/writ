## What & why

<!-- one paragraph; link the issue -->

## Checklist

- [ ] `cargo test --workspace` passes locally
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean
- [ ] I edited only the crate(s)/paths this change owns (see WRIT_MASTER_BUILD_PLAN.md §5.2)
- [ ] Docs updated if behaviour changed
- [ ] **This PR contains no unearned assurance or performance claims** (no "bank grade", no "tamper-proof", no latency numbers outside published benchmarks — see the claim-discipline CI job)
- [ ] Commits are DCO signed off (`Signed-off-by: Name <email>`; `git commit -s`)

## Risk notes for reviewers

<!-- security-path change? ledger schema touch? say so loudly -->
