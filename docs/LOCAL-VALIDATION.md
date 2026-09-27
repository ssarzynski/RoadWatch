# Local Validation

RoadWatch does not require GitHub-hosted Actions for routine validation.

## Verification engine

From the repository root:

```bash
bash scripts/verify.sh
```

The script fails immediately if any required check fails. It currently runs:

1. Rust formatting validation
2. Clippy with warnings treated as errors
3. Unit tests

## Before merging or committing release-ready code

Run `scripts/verify.sh` locally and record/fix any failures. Future API/database/mobile checks should be added to this script so RoadWatch has one repeatable local quality gate.

A self-hosted runner may be added later, but hosted CI is not required for development.
