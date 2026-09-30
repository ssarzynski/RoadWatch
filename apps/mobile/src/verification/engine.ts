export type VerificationStatus =
  | "unverified"
  | "candidate"
  | "verified"
  | "disputed"
  | "rejected";

export type VerificationSource =
  | "user_report"
  | "repeat_observation"
  | "trusted_source"
  | "admin_review";

export type VerificationEvidence = {
  source: VerificationSource;
  reporterId?: string;
  latitude: number;
  longitude: number;
  accuracyMeters?: number | null;
  observedAt: number;
  confidence?: number;
};

export type VerificationResult = {
  status: VerificationStatus;
  score: number;
  independentReports: number;
  reasons: string[];
};

const MAX_SCORE = 100;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function evidenceWeight(evidence: VerificationEvidence): number {
  let weight = 0;

  switch (evidence.source) {
    case "user_report":
      weight = 20;
      break;

    case "repeat_observation":
      weight = 25;
      break;

    case "trusted_source":
      weight = 45;
      break;

    case "admin_review":
      weight = 50;
      break;
  }

  if (
    evidence.accuracyMeters != null &&
    evidence.accuracyMeters <= 20
  ) {
    weight += 5;
  }

  if (evidence.confidence != null) {
    weight *= clamp(evidence.confidence, 0, 1);
  }

  return weight;
}

export function verifyCamera(
  evidence: VerificationEvidence[]
): VerificationResult {
  if (evidence.length === 0) {
    return {
      status: "unverified",
      score: 0,
      independentReports: 0,
      reasons: ["No verification evidence available."],
    };
  }

  const reasons: string[] = [];

  const reporters = new Set(
    evidence
      .map((item) => item.reporterId)
      .filter((id): id is string => Boolean(id))
  );

  const independentReports = reporters.size;

  let score = evidence.reduce(
    (total, item) => total + evidenceWeight(item),
    0
  );

  if (independentReports >= 2) {
    score += 15;
    reasons.push("Confirmed by multiple independent reporters.");
  }

  if (evidence.some((item) => item.source === "trusted_source")) {
    reasons.push("Trusted-source evidence present.");
  }

  if (evidence.some((item) => item.source === "admin_review")) {
    reasons.push("Administrative review present.");
  }

  score = clamp(Math.round(score), 0, MAX_SCORE);

  let status: VerificationStatus = "unverified";

  if (score >= 70) {
    status = "verified";
  } else if (score >= 30) {
    status = "candidate";
  }

  reasons.push(`Verification score: ${score}/${MAX_SCORE}.`);

  return {
    status,
    score,
    independentReports,
    reasons,
  };
}