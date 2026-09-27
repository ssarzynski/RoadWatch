CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TYPE camera_status AS ENUM (
  'unverified','probable','high_confidence','verified',
  'stale','removed_pending','removed','disputed'
);

CREATE TYPE camera_function AS ENUM (
  'alpr','speed_enforcement','red_light_enforcement','traffic_monitoring',
  'tolling','public_surveillance','parking_municipal','unknown'
);

CREATE TYPE observation_result AS ENUM ('present','absent','unsure');

CREATE TABLE sources (
  source_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name TEXT NOT NULL,
  source_type TEXT NOT NULL,
  source_url TEXT,
  license_identifier TEXT,
  retrieved_at TIMESTAMPTZ,
  content_sha256 TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE cameras (
  camera_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  location GEOGRAPHY(POINT,4326) NOT NULL,
  road_name TEXT,
  bearing_degrees SMALLINT CHECK (bearing_degrees BETWEEN 0 AND 359),
  camera_function camera_function NOT NULL DEFAULT 'unknown',
  manufacturer TEXT,
  model TEXT,
  operator_name TEXT,
  status camera_status NOT NULL DEFAULT 'unverified',
  presence_evidence_score SMALLINT NOT NULL DEFAULT 0
    CHECK (presence_evidence_score BETWEEN 0 AND 100),
  classification_evidence_score SMALLINT NOT NULL DEFAULT 0
    CHECK (classification_evidence_score BETWEEN 0 AND 100),
  first_seen_at TIMESTAMPTZ,
  last_seen_at TIMESTAMPTZ,
  last_verified_at TIMESTAMPTZ,
  replaced_by_camera_id UUID REFERENCES cameras(camera_id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX cameras_location_gix ON cameras USING GIST(location);

CREATE TABLE reports (
  report_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  camera_id UUID REFERENCES cameras(camera_id),
  contributor_token_hash TEXT,
  observed_location GEOGRAPHY(POINT,4326) NOT NULL,
  claimed_function camera_function NOT NULL DEFAULT 'unknown',
  claimed_manufacturer TEXT,
  claimed_model TEXT,
  bearing_degrees SMALLINT CHECK (bearing_degrees BETWEEN 0 AND 359),
  observed_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reports_location_gix ON reports USING GIST(observed_location);

CREATE TABLE evidence (
  evidence_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  report_id UUID NOT NULL REFERENCES reports(report_id) ON DELETE CASCADE,
  source_id UUID REFERENCES sources(source_id),
  evidence_type TEXT NOT NULL,
  -- Private quarantine key is server-internal and MUST NEVER be serialized by public APIs.
  quarantine_object_key TEXT,
  -- Sanitized derivative is the only image object eligible for public serving.
  sanitized_object_key TEXT,
  original_sha256 TEXT,
  sanitized_sha256 TEXT,
  perceptual_hash TEXT,
  ingestion_decision TEXT NOT NULL DEFAULT 'pending'
    CHECK (ingestion_decision IN ('pending','accepted_sanitized','quarantined','rejected')),
  eligible_for_scoring BOOLEAN NOT NULL DEFAULT false,
  metadata_stripped BOOLEAN NOT NULL DEFAULT false,
  captured_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE observations (
  observation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  camera_id UUID NOT NULL REFERENCES cameras(camera_id),
  contributor_token_hash TEXT,
  result observation_result NOT NULL,
  observed_location GEOGRAPHY(POINT,4326),
  observed_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX observations_camera_time_idx ON observations(camera_id, observed_at DESC);

CREATE TABLE verification_events (
  event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  camera_id UUID NOT NULL REFERENCES cameras(camera_id),
  prior_status camera_status,
  new_status camera_status NOT NULL,
  presence_score_before SMALLINT CHECK (presence_score_before BETWEEN 0 AND 100),
  presence_score_after SMALLINT CHECK (presence_score_after BETWEEN 0 AND 100),
  classification_score_before SMALLINT CHECK (classification_score_before BETWEEN 0 AND 100),
  classification_score_after SMALLINT CHECK (classification_score_after BETWEEN 0 AND 100),
  rule_version TEXT NOT NULL,
  explanation JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Public API responses must never expose contributor_token_hash,
-- quarantine_object_key, raw upload metadata, or other private moderation fields.
-- Public image delivery must resolve sanitized_object_key server-side; object storage
-- bucket/key details should not be serialized as evidence metadata.
