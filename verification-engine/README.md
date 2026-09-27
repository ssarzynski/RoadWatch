# RoadWatch Verification Engine

Status: **implemented / awaiting local validation**

This Rust crate contains deterministic, auditable verification and duplicate-candidate policy.

## Evidence model

RoadWatch uses integer evidence scores from 0–100. They are **policy scores, not probabilities**.

Two dimensions are deliberately separate:

- **Presence evidence score** — evidence that a physical camera installation exists at the mapped location.
- **Classification evidence score** — evidence supporting what that installation does.

AI classification contributes only to classification. It contributes **zero** to physical-presence verification.

Repeated evidence from the same source and evidence class is counted once. Multiple evidence classes from one source may add evidence, but they do not create independent corroboration.

Presence states:

- Unverified
- Probable
- High Confidence
- Verified

Verification requires strong evidence plus source independence. An authoritative dataset alone is not enough for final verification under the current v0.1 policy.

## Duplicate candidates

The engine uses a 30-meter spatial candidate boundary, then explicit geometry/evidence rules. Proximity alone never establishes that two reports describe the same installation.

## Local validation

Run from the repository root:

```bash
bash scripts/verify.sh
```

This runs formatting, Clippy with warnings denied, and tests for the verification engine and API.
