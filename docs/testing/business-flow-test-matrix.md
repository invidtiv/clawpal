# Business Flow Test Matrix

## Goal
After GUI-CLI-Core layering, business logic verification is core/CLI-first, with GUI focused on integration and UX wiring.

## Fast Local Gate (required before commit)
1. `cargo test -p clawpal-core`
2. `cargo test -p clawpal-cli`
3. `cargo build -p clawpal`

## Extended Local Gate (recommended before merge)
1. `cargo test -p clawpal --test install_api --test runtime_types --test commands_delegation`
2. `cargo run -p clawpal-cli -- instance list`
3. `cargo run -p clawpal-cli -- ssh list`

## Remote Gate (requires reachable `vm1`)
1. `cargo test -p clawpal --test remote_api -- --test-threads=1`

Expected notes:
- 4 tests are `ignored` in `remote_api` by design (manual/optional checks).
- Environment must allow outbound SSH to `vm1`.

## Layer Ownership
- `clawpal-core`: business rules, persistence, SSH registry, install/connect health logic.
- `clawpal-cli`: JSON contract and command routing.
- `src-tauri`: thin command delegation, state wiring, runtime event bridge.
- Frontend GUI: user interactions, rendering, invoke approval UX.

## Regression Priorities
1. Instance registry consistency (`instances.json` for local/docker/remote ssh).
2. SSH read/write correctness (must fail loudly on remote command errors).
3. Docker install behavior (no-op regressions blocked).
4. Doctor tool contract (`clawpal`/`openclaw` only).
