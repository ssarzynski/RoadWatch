# RoadWatch Architecture — v0.1

## Components
1. **Camera Registry** — PostgreSQL/PostGIS canonical camera records, observations and history.
2. **Verification Engine** — deterministic evidence rules plus optional advisory classifiers.
3. **RoadWatch API** — reporting, evidence submission, nearby queries and verification status.
4. **Mobile Companion** — later phase; on-device proximity and alerts with open-map default.

## Data flow
Report -> validation -> duplicate candidate search -> evidence evaluation -> verification event -> canonical registry -> nearby query -> client-side proximity evaluation.

## Design constraints
- PostgreSQL + PostGIS is the source of truth.
- History is append-oriented; important provenance is not overwritten.
- Stable UUIDs identify physical camera records.
- Geographic queries use indexed geometry/geography.
- Public API responses do not expose contributor identity.
- OpenStreetMap/MapLibre is the preferred open map path.
- Google/Waze integrations are optional adapters, not core dependencies.
