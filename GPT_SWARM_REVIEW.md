# Swerve Project — GPT Swarm Review Reports

> **4 rounds** of GPT swarm reviews conducted. Each round uses 5 parallel GPT sub-agents.
> Models used: Claude Sonnet 4.5 (general-purpose agents)

---

# Round 1 Review (Original)

---

## Review Agents & Focus Areas

| # | Agent | Model | Focus |
|---|-------|-------|-------|
| 1 | 🔒 Security | GPT-5.4 | Crypto, auth, injection, DoS, secrets |
| 2 | 🏗️ Architecture | GPT-5.4 | Module design, error handling, API, state, extensibility |
| 3 | 🖥️ CLI UX | GPT-5.2 | Command hierarchy, help text, ergonomics, discoverability |
| 4 | 🦀 Rust Idioms | GPT-5.2 | Ownership, error types, async patterns, clippy |
| 5 | 🧪 Test Coverage | GPT-5.4-mini | Missing tests, edge cases, priorities |

---

## CRITICAL / HIGH Findings (cross-agent consensus)

These findings were flagged by **multiple agents** as the most important:

### 1. 🔒🦀 Unbounded uploads / downloads — memory exhaustion DoS
- **Where**: `swerve/src/mgmt.rs` (upload handler), `serve.rs` (serve handler)
- **Problem**: Entire files buffered in memory (upload: `field.bytes().await` + `to_vec()`; download: full read + decrypt). No body size limit.
- **Impact**: Single large upload can OOM the server. Worse on unauthenticated serve sockets.
- **Fix**: Add `DefaultBodyLimit`, stream uploads/downloads, enforce max file size.

### 2. 🔒🏗️ Startup deletes entire storage directory
- **Where**: `swerve/src/main.rs:38-42`
- **Problem**: `remove_dir_all` on user-supplied `--storage-dir`. Dangerous misconfiguration risk.
- **Fix**: Use dedicated subdirectory with marker file, or require `--wipe-on-start` flag.

### 3. 🔒🦀 TOCTOU race on serve_name uniqueness
- **Where**: `swerve/src/mgmt.rs:92-106` (upload), `mgmt.rs:226-257` (set_serve_state)
- **Problem**: Uniqueness checked under read lock, insertion under write lock. Concurrent uploads can create duplicate serve_names.
- **Fix**: Check + insert under single write lock.

### 4. 🔒🦀🏗️ Multipart parse errors silently swallowed
- **Where**: `swerve/src/mgmt.rs:55`
- **Problem**: `while let Ok(Some(field))` treats parse errors as end-of-stream.
- **Fix**: Propagate errors as 400 Bad Request.

### 5. 🔒🦀🏗️ Panics in production code paths (expect/unwrap)
- **Where**: `swerve/src/main.rs:42,50,55-56`, `mgmt.rs:243,257`
- **Problem**: `expect()` on bind/serve failures; `unwrap()` in handlers after map lookups.
- **Fix**: Return `Result` from main, use `ok_or_else` in handlers.

### 6. 🔒🖥️🦀 HTTP error handling inconsistent in CLI
- **Where**: `fswerve/src/client.rs` (all methods except download)
- **Problem**: Non-2xx responses parsed as JSON blindly → confusing parse errors.
- **Fix**: Centralize response handling; check status first, parse StatusResponse on error.

### 7. 🔒🦀 Filename injection in Content-Disposition headers
- **Where**: `swerve/src/mgmt.rs:181-190`, `serve.rs:75-84`
- **Problem**: Raw filenames interpolated into headers without sanitization.
- **Fix**: Use typed header builders or sanitize to safe character subset.

### 8. 🔒🦀 Custom URL encoding is incomplete/incorrect
- **Where**: `fswerve/src/client.rs:201-209`
- **Problem**: Only encodes 6 characters. Missing `/`, `:`, `@`, unicode, etc.
- **Fix**: Use `percent-encoding` or `url` crate.

---

## MEDIUM Findings

### Security
| Finding | Location | Fix |
|---------|----------|-----|
| API key over plaintext HTTP, default bind 0.0.0.0 | `main.rs`, `auth.rs` | Default to 127.0.0.1, warn on non-TLS |
| API key stored plaintext on disk | `fswerve/config.rs` | Restrict file permissions (0600) |
| No auth throttling, non-constant-time compare | `auth.rs` | Rate limit + constant-time comparison |
| Overwrite race breaks concurrent downloads | `mgmt.rs:117-138` | Atomic write: temp file → rename under write lock |
| Unlimited socket creation (resource exhaustion) | `mgmt.rs:320-353` | Cap socket count, restrict bindable addresses |
| Error messages leak other filenames | `mgmt.rs:99,248,280` | Generic conflict messages |

