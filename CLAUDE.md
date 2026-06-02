# Swerve

Encrypted file staging and serving system.

## Architecture
- Rust workspace with 3 crates: `swerve` (server), `fswerve` (CLI client), `swerve-core` (shared types)
- Server: axum-based HTTP, management API + dynamic swerve socket listeners
- Client: clap-based CLI, reqwest HTTP client
- Encryption: AES-256-GCM per-file, keys in memory
- Storage: temp dir, filenames are SHA-256 of real_name

## Build
```
cargo build --release
```

## Test
```
cargo test
```
