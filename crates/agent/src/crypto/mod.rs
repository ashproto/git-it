//! Phase 2b end-to-end crypto for the agent: HPKE base-mode encryption + Ed25519
//! sender signatures + the CBOR message envelope. Convex sees only ciphertext.
//! The Swift side (git-it-ios Sources/Support/Crypto) mirrors this; the shared
//! KAT vectors (tests/vectors/kat_v1.json) prove byte-interop in both directions.

pub mod envelope;
pub mod roster;
