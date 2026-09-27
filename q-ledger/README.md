# Q Ledger

Q Ledger is RoadWatch's cryptographic transparency layer. It is **not** a cryptocurrency blockchain.

It provides:
- deterministic event commitments
- previous-event hash chaining
- Merkle roots over ordered event hashes
- Ed25519-signed checkpoints
- independent verification

Only commitments and non-sensitive event metadata belong in the ledger. Do not put photographs, contributor tokens, route history, raw GPS trails, private storage keys, or other personal/private data into an immutable log.

Status: implemented, awaiting local Rust validation.
