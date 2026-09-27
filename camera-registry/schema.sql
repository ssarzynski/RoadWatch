CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TYPE camera_status AS ENUM (
  'unverified','probable','verified','stale','removed_pending','removed','disputed'
);

CREATE TABLE cameras (
  camera_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  location GEOGRAPHY(POINT,4326) NOT NULL,
  road_name TEXT,
  bearing_degrees SMALLINT CHECK (bearing_degrees BETWEEN 0 AND 359),
  camera_type TEXT NOT NULL DEFAULT 'unknown',
  manufacturer TEXT,
  model TEXT,
  operator_name TEXT,
  status camera_status NOT NULL DEFAULT 'unverified',
  confidence NUMERIC(5,4) NOT NULL DEFAULT 0 CHECK (confidence BETWEEN 0 AND 1),
  first_seen_at TIMESTAMPTZ,
  last_seen_at TIMESTAMPTZ,
  last_verified_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX cameras_location_gix ON cameras USING GIST(location);

CREATE TABLE reports (
  report_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  camera_id UUID REFERENCES cameras(camera_id),
  observed_location GEOGRAPHY(POINT,4326) NOT NULL,
  claimed_type TEXT,
  observed_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reports_location_gix ON reports USING GIST(observed_location);

CREATE TABLE evidence (
  evidence_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  report_id UUID NOT NULL REFERENCES reports(report_id),
  evidence_type TEXT NOT NULL,
  source_uri TEXT,
  content_hash TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE verification_events (
  event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  camera_id UUID NOT NULL REFERENCES cameras(camera_id),
  prior_status camera_status,
  new_status camera_status NOT NULL,
  confidence NUMERIC(5,4) CHECK (confidence BETWEEN 0 AND 1),
  reason TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
