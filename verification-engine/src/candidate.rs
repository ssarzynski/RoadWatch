#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateDecision {
    SameInstallation,
    NeedsReview,
    DifferentInstallation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateResolution {
    pub decision: CandidateDecision,
    pub reasons: Vec<&'static str>,
}

#[derive(Debug, Clone)]
pub struct CandidateFacts<'a> {
    pub distance_m: f64,
    pub report_bearing: Option<u16>,
    pub camera_bearing: Option<u16>,
    pub report_function: &'a str,
    pub camera_function: &'a str,
    pub same_road: Option<bool>,
    /// Strong independent evidence that both observations depict the same
    /// physical installation (for example, independently captured matching
    /// physical characteristics). This must be established outside this module.
    pub physical_match: bool,
}

pub fn bearing_difference_degrees(a: u16, b: u16) -> u16 {
    let raw = a.abs_diff(b) % 360;
    raw.min(360 - raw)
}

pub fn resolve_candidate(f: CandidateFacts<'_>) -> CandidateResolution {
    let mut reasons = Vec::new();

    if f.distance_m > 30.0 {
        reasons.push("outside duplicate candidate radius");
        return CandidateResolution {
            decision: CandidateDecision::DifferentInstallation,
            reasons,
        };
    }

    let bearing_difference = match (f.report_bearing, f.camera_bearing) {
        (Some(a), Some(b)) => Some(bearing_difference_degrees(a, b)),
        _ => None,
    };

    // Strong opposite-direction evidence is important on divided roads where
    // two cameras can be only meters apart.
    if bearing_difference.is_some_and(|d| d >= 120) && !f.physical_match {
        reasons.push("bearings indicate different viewing directions");
        return CandidateResolution {
            decision: CandidateDecision::DifferentInstallation,
            reasons,
        };
    }

    if f.same_road == Some(false) && !f.physical_match {
        reasons.push("candidate is associated with a different road");
        return CandidateResolution {
            decision: CandidateDecision::DifferentInstallation,
            reasons,
        };
    }

    let functions_compatible = f.report_function == "unknown"
        || f.camera_function == "unknown"
        || f.report_function == f.camera_function;

    if !functions_compatible {
        reasons.push("camera functions conflict");
        return CandidateResolution {
            decision: CandidateDecision::NeedsReview,
            reasons,
        };
    }

    // Distance is only a candidate-generation signal. A same-installation
    // decision requires physical-match evidence plus no contradictory geometry.
    if f.physical_match {
        reasons.push("independent physical evidence matches installation");
        if bearing_difference.is_some_and(|d| d <= 45) {
            reasons.push("bearings are compatible");
        }
        if f.same_road == Some(true) {
            reasons.push("road association matches");
        }
        return CandidateResolution {
            decision: CandidateDecision::SameInstallation,
            reasons,
        };
    }

    reasons.push("proximity alone cannot establish a duplicate");
    CandidateResolution {
        decision: CandidateDecision::NeedsReview,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>() -> CandidateFacts<'a> {
        CandidateFacts {
            distance_m: 8.0,
            report_bearing: Some(90),
            camera_bearing: Some(92),
            report_function: "alpr",
            camera_function: "alpr",
            same_road: Some(true),
            physical_match: false,
        }
    }

    #[test]
    fn bearing_wraparound_is_correct() {
        assert_eq!(bearing_difference_degrees(359, 1), 2);
        assert_eq!(bearing_difference_degrees(10, 350), 20);
    }

    #[test]
    fn proximity_alone_never_merges() {
        let result = resolve_candidate(facts());
        assert_eq!(result.decision, CandidateDecision::NeedsReview);
    }

    #[test]
    fn physical_match_can_resolve_same_installation() {
        let mut f = facts();
        f.physical_match = true;
        let result = resolve_candidate(f);
        assert_eq!(result.decision, CandidateDecision::SameInstallation);
    }

    #[test]
    fn opposite_direction_is_different_without_physical_match() {
        let mut f = facts();
        f.camera_bearing = Some(270);
        let result = resolve_candidate(f);
        assert_eq!(result.decision, CandidateDecision::DifferentInstallation);
    }

    #[test]
    fn conflicting_function_requires_review() {
        let mut f = facts();
        f.camera_function = "speed_enforcement";
        let result = resolve_candidate(f);
        assert_eq!(result.decision, CandidateDecision::NeedsReview);
    }

    #[test]
    fn outside_radius_is_different() {
        let mut f = facts();
        f.distance_m = 30.01;
        let result = resolve_candidate(f);
        assert_eq!(result.decision, CandidateDecision::DifferentInstallation);
    }
}
