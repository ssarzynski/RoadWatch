# Camera Verification Specification — v0.1

## Principle
No single user, AI model, popularity vote, or uncorroborated source can independently establish a camera as verified.

## States
- `unverified` — newly reported; insufficient corroboration
- `probable` — credible evidence exists but verification threshold is not met
- `verified` — independent evidence satisfies verification policy
- `stale` — not reconfirmed within the applicable review interval
- `removed_pending` — credible removal report awaiting corroboration
- `removed` — removal independently corroborated
- `disputed` — credible conflicting evidence requires review

## Evidence types
- geotagged field observation
- photograph of publicly observable equipment
- independent second observation
- authoritative government/open dataset
- OpenStreetMap/open geospatial record
- public procurement/deployment record
- historical observation

AI/computer vision classification is advisory evidence only and cannot independently verify a camera.

## Required checks
1. Validate coordinates and timestamp.
2. Search for nearby duplicates.
3. Determine whether the observation refers to an existing, replaced, moved, or new device.
4. Evaluate evidence provenance.
5. Check independence of corroborating observations.
6. Determine type/manufacturer only to the confidence supported by evidence.
7. Record the decision and reasons in an append-only verification event.

## Anti-abuse
Repeated submissions, impossible travel patterns, reused media, coordinated voting, bulk anomalies and suspicious contributor behavior are signals for review, not automatic proof of fraud.

## Reverification
Verification is not permanent. Records carry `last_seen_at` and `last_verified_at`. Stale cameras are downgraded rather than silently deleted.

## Removal
A single removal report does not erase a camera. Removal follows corroboration and preserves historical records.
