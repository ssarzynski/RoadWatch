//! Deterministic RoadWatch verification policy.
//! AI classifiers may provide advisory evidence, but cannot independently verify a camera.

pub mod candidate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    FieldObservation,
    Photo,
    IndependentObservation,
    GovernmentDataset,
    OpenGeospatialRecord,
    PublicProcurementRecord,
    AiClassification,
}

#[derive(Debug, Clone)]
pub struct Evidence {
    pub kind: EvidenceKind,
    /// Stable pseudonymous source key. Equal keys are not independent.
    pub source_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationStatus {
    Unverified,
    Probable,
    Verified,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerificationResult {
    pub status: VerificationStatus,
    pub confidence: f32,
    pub independent_sources: usize,
    pub reasons: Vec<&'static str>,
}

fn weight(kind: EvidenceKind) -> f32 {
    match kind {
        EvidenceKind::FieldObservation => 0.22,
        EvidenceKind::Photo => 0.22,
        EvidenceKind::IndependentObservation => 0.20,
        EvidenceKind::GovernmentDataset => 0.35,
        EvidenceKind::OpenGeospatialRecord => 0.15,
        EvidenceKind::PublicProcurementRecord => 0.20,
        EvidenceKind::AiClassification => 0.08,
    }
}

pub fn evaluate(evidence: &[Evidence]) -> VerificationResult {
    use std::collections::HashSet;

    let sources: HashSet<&str> = evidence.iter().map(|e| e.source_key.as_str()).collect();
    let independent_sources = sources.len();

    // Diminishing-return evidence score. This is a policy score, not a statistical probability.
    let confidence = evidence
        .iter()
        .fold(0.0_f32, |score, e| score + (1.0 - score) * weight(e.kind))
        .clamp(0.0, 0.99);

    let has_authoritative = evidence.iter().any(|e| e.kind == EvidenceKind::GovernmentDataset);
    let has_non_ai = evidence.iter().any(|e| e.kind != EvidenceKind::AiClassification);
    let corroborated = independent_sources >= 2;
    let verification_gate = has_non_ai && (corroborated || has_authoritative);

    let status = if verification_gate && confidence >= 0.60 {
        VerificationStatus::Verified
    } else if confidence >= 0.35 {
        VerificationStatus::Probable
    } else {
        VerificationStatus::Unverified
    };

    let mut reasons = Vec::new();
    if !has_non_ai {
        reasons.push("AI-only evidence cannot verify a camera");
    }
    if !corroborated && !has_authoritative {
        reasons.push("independent corroboration is required");
    }
    if has_authoritative {
        reasons.push("authoritative public dataset present");
    }

    VerificationResult { status, confidence, independent_sources, reasons }
}

/// Haversine distance in meters. Used for candidate duplicate searches before
/// database-level PostGIS matching.
pub fn distance_meters(a_lat: f64, a_lon: f64, b_lat: f64, b_lon: f64) -> f64 {
    let r = 6_371_008.8_f64;
    let (lat1, lat2) = (a_lat.to_radians(), b_lat.to_radians());
    let dlat = (b_lat - a_lat).to_radians();
    let dlon = (b_lon - a_lon).to_radians();
    let h = (dlat / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}

pub fn is_duplicate_candidate(distance_m: f64) -> bool {
    distance_m <= 30.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(kind: EvidenceKind, source: &str) -> Evidence {
        Evidence { kind, source_key: source.to_string() }
    }

    #[test]
    fn ai_alone_never_verifies() {
        let result = evaluate(&[
            e(EvidenceKind::AiClassification, "model-a"),
            e(EvidenceKind::AiClassification, "model-b"),
        ]);
        assert_ne!(result.status, VerificationStatus::Verified);
    }

    #[test]
    fn repeated_same_source_is_not_independent() {
        let result = evaluate(&[
            e(EvidenceKind::Photo, "alice"),
            e(EvidenceKind::FieldObservation, "alice"),
            e(EvidenceKind::IndependentObservation, "alice"),
        ]);
        assert_eq!(result.independent_sources, 1);
        assert_ne!(result.status, VerificationStatus::Verified);
    }

    #[test]
    fn independent_strong_evidence_can_verify() {
        let result = evaluate(&[
            e(EvidenceKind::Photo, "alice"),
            e(EvidenceKind::FieldObservation, "alice"),
            e(EvidenceKind::IndependentObservation, "bob"),
            e(EvidenceKind::OpenGeospatialRecord, "osm"),
        ]);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    #[test]
    fn government_data_still_needs_sufficient_evidence_score() {
        let result = evaluate(&[e(EvidenceKind::GovernmentDataset, "agency")]);
        assert_eq!(result.status, VerificationStatus::Probable);
    }

    #[test]
    fn duplicate_radius_boundary() {
        assert!(is_duplicate_candidate(30.0));
        assert!(!is_duplicate_candidate(30.01));
    }

    #[test]
    fn distance_is_zero_for_same_point() {
        assert!(distance_meters(33.749, -84.388, 33.749, -84.388) < 0.001);
    }
}
