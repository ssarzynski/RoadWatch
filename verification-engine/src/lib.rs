//! Deterministic RoadWatch verification policy.
//! Scores are explainable policy evidence scores, never statistical probabilities.
//! AI classification is advisory and cannot establish physical camera presence.

pub mod candidate;
pub mod risk;

use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    HighConfidence,
    Verified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationResult {
    pub presence_status: VerificationStatus,
    pub presence_evidence_score: u8,
    pub classification_evidence_score: u8,
    pub independent_presence_sources: usize,
    pub reasons: Vec<&'static str>,
}

fn presence_weight(kind: EvidenceKind) -> u8 {
    match kind {
        EvidenceKind::FieldObservation => 25,
        EvidenceKind::Photo => 25,
        EvidenceKind::IndependentObservation => 20,
        EvidenceKind::GovernmentDataset => 40,
        EvidenceKind::OpenGeospatialRecord => 15,
        EvidenceKind::PublicProcurementRecord => 10,
        EvidenceKind::AiClassification => 0,
    }
}

fn classification_weight(kind: EvidenceKind) -> u8 {
    match kind {
        EvidenceKind::FieldObservation => 10,
        EvidenceKind::Photo => 25,
        EvidenceKind::IndependentObservation => 10,
        EvidenceKind::GovernmentDataset => 35,
        EvidenceKind::OpenGeospatialRecord => 10,
        EvidenceKind::PublicProcurementRecord => 20,
        EvidenceKind::AiClassification => 10,
    }
}

fn capped_score(evidence: &[Evidence], weight: fn(EvidenceKind) -> u8) -> u8 {
    // One contribution per source+evidence class prevents replaying the same
    // source/class from inflating a score.
    let mut seen: HashSet<(&str, EvidenceKind)> = HashSet::new();
    let mut total: u16 = 0;
    for item in evidence {
        if seen.insert((item.source_key.as_str(), item.kind)) {
            total = total.saturating_add(weight(item.kind) as u16);
        }
    }
    total.min(100) as u8
}

pub fn evaluate(evidence: &[Evidence]) -> VerificationResult {
    let presence_evidence_score = capped_score(evidence, presence_weight);
    let classification_evidence_score = capped_score(evidence, classification_weight);

    let presence_sources: HashSet<&str> = evidence
        .iter()
        .filter(|e| presence_weight(e.kind) > 0)
        .map(|e| e.source_key.as_str())
        .collect();
    let independent_presence_sources = presence_sources.len();

    let has_authoritative_presence = evidence
        .iter()
        .any(|e| e.kind == EvidenceKind::GovernmentDataset);
    let corroborated = independent_presence_sources >= 2;

    // Verification requires a high score AND independent corroboration, or
    // authoritative public data plus at least one independent field/source signal.
    let authoritative_plus_independent =
        has_authoritative_presence && independent_presence_sources >= 2;

    let presence_status = if presence_evidence_score >= 90
        && (corroborated || authoritative_plus_independent)
    {
        VerificationStatus::Verified
    } else if presence_evidence_score >= 70 && corroborated {
        VerificationStatus::HighConfidence
    } else if presence_evidence_score >= 40 {
        VerificationStatus::Probable
    } else {
        VerificationStatus::Unverified
    };

    let mut reasons = Vec::new();
    if evidence.iter().all(|e| e.kind == EvidenceKind::AiClassification) && !evidence.is_empty() {
        reasons.push("AI-only evidence cannot establish camera presence");
    }
    if !corroborated {
        reasons.push("independent presence corroboration is required for high confidence");
    }
    if has_authoritative_presence {
        reasons.push("authoritative public dataset present");
    }

    VerificationResult {
        presence_status,
        presence_evidence_score,
        classification_evidence_score,
        independent_presence_sources,
        reasons,
    }
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
    fn ai_alone_has_zero_presence_score() {
        let result = evaluate(&[
            e(EvidenceKind::AiClassification, "model-a"),
            e(EvidenceKind::AiClassification, "model-b"),
        ]);
        assert_eq!(result.presence_evidence_score, 0);
        assert_eq!(result.presence_status, VerificationStatus::Unverified);
        assert!(result.classification_evidence_score > 0);
    }

    #[test]
    fn repeated_same_source_and_class_does_not_stack() {
        let one = evaluate(&[e(EvidenceKind::Photo, "alice")]);
        let repeated = evaluate(&[
            e(EvidenceKind::Photo, "alice"),
            e(EvidenceKind::Photo, "alice"),
            e(EvidenceKind::Photo, "alice"),
        ]);
        assert_eq!(one.presence_evidence_score, repeated.presence_evidence_score);
        assert_eq!(repeated.independent_presence_sources, 1);
    }

    #[test]
    fn same_source_different_classes_can_add_evidence_but_not_independence() {
        let result = evaluate(&[
            e(EvidenceKind::Photo, "alice"),
            e(EvidenceKind::FieldObservation, "alice"),
        ]);
        assert_eq!(result.presence_evidence_score, 50);
        assert_eq!(result.independent_presence_sources, 1);
        assert_eq!(result.presence_status, VerificationStatus::Probable);
    }

    #[test]
    fn independent_strong_evidence_can_verify() {
        let result = evaluate(&[
            e(EvidenceKind::GovernmentDataset, "agency"),
            e(EvidenceKind::Photo, "alice"),
            e(EvidenceKind::FieldObservation, "alice"),
            e(EvidenceKind::IndependentObservation, "bob"),
        ]);
        assert_eq!(result.presence_evidence_score, 100);
        assert_eq!(result.presence_status, VerificationStatus::Verified);
    }

    #[test]
    fn authoritative_source_alone_is_not_verified() {
        let result = evaluate(&[e(EvidenceKind::GovernmentDataset, "agency")]);
        assert_eq!(result.presence_status, VerificationStatus::Probable);
        assert_eq!(result.independent_presence_sources, 1);
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
