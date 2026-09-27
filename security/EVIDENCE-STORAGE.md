# RoadWatch Evidence Storage Boundary

Status: **designed / schema implemented / storage adapter pending local validation**

## Trust zones

### Zone A — private quarantine

Contains original untrusted upload bytes only when retention is operationally necessary.

Requirements:
- never public
- never directly linked from API responses
- opaque randomized object keys
- encryption at rest
- least-privilege service identity
- no directory listing
- no user-controlled object paths
- short retention by default
- deletion lifecycle
- access audit logging
- security/moderation worker access only

Suggested development layout:

```text
private-quarantine/<random-uuid>
```

The original filename must not become an object key.

### Zone B — sanitized evidence

Contains only derivatives produced after successful strict decode and fresh re-encoding.

Requirements:
- separate bucket/directory/permission boundary from quarantine
- immutable/content-addressable naming is preferred
- no original EXIF/XMP/IPTC metadata
- only `accepted_sanitized` evidence is eligible for verification scoring
- quarantined derivatives are not public until review clears them

Suggested layout:

```text
sanitized/<sanitized-sha256>
```

## Public API rule

Public response structs must be allowlists. Do not serialize database evidence rows directly.

Allowed public evidence fields may include:
- evidence ID
- camera/report association when appropriate
- sanitized evidence delivery URL generated server-side
- sanitized hash when useful
- evidence type
- public verification status

Never public:
- quarantine object key
- contributor token/hash
- original upload location
- original filename
- raw EXIF/XMP/IPTC
- abuse/moderation signals
- storage credentials
- internal network paths

## Retention

Initial policy target:
- rejected uploads: do not retain unless required for short-lived abuse/security diagnostics
- quarantined originals: short retention, default target 7 days, then automatic deletion unless actively reviewed
- accepted original uploads: delete after sanitization/security processing unless a documented moderation need requires temporary retention
- sanitized derivatives: retain according to evidence/history policy
- hashes may be retained longer for replay/abuse detection without retaining hostile original bytes

Retention durations must remain configurable and documented before production deployment.

## Failure behavior

If sanitized storage fails, evidence is not eligible for scoring.
If quarantine storage fails for suspicious input, fail closed and do not promote it.
If database persistence fails, do not report evidence as safely stored.
