#[derive(Debug, Clone, PartialEq)]
pub struct PriorObservation {
    pub latitude: f64,
    pub longitude: f64,
    /// Server receipt time in Unix seconds.
    pub received_at: i64,
}

#[derive(Debug, Clone)]
pub struct RiskInput<'a> {
    pub latitude: f64,
    pub longitude: f64,
    pub received_at: i64,
    pub reports_last_hour: u32,
    pub exact_image_replay: bool,
    pub perceptual_image_replay: bool,
    pub correlated_source_count: u32,
    pub prior: Option<&'a PriorObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewDisposition {
    Normal,
    Review,
    Quarantine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskAssessment {
    /// Deterministic policy score, not a probability of abuse.
    pub score: u8,
    pub disposition: ReviewDisposition,
    pub independent_weight_allowed: bool,
    pub signals: Vec<&'static str>,
}

pub fn assess(input: &RiskInput<'_>) -> RiskAssessment {
    let mut score: u16 = 0;
    let mut signals = Vec::new();
    let mut independent = true;

    if input.exact_image_replay {
        score += 45;
        signals.push("exact_image_replay");
        independent = false;
    } else if input.perceptual_image_replay {
        score += 25;
        signals.push("perceptual_image_replay");
        independent = false;
    }

    if input.reports_last_hour >= 60 {
        score += 35;
        signals.push("extreme_submission_rate");
    } else if input.reports_last_hour >= 20 {
        score += 15;
        signals.push("elevated_submission_rate");
    }

    if input.correlated_source_count >= 3 {
        score += 30;
        signals.push("correlated_sources");
        independent = false;
    }

    if let Some(prior) = input.prior {
        if impossible_travel(prior, input.latitude, input.longitude, input.received_at) {
            score += 40;
            signals.push("impossible_travel");
            independent = false;
        }
    }

    let score = score.min(100) as u8;
    let disposition = if score >= 70 {
        ReviewDisposition::Quarantine
    } else if score >= 30 {
        ReviewDisposition::Review
    } else {
        ReviewDisposition::Normal
    };

    RiskAssessment {
        score,
        disposition,
        independent_weight_allowed: independent && disposition != ReviewDisposition::Quarantine,
        signals,
    }
}

/// Conservative plausibility screen. Uses server receipt times, not client timestamps.
/// Speeds above 350 km/h over at least 60 seconds are treated as impossible for
/// normal RoadWatch road-reporting activity. Very short intervals are not judged
/// here because GPS jitter can create misleading instantaneous speeds.
fn impossible_travel(
    prior: &PriorObservation,
    latitude: f64,
    longitude: f64,
    received_at: i64,
) -> bool {
    let elapsed = received_at.saturating_sub(prior.received_at);
    if elapsed < 60 {
        return false;
    }
    let km = haversine_km(prior.latitude, prior.longitude, latitude, longitude);
    let hours = elapsed as f64 / 3600.0;
    km / hours > 350.0
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0088_f64;
    let p1 = lat1.to_radians();
    let p2 = lat2.to_radians();
    let dp = (lat2 - lat1).to_radians();
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2)
        + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * a.sqrt().atan2((1.0 - a).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_report_is_normal() {
        let input = RiskInput {
            latitude: 42.0, longitude: -83.0, received_at: 1000,
            reports_last_hour: 2, exact_image_replay: false,
            perceptual_image_replay: false, correlated_source_count: 0, prior: None,
        };
        let r = assess(&input);
        assert_eq!(r.disposition, ReviewDisposition::Normal);
        assert!(r.independent_weight_allowed);
    }

    #[test]
    fn exact_replay_loses_independence() {
        let input = RiskInput {
            latitude: 42.0, longitude: -83.0, received_at: 1000,
            reports_last_hour: 1, exact_image_replay: true,
            perceptual_image_replay: false, correlated_source_count: 0, prior: None,
        };
        let r = assess(&input);
        assert!(!r.independent_weight_allowed);
        assert!(r.signals.contains(&"exact_image_replay"));
    }

    #[test]
    fn impossible_travel_forces_review() {
        let prior = PriorObservation {
            latitude: 42.0, longitude: -83.0, received_at: 1000,
        };
        let input = RiskInput {
            latitude: 33.75, longitude: -84.39, received_at: 1600,
            reports_last_hour: 2, exact_image_replay: false,
            perceptual_image_replay: false, correlated_source_count: 0,
            prior: Some(&prior),
        };
        let r = assess(&input);
        assert!(r.signals.contains(&"impossible_travel"));
        assert!(!r.independent_weight_allowed);
    }

    #[test]
    fn combined_signals_quarantine_without_declaring_malice() {
        let input = RiskInput {
            latitude: 42.0, longitude: -83.0, received_at: 1000,
            reports_last_hour: 80, exact_image_replay: true,
            perceptual_image_replay: false, correlated_source_count: 4, prior: None,
        };
        let r = assess(&input);
        assert_eq!(r.disposition, ReviewDisposition::Quarantine);
        assert_eq!(r.score, 100);
    }
}
