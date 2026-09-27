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
CREATE INDEX reports_contributor_created_idx
  ON reports(contributor_token_hash, created_at DESC)
  WHERE contributor_token_hash IS NOT NULL;

-- Append-only private moderation audit. Stores policy outcomes/signals only;
-- never raw contributor credentials or reconstructed route history.
CREATE TABLE report_risk_assessments (
  assessment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  report_id UUID NOT NULL REFERENCES reports(report_id) ON DELETE RESTRICT,
  policy_version TEXT NOT NULL,
  risk_score SMALLINT NOT NULL CHECK (risk_score BETWEEN 0 AND 100),
  disposition TEXT NOT NULL CHECK (disposition IN ('normal','review','quarantine')),
  independent_weight_allowed BOOLEAN NOT NULL,
  signals TEXT[] NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX report_risk_report_created_idx
  ON report_risk_assessments(report_id, created_at DESC);

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

CREATE INDEX evidence_original_sha256_idx
  ON evidence(original_sha256)
  WHERE original_sha256 IS NOT NULL;
CREATE INDEX evidence_perceptual_hash_created_idx
  ON evidence(perceptual_hash, created_at DESC)
  WHERE perceptual_hash IS NOT NULL;

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


-- Q Ledger stores cryptographic commitments only. Never put private evidence,
-- contributor identifiers, raw GPS trails, or storage keys in this table.
CREATE TABLE q_ledger_events (
  sequence BIGINT PRIMARY KEY CHECK (sequence >= 0),
  event_type TEXT NOT NULL,
  subject_commitment TEXT NOT NULL CHECK (length(subject_commitment) = 64),
  previous_event_hash TEXT NOT NULL CHECK (length(previous_event_hash) = 64),
  event_hash TEXT NOT NULL UNIQUE CHECK (length(event_hash) = 64),
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE verification_events
  ADD COLUMN subject_commitment TEXT CHECK (subject_commitment IS NULL OR length(subject_commitment) = 64),
  ADD COLUMN ledger_sequence BIGINT REFERENCES q_ledger_events(sequence),
  ADD COLUMN ledger_event_hash TEXT CHECK (ledger_event_hash IS NULL OR length(ledger_event_hash) = 64);

CREATE UNIQUE INDEX verification_events_ledger_sequence_uidx
  ON verification_events(ledger_sequence)
  WHERE ledger_sequence IS NOT NULL;


CREATE TABLE q_ledger_checkpoints (
  checkpoint_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tree_size BIGINT NOT NULL CHECK (tree_size >= 0),
  merkle_root TEXT NOT NULL CHECK (length(merkle_root) = 64),
  last_event_hash TEXT NOT NULL CHECK (length(last_event_hash) = 64),
  created_at TIMESTAMPTZ NOT NULL,
  signer_public_key TEXT NOT NULL CHECK (length(signer_public_key) = 64),
  signature TEXT NOT NULL UNIQUE CHECK (length(signature) = 128),
  UNIQUE (tree_size, merkle_root)
);

CREATE TABLE q_ledger_witnesses (
  witness_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  checkpoint_id UUID NOT NULL REFERENCES q_ledger_checkpoints(checkpoint_id) ON DELETE RESTRICT,
  witness_public_key TEXT NOT NULL CHECK (length(witness_public_key) = 64),
  witnessed_at TIMESTAMPTZ NOT NULL,
  witness_signature TEXT NOT NULL UNIQUE CHECK (length(witness_signature) = 128),
  UNIQUE (checkpoint_id, witness_public_key)
);

-- A witness key represents an independent trust domain. Do not store witness
-- private keys in the RoadWatch application database.


-- Preserve conflicting signed checkpoints as evidence. Never auto-delete or
-- auto-resolve these rows merely because a later checkpoint appears normal.
CREATE TABLE q_ledger_conflicts (
  conflict_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  signer_public_key TEXT NOT NULL CHECK (length(signer_public_key) = 64),
  tree_size BIGINT NOT NULL CHECK (tree_size >= 0),
  first_merkle_root TEXT NOT NULL CHECK (length(first_merkle_root) = 64),
  second_merkle_root TEXT NOT NULL CHECK (length(second_merkle_root) = 64),
  first_signature TEXT NOT NULL CHECK (length(first_signature) = 128),
  second_signature TEXT NOT NULL CHECK (length(second_signature) = 128),
  detected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  resolution_state TEXT NOT NULL DEFAULT 'open'
    CHECK (resolution_state IN ('open', 'investigating', 'explained', 'confirmed_equivocation')),
  UNIQUE (signer_public_key, tree_size, first_merkle_root, second_merkle_root)
);
