# RoadWatch Untrusted Image Ingestion Policy

Status: **security design / implementation pending local validation**

Every user-supplied image is untrusted. RoadWatch must never publish or directly serve the original upload.

## Threats checked

- misleading file extension or MIME type
- malformed/truncated image structures
- EXIF, XMP, IPTC and other metadata
- GPS/device/identity metadata leakage
- appended bytes after the expected image end
- image/file polyglots
- embedded archives, scripts or executable signatures
- suspicious ancillary chunks/segments
- duplicate exact files
- visually duplicated/re-encoded images
- unusually large dimensions/decompression bombs
- steganography indicators or anomalous pixel/channel statistics

Steganography detection is heuristic: a clean scan cannot prove that an image contains no hidden data. Therefore RoadWatch's primary defense is **decode and re-encode** accepted images into a new sanitized image rather than trusting or redistributing original bytes.

## Required pipeline

1. Enforce request/body byte limits before full buffering.
2. Detect format from file content (magic/signature), never extension alone.
3. Permit only a small image allowlist initially: JPEG and PNG.
4. Decode with a hardened image library under pixel/dimension/resource limits.
5. Reject malformed images and unexpected trailing/embedded payload structures where detectable.
6. Compute SHA-256 of the original privately for replay/abuse detection.
7. Compute a perceptual hash from decoded pixels for near-duplicate detection.
8. Inspect metadata for moderation/security signals, but do not copy it into the public derivative.
9. Run heuristic steganography/anomaly checks as a review signal, not as proof.
10. Re-encode decoded pixels into a fresh sanitized derivative with no unnecessary metadata.
11. Compute SHA-256 of the sanitized derivative.
12. Store originals only in a private quarantine area if operationally necessary; never expose the private object URI publicly.
13. Public serving uses only the sanitized derivative.
14. Evidence scoring occurs only after the ingestion/security checks succeed.

## Steganography rule

A steganography detector must never be represented as a perfect detector. Statistical checks can flag suspicious least-significant-bit patterns, channel anomalies, unexpected entropy, or tool-specific signatures, but sophisticated hidden content can evade detection.

Accordingly:
- suspicious result -> quarantine/manual review
- inconclusive result -> normal sanitized re-encode path
- original bytes -> never publicly served
- decoded/re-encoded derivative -> only publishable image form

## Privacy

GPS coordinates used for the camera report belong in structured RoadWatch report data. They should not remain embedded in a publicly served photo. Device model, serial-like identifiers, timestamps not required for evidence, author fields, editing history and thumbnails should be removed from public derivatives.

## Future hardening

- sandboxed image decoder worker
- CPU/memory/time quotas
- content-disarm-and-reconstruction worker
- malware scanner as an additional signal
- image parser fuzzing
- corpus tests with malformed/polyglot/trailing-data samples
- steganography test corpus
- quarantine retention/deletion policy
