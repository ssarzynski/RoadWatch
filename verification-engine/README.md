# RoadWatch Verification Engine

Deterministic, auditable policy engine for camera-report verification.

## Important
The confidence value is a **policy/evidence score, not a statistical probability**. A camera cannot become verified from AI classification alone. Independent corroboration or authoritative public evidence is required.

## Run
```bash
cd verification-engine
cargo test
```

The initial duplicate candidate radius is 30 meters. This is intentionally a candidate threshold only; final deduplication must also consider road geometry, bearing, camera type, timestamps and evidence.
