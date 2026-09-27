# RoadWatch Image Security Test Corpus

These fixtures are generated in unit tests rather than storing potentially confusing or unsafe binary samples in the repository.

## Required cases

| Case | Expected decision |
|---|---|
| Valid minimal PNG | AcceptSanitized |
| Non-image text bytes | Reject |
| Malformed PNG-like bytes | Reject |
| Upload over byte ceiling | Reject |
| Valid PNG plus trailing payload bytes | Quarantine |
| Exact replay | Same SHA-256 |
| Same decoded pixels | Same perceptual hash |

## Expansion cases for local/server validation

- valid JPEG with bytes appended after EOI
- PNG with unusual ancillary metadata chunks
- EXIF GPS/device metadata followed by sanitized re-encode
- very large dimensions with small compressed representation
- truncated JPEG and PNG
- corrupted chunk/segment lengths
- duplicate image recompressed at different quality
- visually modified image with perceptual distance measurement
- known-benign steganography test fixtures for detector benchmarking
- image/polyglot research fixtures kept in a non-public quarantine test directory

## Test rule

A scanner result must never be described as proving that steganography is absent. RoadWatch's durable boundary is strict decode plus fresh re-encoding, with suspicious input quarantined and originals never publicly served.
