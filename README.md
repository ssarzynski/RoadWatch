# RoadWatch

RoadWatch is an open-source, community-verified camera awareness platform for documenting publicly observable road and public-safety camera infrastructure and providing privacy-first proximity awareness.

## Mission

Build a free, transparent, auditable registry of ALPR, traffic, speed, red-light, toll, and public surveillance cameras while protecting contributor and driver privacy.

## Core principles

- **Open:** source code and public camera data are designed for community access.
- **Verified:** reports are evidence-backed and confidence-scored; popularity alone never makes a camera verified.
- **Privacy-first:** no permanent trip history by default; proximity processing should happen on-device where practical.
- **Auditable:** camera records retain provenance and verification history.
- **Safe:** RoadWatch documents publicly observable infrastructure. It is not for accessing, disabling, interfering with, or evading camera systems.
- **Vendor-independent:** open mapping is the default; proprietary map/navigation integrations are optional.

## MVP

1. Submit a camera report.
2. Check for spatial/record duplicates.
3. Attach evidence and provenance.
4. Calculate a verification state/confidence.
5. Store the camera and its history.
6. Query cameras near a coordinate.
7. Support privacy-preserving proximity alerts.

## Planned components

- `camera-registry/` — schema, migrations, geographic data model
- `verification-engine/` — evidence evaluation and confidence logic
- `roadwatch-api/` — reporting and nearby-camera API
- `roadwatch-mobile/` — future mobile/navigation companion
- `docs/` — architecture, verification, privacy and security specifications

## Status

**v0.1 — foundation / pre-alpha.** Interfaces and schemas will change.

## Contributing

Community participation is welcome. See `CONTRIBUTING.md` for reporting and verification expectations.

## License

Apache License 2.0 for source code. Imported datasets may have their own licenses and attribution requirements.
