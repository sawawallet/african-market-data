## What this changes

<!-- What is different afterwards, and why it was wrong before. -->

## Evidence

<!--
Verifying a venue's hours? Quote the schedule from the exchange's own rulebook,
with its phase names, and link it. Secondary summaries are what produced the
values already in the registry.

Adding an adapter? Link the source's terms of use.

Fixing a bug? The failing case before, passing after.
-->

## Checks

- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets`
- [ ] `cargo fmt --check`

<!--
No network is needed for any of these — the build is offline by default and
`.sqlx` holds the prepared query cache.
-->
