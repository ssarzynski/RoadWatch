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


## Compromised contributor device

RoadWatch MUST assume that a reporting phone may be compromised by spyware, a RAT, malicious accessibility service, rooted/jailbroken software, instrumentation, or another hostile local process.

Therefore client-provided data is evidence, not authority.

Potential attacks:
- spoofed GPS coordinates or impossible movement
- falsified timestamps and bearings
- replayed or substituted camera photographs
- theft/reuse of contributor credentials or pseudonymous tokens
- automated high-volume false reports
- manipulation of app requests after UI validation
- extraction of locally cached RoadWatch data
- malicious attempts to make one compromised device appear to be many independent observers

Required controls:
- never let one device or contributor verify a canonical camera
- server-side schema and range validation for every request
- spatial/temporal plausibility checks and impossible-travel detection
- exact and perceptual image replay detection
- independent-source requirements for verification
- rate limits and abuse quotas
- short-lived scoped authentication credentials where authentication is used
- rotate/revoke contributor credentials after suspected compromise
- do not treat device attestation as proof that a report is true
- avoid permanent hardware identifiers and invasive device fingerprinting
- minimize sensitive local caches and travel history
- signed application releases and verified update channels
- append-only verification history so later compromise cannot silently rewrite prior decisions
- quarantine anomalous evidence instead of promoting it

### Independence rule

Multiple submissions from the same device, credential lineage, image, source record, or strongly correlated session do not count as independent corroboration merely because they are separate HTTP requests.

Device-integrity or attestation signals, when available, may increase abuse-review confidence but MUST NOT become a requirement that excludes open/community clients or a substitute for evidence verification.

### Fail-safe assumption

A compromised client must not be able to convert untrusted client assertions into a `verified` camera record by itself. Verification remains a server-side evidence-policy decision.
