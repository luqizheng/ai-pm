# SEM Callback JCS Canonicalizer Dependency Evaluation

**Status:** Candidate evaluation only — no dependency selected, no Cargo manifest or toolchain changed, no runtime implementation authorized.

**Scope:** AI-PM's SEM callback Inbox. AI-PM must compute two hashes over authenticated raw callback bytes: payload-level `payload_hash` and full-envelope `event_envelope_hash`. The canonicalizer must operate on raw UTF-8 JSON bytes before a lossy `serde_json::Value` parse.

## 1. Requirements

The production implementation must:

1. Canonicalize RFC 8785 JCS bytes with UTF-16 code-unit property ordering and ECMAScript-compatible binary64 serialization.
2. Reject duplicate object member names after escape decoding, including nested objects, before any information-losing JSON value materialization.
3. Reject invalid UTF-8, malformed JSON, lone surrogates, non-finite numbers and out-of-domain integer tokens; never silently repair or round an exact integer into a different value.
4. Apply explicit input-byte, nesting-depth, object-member and string-size limits before opening the Inbox transaction.
5. Produce deterministic refusal codes and byte-identical output across independent implementations.
6. Pass the shared callback-envelope vectors and the full JCS edge corpus. A refusal must cause no Inbox insert, no cursor movement, no ACK and no generic AI-analysis job.

`serde_json::Value::to_string()` is not a canonicalizer. Parsing untrusted JSON into a normal map before duplicate detection is also prohibited.

## 2. Candidate comparison

| Candidate | Technical fit | Compatibility / maturity | Disposition |
|---|---|---|---|
| `canonprims-json 0.1.4` | Raw-byte, one-pass strict I-JSON parser; rejects duplicate names, lone surrogates, noncharacters, non-finite numbers, invalid UTF-8 and inexact integer tokens; explicit limits; UTF-16 ordering; ECMAScript number output; shared `canonical-json/v0` corpus | Requires Rust 1.88.0. Repository is very new (created 2026-10-24; 25 commits; 0 stars/forks at review); crate is 0.1.x. MIT OR Apache-2.0. | **Best technical candidate for an isolated spike; not yet approved for production** |
| `serde_json_canonicalizer 0.4.1` | Strong JCS serializer and Ryu-JS number output, but accepts Rust `Serialize` values / `serde_json::Value`; it is not a strict raw JSON admission parser. Duplicate keys may already be lost before canonicalization; integer-to-f64 conversion can alter values unless a separate admission layer rejects them. | More established public project, but would require a separately maintained strict parser and numeric-domain guard. | Not suitable as the sole raw callback canonicalizer |
| `json-syntax 0.13.1` | Parser/AST preserves duplicate members and has a canonicalization feature. | Current version requires Rust 1.96, above the SEM candidate MSRV 1.85 and the AI-PM toolchain floor currently implied by its floating builder image. Older 0.12.x API/behavior has not been re-audited. | Do not select without a separate version-pinned compatibility audit |
| `unbounded-ai/jcs 0.1.0` | Advertises strict duplicate-key parsing, integer exactness and Rust 1.85 compatibility. | Extremely new, very small public history and adoption; provenance and maintenance need deeper review. | Not preferred over the corpus-backed candidate without further evidence |

## 3. Leading candidate: canonprims-json 0.1.4

Reviewed published source: `3leaps/canonprims` release commit `3595826f2a7c0963783c842cc77e5f6242f335fd`.

The release workspace declares edition 2021, `rust-version = 1.88.0`, and `MIT OR Apache-2.0`. The Rust API accepts raw bytes and explicit `Limits`:

```rust
use canonprims_json::canonprims_core::{DigestAlg, Limits};

let limits = Limits::default();
let canonical = canonprims_json::canonicalize(raw_json_bytes, &limits)?;
let digest = canonprims_json::digest(raw_json_bytes, &limits, DigestAlg::Sha256)?;
```

The implementation uses a single-pass parser and emits canonical bytes while parsing; each object tracks decoded names, rejects duplicates, and sorts its own members by UTF-16 code units. It does not build an intermediate document tree. The number layer checks exact integer admission and uses `ryu-js` for ECMAScript number spelling. The crate exposes stable refusal codes and explicit resource limits.

The repository maintains a shared `canonical-json/v0` corpus and Rust/Go implementations, with Python and TypeScript bindings. Its CI runs formatting, lint, tests, corpus checks, cross-language comparison, witnesses and dependency/license checks across Linux, macOS, Windows and ARM64 targets. The maintainers explicitly note that Rust and Go share parser architecture, so their agreement alone is not independent proof; the file-based accepted cases are also checked against ECMAScript and a separately pinned Python witness.

### Risks that remain

- The project was created on 2026-10-24 and is still 0.x, with little external adoption. Its strong engineering evidence does not substitute for our own dependency/provenance review.
- The crate's strict input domain includes explicit I-JSON refusal rules for inexact integer tokens and Unicode noncharacters. AI-PM must adopt those rules deliberately; large exact quantities should be represented as strings in business payloads.
- The release requires Rust 1.88.0. AI-PM's `Dockerfile` currently uses floating `rust:1-slim` and the workspace does not declare `rust-version`; this is not reproducible enough for a new cryptographic dependency. If adopted, pin the builder/toolchain to at least 1.88.0 and declare the workspace floor.
- The SEM workspace candidate currently declares MSRV 1.85. This dependency is proposed for the AI-PM callback Inbox only; SEM does not need to adopt it or raise its MSRV unless SEM later becomes a producer of these JCS hashes.

## 4. Recommendation

Do not add a production dependency yet. Use `canonprims-json 0.1.4` as the leading candidate for a bounded, isolated compatibility spike in AI-PM. Do not use `serde_json::Value::to_string()`, and do not call a permissive `serde_json::Value` parser before the strict canonicalizer.

The spike should be a separate, reviewable test-only change and must prove:

1. Build and test under pinned Rust 1.88.0 on the same Linux target used by AI-PM's Docker builder.
2. The crate's `CORPUS_DIGEST` matches the pinned upstream corpus manifest; import the complete corpus into AI-PM tests rather than copying only selected examples.
3. Rust produces byte-identical JCS output and both hashes for the shared callback envelope vectors; Node and Python independently verify the same bytes and hashes.
4. Every refusal class is exercised, including nested and escaped-equivalent duplicate names, lone surrogates, invalid UTF-8, noncharacters, non-finite numbers, inexact integer tokens, malformed JSON and every configured resource limit.
5. Refusals happen before transaction start and produce no Inbox row, no cursor movement, no ACK, no outbox side effect and no generic analysis job.
6. `cargo deny`, license inventory, dependency checksum/provenance, release workflow and source pin are reviewed.

Only after that spike passes should the dependency/version and toolchain floor be ratified. Until then, `payload_hash` and `event_envelope_hash` remain protocol requirements, not an approved implementation choice.

## 5. Decision boundary

No dependency, `Cargo.toml`, `Cargo.lock`, Dockerfile, toolchain pin, migration or runtime parser was changed by this evaluation. A Rust 1.88 floor for AI-PM is a build-compatibility decision and must be explicitly accepted before the spike becomes an implementation change.
