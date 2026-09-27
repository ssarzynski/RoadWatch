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


## Untrusted image/evidence uploads

Image evidence is an active attack surface. Threats include metadata leakage, parser exploits, decompression bombs, malformed files, polyglots/appended payloads, embedded content, replayed images, near-duplicate re-encodes, and steganography.

Controls:
- original uploads are never publicly served
- strict byte/pixel/dimension/resource limits
- content-signature validation rather than extension trust
- JPEG/PNG allowlist initially
- exact SHA-256 and perceptual-hash checks
- metadata inspection followed by stripping
- decode/re-encode content disarm before public use
- heuristic steganography/anomaly checks as review signals only
- private quarantine for suspicious originals
- public API never exposes private object locations
- sandboxed decoder and fuzz/malformed-image corpus are required hardening targets

See `security/IMAGE-INGESTION.md`.
