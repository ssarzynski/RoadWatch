# Threat Model — v0.1

## Protected assets
Camera-registry integrity, evidence provenance, contributor privacy, API availability and release integrity.

## Primary threats
- fabricated or malicious reports
- Sybil/collusive verification
- duplicate/replayed photographs
- location-history collection
- contributor deanonymization
- API scraping/abuse that harms service availability
- dependency/supply-chain compromise
- unauthorized database modification

## Baseline controls
- least-privilege service accounts
- parameterized database access
- rate limits and abuse controls
- append-only verification audit events
- media hashing for duplicate detection
- server-side validation
- secrets never committed to source
- dependency scanning and CI tests
- minimize collection of contributor/device identifiers
- no permanent driver trip history by default
- strip unnecessary image metadata before public publication

## Explicit non-goals
RoadWatch will not provide capabilities to access camera feeds, compromise systems, disable equipment, interfere with infrastructure, or identify individual motorists.