### Architecture
| Finding | Location | Fix |
|---------|----------|-----|
| `mgmt.rs` is a god module | `swerve/src/mgmt.rs` | Split into routes/files.rs, routes/sockets.rs, services/ |
| No storage abstraction (hard to add persistence) | `state.rs`, `mgmt.rs` | Introduce FileRepository, KeyStore, SocketRegistry traits |
| Socket listener failures swallowed, status always "active" | `serve.rs:26-35` | Listener supervisor with real status tracking |
| Health check behind auth middleware | `mgmt.rs:21` | Route /health outside auth layer |
| CPU-heavy crypto on async workers | `mgmt.rs`, `serve.rs` | Use `spawn_blocking` for encrypt/decrypt |
| `AppStateInner` exposes raw fields | `state.rs` | Hide behind methods/service structs |
| Dead API types (`UploadParams`, always-true `SwerveSocket.active`) | `api.rs`, `mgmt.rs` | Remove or implement properly |

### CLI UX
| Finding | Location | Fix |
|---------|----------|-----|
| No confirmation for destructive `destroy` command | `cli.rs`, `main.rs` | Add `--yes` flag + interactive TTY confirm |
| Inconsistent naming: `--serve-as` vs `--name` | `cli.rs` | Standardize to `--serve-name` everywhere |
| `files` is a noun, others are verbs | `cli.rs` | Consider `files list` or add aliases |
| No `--json` output mode | `output.rs` | Add global `--json` flag for scriptability |
| No `--verbose`/`--quiet`/`--no-color` globals | `cli.rs` | Add global flags on Cli struct |
| Hard-coded table widths break on long names | `output.rs` | Dynamic widths or truncation with ellipsis |
| No `status`/`ping` command to verify connectivity | `cli.rs` | Add `fswerve status` hitting /health |
| Missing shell completion support | - | Add `clap_complete` for bash/zsh/fish/powershell |
| Config only from file, no env/flag overrides | `config.rs` | Support FSWERVE_SERVER_URL, FSWERVE_API_KEY env vars |

### Rust Idioms
| Finding | Location | Fix |
|---------|----------|-----|
| Blocking `std::fs` in async contexts | `main.rs`, `config.rs` | Use `tokio::fs` or `spawn_blocking` |
| O(n) file lookup per serve request | `serve.rs:54-60` | Add serve_name → storage_name index |
| Unnecessary clones (owned values could be moved) | `mgmt.rs:298,324` | Destructure and move from owned bodies |
| No newtypes for RealName/ServeName/StorageName | `types.rs` | Prevent mixing up string types |
| FileKey is Debug+Clone with public fields | `crypto.rs` | Remove Debug, add zeroize on Drop |
| Unused deps: base64, serde_json (in core) | `swerve-core/Cargo.toml` | Remove |
| tokio "full" feature is overkill | All Cargo.toml | Use minimal features |

---

## LOW Findings

- Secret material never zeroized in memory
- `tower-http`/CORS declared but unused
- Edition 2024 may cause contributor toolchain friction
- `serve rename` subcommand name is misleading (should be `set-name`)

---

## Test Coverage: Current State

**Zero tests exist.** No `#[test]`, no `#[cfg(test)]`, no `tests/` directory.

### Priority test plan:

| Priority | Crate | Tests |
|----------|-------|-------|
| **P0** | swerve-core | Crypto encrypt/decrypt round-trip, wrong-key rejection, `storage_name_for` determinism, serde round-trips |
| **P0** | swerve | Auth accept/reject, upload→download round-trip, serve state toggle, serve_name conflict, socket lifecycle |
| **P0** | fswerve | Config save/load round-trip, missing config error |
| **P1** | swerve | Destroy + 404, serve socket serves by serve_name, file list accuracy |
| **P1** | fswerve | CLI parsing for all subcommands, URL encoding edge cases |
| **P2** | swerve | Empty file, large file, special chars, concurrent uploads, duplicate socket bind |

---

## Top 10 Remediation Priorities (consolidated across all agents)

