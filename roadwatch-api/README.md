# RoadWatch API v0.1

Status: **implemented / awaiting local validation**

Initial routes:

- `GET /health`
- `POST /v1/reports`
- `GET /v1/cameras/nearby`

This first slice validates untrusted location/radius input and deliberately keeps report acceptance separate from verification.

The nearby route currently returns an empty collection until PostgreSQL/PostGIS persistence is connected. This is intentional: the API must not fabricate camera records.

## Local test

```bash
cargo fmt --manifest-path roadwatch-api/Cargo.toml -- --check
cargo clippy --manifest-path roadwatch-api/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path roadwatch-api/Cargo.toml
```
