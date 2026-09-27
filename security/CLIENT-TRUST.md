# RoadWatch Client Trust Policy

RoadWatch mobile clients are intentionally treated as untrusted.

## Server-authoritative rules

The server, not the phone, decides:
- whether coordinates and bearings are valid
- whether a report is a duplicate candidate
- whether evidence is safe enough to store
- whether observations are independent
- evidence scores
- camera status
- verification, dispute, stale, and removal transitions

The client may propose facts. It cannot declare them verified.

## Signals from a report

Treat these as claims/signals:
- latitude/longitude
- GPS accuracy
- bearing
- device-local timestamp
- camera type/manufacturer/model
- image capture time
- contributor identity/token
- optional device-integrity signal

Cross-check where practical against:
- server receipt time
- previous pseudonymous contributor observations
- road geometry
- physical distance/time feasibility
- duplicate/replayed imagery
- independent observations
- authoritative/open datasets

## Privacy boundary

Anti-abuse controls should not become a surveillance system for contributors.

RoadWatch should not require:
- permanent advertising identifiers
- IMEI/serial numbers
- contact lists
- unrelated application inventory
- continuous route-history upload

Prefer rotating pseudonymous identifiers and minimum necessary telemetry.

## Compromise response

When a contributor credential/device is suspected compromised:
1. stop its submissions from contributing independent verification weight;
2. preserve existing append-only audit history;
3. revoke/rotate scoped credentials when applicable;
4. review correlated reports for replay/automation;
5. do not automatically delete legitimate historical observations solely because a later compromise occurred.