1. **Add body size limits** (DefaultBodyLimit) — prevents OOM DoS
2. **Fix TOCTOU race** — check+insert under single write lock
3. **Fix multipart error handling** — propagate as 400
4. **Eliminate panics** — `expect`→`Result`, `unwrap`→`ok_or_else`
5. **Fix CLI HTTP error handling** — centralize response checking
6. **Replace custom URL encoding** — use `percent-encoding` crate
7. **Sanitize Content-Disposition filenames** — prevent header injection
8. **Stop deleting storage dir on startup** — use subdirectory or flag
9. **Add P0 tests** — crypto round-trip, auth, upload/download, config
10. **Add `--yes` to destroy** — prevent accidental deletion

---

# Round 3 Review (Post Round-2 Fixes)

> 90 tests passing. All round 1+2 fixes verified by all 5 agents.

## Findings Summary: 6 HIGH, 10 MEDIUM, 6 LOW

### HIGH

| # | Issue | Source |
|---|-------|--------|
| 1 | **Nonce reuse risk** — FileKey stores fixed nonce; public API allows encrypt() called twice → catastrophic AES-GCM break | Rust Idioms |
| 2 | **Unauthenticated DoS on public swerve sockets** — full file buffered in RAM (2x 50MB/req, no concurrency limit) | Security, Architecture |
| 3 | **Overwrite races with concurrent downloads** — reader gets old key + new ciphertext → 500 | Security, Architecture |
| 4 | **Restart leaves orphaned ciphertext on disk** — state is memory-only, no cleanup on start | Architecture |
| 5 | **Path traversal in default download path** — malicious real_name like ../../.bashrc overwrites local files | CLI UX |
| 6 | **Non-TTY destroy prompt** — piped stdin can accidentally confirm deletion | CLI UX |

### MEDIUM

| # | Issue | Source |
|---|-------|--------|
| 7 | Socket cap transiently bypassable — listener spawned before cap check | Security, Architecture |
| 8 | Write lock held across async rename in upload_file_atomic | Rust, Architecture |
| 9 | Stringly-typed state errors + string matching drives HTTP status | Rust Idioms |
| 10 | Destroy returns success even when disk deletion fails (leaks blobs) | Architecture |
| 11 | --quiet --json still emits JSON on success (inconsistent semantics) | CLI UX |
| 12 | JSON output schema inconsistent across commands | CLI UX |
| 13 | Download error message drops HTTP status code | CLI UX |
| 14 | Malformed multipart test doesn't actually test multipart parsing | Tests |
| 15 | Socket tests don't exercise real network traffic | Tests |
| 16 | Overwrite test is sequential only (no concurrent read/write) | Tests |

### LOW

| # | Issue | Source |
|---|-------|--------|
| 17 | Clippy: 5 collapsible_if warnings + redundant import | Rust Idioms |
| 18 | Unused zeroize "derive" feature | Rust Idioms |
| 19 | Config error doesn't include file path | CLI UX |
| 20 | Help text says destroy "requires --yes" (it just prompts) | CLI UX |
| 21 | Missing download -o - (stdout) support | CLI UX |
| 22 | Missing edge-case upload tests (empty file, Unicode filenames) | Tests |

---

## Round 4 — Post Round-3-Fix Review

**Date**: Round 4  
**Baseline**: 97 tests passing, zero clippy warnings, all 22 round 3 fixes applied  
**Agents**: Security, Architecture, CLI UX, Rust Idioms, Test Coverage

### Summary

| Severity | Count |
|----------|-------|
| HIGH     | 7     |
| MEDIUM   | 17    |
| LOW      | 9     |
| **Total**| **33**|

Quality is significantly improved. No correctness bugs found. Remaining issues are hardening (TLS, streaming, typed errors), UX polish, and test gaps.

### HIGH Findings

