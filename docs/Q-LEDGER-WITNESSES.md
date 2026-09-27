# Q Ledger Witness Model

Status: implemented design and Rust verification primitives; awaiting local validation.

## Purpose

A RoadWatch checkpoint proves what the log signer committed to at a particular tree size.
An independent witness makes later equivocation or silent history rewriting harder by
verifying that checkpoint against the public ledger events and then countersigning it.

## Trust separation

Each witness MUST have its own Ed25519 private key.

- RoadWatch API/signing service: one signing key.
- Georgia home-lab witness A: separate key.
- Additional home-lab/community witness B: separate key.
- Future independent community/institutional witnesses: separate keys.

Never copy one private key between machines and call those machines independent witnesses.
RoadWatch stores only witness public keys and witness statements.

## Witness procedure

1. Fetch public ledger events through checkpoint tree size.
2. Verify sequence numbers and previous-event hash chain.
3. Recompute every event hash.
4. Recompute the Merkle root.
5. Confirm tree size, Merkle root and last-event hash equal the checkpoint.
6. Verify the RoadWatch checkpoint Ed25519 signature.
7. Only then sign the checkpoint identity with the witness key.
8. Publish the witness statement.

The witness does not need photographs, contributor credentials, quarantine objects,
private GPS trails, or the underlying private verification record.

## Failure behavior

A mismatch is evidence of inconsistency, not automatic proof of malicious behavior.
The witness MUST refuse to sign and retain enough public checkpoint metadata for review.
It must not rewrite the ledger, delete evidence, ban contributors, or repair history.

## Key handling

Private witness keys must not be committed to Git, placed in PostgreSQL, embedded in
container images, or returned through APIs. Initial home-lab deployment should use
root/service-account readable local secret files with restrictive permissions. Later
versions can use hardware-backed or OS key stores.

## Independence rule

Multiple signatures only provide meaningful additional assurance when their private keys
and administrative control are genuinely independent. Three services on one host using
the same secret are one trust domain, not three witnesses.


## Split-view detection

Witnesses should exchange signed checkpoint identities (signer public key, tree size,
Merkle root, last-event hash and signature).

Two independently valid checkpoints signed by the same log key at the same tree size but
with different Merkle roots or last-event hashes are an equivocation conflict. Preserve
both signed checkpoints and raise the conflict for investigation. Do not automatically
choose one branch or delete either statement.

A checkpoint at a larger tree size is not, by itself, cryptographic proof that it extends
an earlier checkpoint. Q Ledger v0.1 therefore labels different valid tree sizes as
potential log growth, not as proven consistency.

The next protocol revision should add Merkle consistency proofs so witnesses can verify
that a newer root is an append-only extension of a previously witnessed root.
