# RoadWatch API v0.1 Contract

Status: **implemented specification / awaiting local validation**

Base path: `/v1`

## Privacy rules

Public responses MUST NOT expose contributor token hashes, private evidence object URIs, raw upload metadata, or internal abuse signals. Route/travel history is not stored by default.

## POST /v1/reports

Creates a report for a publicly observable camera installation.

Request:
```json
{
  "latitude": 33.749,
  "longitude": -84.388,
  "bearing_degrees": 90,
  "claimed_function": "alpr",
  "claimed_manufacturer": "unknown",
  "observed_at": "2026-09-27T16:00:00Z"
}
```

Validation:
- latitude: -90..90
- longitude: -180..180
- bearing: 0..359 when supplied
- function must be a supported enum
- server calculates nearby duplicate candidates
- contributor identity is represented only by a salted pseudonymous token hash

Response: `202 Accepted`

The response may include a candidate camera ID, but acceptance does **not** mean verification.

## POST /v1/reports/{report_id}/evidence

Adds evidence to an existing report.

Accepted evidence classes initially:
- field observation
- sanitized photo
- authoritative public dataset
- open geospatial record
- public procurement/deployment record
- AI classification (advisory only)

Uploads must be validated by content, transcoded where appropriate, stripped of unnecessary metadata before public serving, hashed, and size-limited.

## POST /v1/cameras/{camera_id}/observations

Records `present`, `absent`, or `unsure`.

A single contributor cannot verify or remove a canonical camera.

## GET /v1/cameras/nearby

Query parameters:
- `lat` required
- `lon` required
- `radius_m` required, server-capped

Database search uses PostGIS `ST_DWithin`. The client performs additional bearing/approach filtering before issuing a driving alert.

Public result fields:
- camera_id
- latitude / longitude
- road_name
- bearing_degrees
- camera_function
- manufacturer/model when sufficiently supported
- status
- presence_evidence_score
- classification_evidence_score
- last_seen_at
- last_verified_at

## GET /v1/cameras/{camera_id}

Returns the public canonical camera record and non-private provenance summary.

## GET /v1/cameras/{camera_id}/history

Returns append-oriented public status/history events. Private moderation data and contributor identifiers are excluded.

## GET /v1/cameras/{camera_id}/verification

Returns the current rule version, evidence-score dimensions and an explainable public reason summary.

## GET /v1/export.geojson

Exports permitted public records with required source/license attribution. OSM-derived data must retain ODbL-required attribution/licensing; RoadWatch must not relabel imported ODbL data as CC0.

## Verification dimensions

RoadWatch deliberately separates:

1. **Presence/location evidence** — does a camera installation exist here?
2. **Classification evidence** — what does it actually do?

A location can therefore be verified while its function remains `unknown`. AI classification may support the second dimension but can never independently establish canonical verification.

## Driving safety

Reporting flows must not encourage photo capture or detailed interaction while driving. A moving user may create a minimal location marker; evidence/details should be completed while stopped or post-trip.


## Evidence storage trust boundary

Evidence images use two physically/logically separate trust zones:

1. **Private quarantine** — original hostile upload bytes, short-lived and never publicly addressable.
2. **Sanitized evidence** — freshly decoded/re-encoded derivatives. Only cleared sanitized derivatives may become public evidence.

Public API types are explicit allowlists and MUST NOT serialize database evidence rows directly. Public responses must never contain `quarantine_object_key`, original filenames, raw metadata, contributor identifiers, moderation/abuse signals, credentials, or internal storage paths.

A sanitized image delivery URL, when implemented, must be resolved server-side from the sanitized evidence record rather than exposing the underlying storage key as general evidence metadata.

If sanitization or sanitized storage fails, the evidence is not eligible for verification scoring.