| # | Finding | Source |
|---|---------|--------|
| 1 | **No TLS for non-loopback connections** — Management API sends API key + file data over plain HTTP. Any non-localhost deployment exposes secrets to sniffing/MITM. Require HTTPS for remote URLs or document TLS termination requirement. | Security |
| 2 | **Predictable temp storage path** — Default `temp_dir()/swerve-storage` has no symlink/ownership/permission validation. Local attacker could pre-create or redirect path. Use private per-user dir with restrictive perms. | Security |
| 3 | **Full memory buffering for all file I/O** — Upload/download/serve paths fully buffer plaintext + ciphertext in memory. 50MB files × 32 concurrent connections = multi-GB spikes. Move to streaming AEAD. | Security, Architecture, Rust Idioms |
| 4 | **`--json` + `download -o -` sends raw bytes** — Machine-readable mode silently stops being JSON when downloading to stdout. Reject combination or send metadata to stderr. | CLI UX |
| 5 | **Corrupted ciphertext untested** — No test for download/serve when stored blob is truncated/bit-flipped/deleted. Should return 500, never partial plaintext. | Test Coverage |
| 6 | **Destroy-vs-overwrite race untested** — Concurrent DELETE + re-upload race path not covered. Should be deterministic (409 or latest version wins). | Test Coverage |
| 7 | **MAX_UPLOAD_SIZE boundary untested** — No test that exact-limit succeeds and limit+1 fails. | Test Coverage |

### MEDIUM Findings

| # | Finding | Source |
|---|---------|--------|
| 8 | **No minimum API key validation** — Server accepts empty/weak keys. Fail closed on empty/short keys at startup. | Security |
| 9 | **Plaintext API key on Windows** — `~/.fswerve/config.toml` uses default inherited ACLs on Windows (Unix gets 0600). Use OS credential manager or enforce ACLs. | Security |
| 10 | **Socket bind errors erased to `Box<dyn Error>`** — "Address in use" returns generic 500 instead of 409. Introduce typed bind error enum. | Architecture, Rust Idioms |
| 11 | **Bind response hides actual address in human string** — Machine clients must parse `"Bound swerve socket on 127.0.0.1:9741"`. Add typed `BindSocketResponse { addr }`. | Architecture |
| 12 | **Single `RwLock` for all state** — Files, serve index, and sockets share one lock. Split by concern for scalability. | Architecture |
| 13 | **`status` command checks unauthenticated `/health`** — Only validates reachability, not credentials. Add authenticated `/auth-check` endpoint. | Architecture |
| 14 | **Upload cancellation leaves orphaned file** — If future dropped after `fs::write` but before state insertion, blob remains until restart. Use temp file + rename guard. | Rust Idioms |
| 15 | **Widespread `Box<dyn Error>` erases structure** — Client, config, and server startup use `format!` errors losing source chains. Use `thiserror` enums. | Rust Idioms |
| 16 | **Flat CLI command structure** — File actions split across top-level verbs. Group under `file` subcommand with current verbs as aliases. | CLI UX |
| 17 | **`completions` ignores `--json`/`--quiet`** — Still emits shell script. Mark flags incompatible or document. | CLI UX |
| 18 | **Raw reqwest errors lack context** — Transport failures don't include operation, target URL, or guidance. Wrap with context. | CLI UX |
| 19 | **`config show` doesn't distinguish value sources** — Env/CLI overrides are invisible. Separate saved vs effective, or annotate sources. | CLI UX |
| 20 | **`config set` requires both fields** — Partial updates awkward. Allow merging with existing config. | CLI UX |
| 21 | **Unvalidated URL/addr string args** — Malformed input fails late. Use typed parsers (`Url`, `SocketAddr`). | CLI UX |
| 22 | **Malformed JSON/query tests missing** — No tests for invalid bodies on serve-state, serve-name, sockets endpoints. | Test Coverage |
| 23 | **Concurrent bind/unbind races untested** — Parallel binds for same addr should yield one success + one conflict. | Test Coverage |
| 24 | **Filename sanitization untested at HTTP level** — Quotes/CRLF/slashes in names not asserted in Content-Disposition headers. | Test Coverage |

### LOW Findings

| # | Finding | Source |
|---|---------|--------|
| 25 | Download/serve code duplication between management and swerve socket paths | Architecture |
| 26 | Socket addresses modeled as `String` instead of `SocketAddr` | Rust Idioms |
| 27 | Internal server types publicly exported (unnecessary semver surface) | Rust Idioms |
| 28 | Clippy test-only `collapsible_if` in config tests | Rust Idioms |
| 29 | No progress/feedback for large transfers | CLI UX |
| 30 | No `ValueHint` on file/URL args for shell completions | CLI UX |
| 31 | Download write error missing destination path | CLI UX |
| 32 | Config tests depend on real HOME directory | Test Coverage |
| 33 | Truncated multipart test allows OK or BAD_REQUEST (masks regressions) | Test Coverage |
