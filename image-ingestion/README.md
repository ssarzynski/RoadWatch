# RoadWatch Image Ingestion

Status: **implemented / awaiting local validation**

This crate is the first content-disarm boundary for untrusted evidence photos.

Current controls:
- 12 MiB upload limit
- JPEG/PNG content allowlist
- content-based format detection
- full image decode
- 8192x8192 per-axis limit
- 40 million pixel limit
- SHA-256 before and after sanitization
- fresh re-encoding without intentionally copying original metadata/container segments
- advisory anomaly flags

The anomaly layer does **not** claim to prove the absence of steganography.

Planned hardening:
- sandboxed decoder worker
- perceptual hashing
- exact/trailing-container validation
- richer statistical steganography indicators
- malformed/polyglot corpus
- CPU/memory/time quotas
- fuzz testing

Run through the repository root validation script:

```bash
bash scripts/verify.sh
```
