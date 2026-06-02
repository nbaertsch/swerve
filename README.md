# swerve

Encrypted file staging and serving system. Upload files to a remote server where they're stored encrypted, then serve them on configurable network sockets under spoofed filenames.

## Architecture

- **`swerve`** — Server binary. Runs on the remote machine. Provides a management API (authenticated via API key) and dynamically-created swerve sockets for file serving.
- **`fswerve`** — CLI client. Runs locally. Manages files, serving state, and socket bindings on the remote swerve server.
- **`swerve-core`** — Shared library. Types, crypto (AES-256-GCM), and API contracts.

## Quick Start

### Build

```bash
cargo build --release
```

### Start the server

```bash
./target/release/swerve --api-key YOUR_SECRET_KEY
# or via environment variable:
SWERVE_API_KEY=YOUR_SECRET_KEY ./target/release/swerve
```

Options:
- `-b, --bind <ADDR>` — Management API bind address (default: `0.0.0.0:9740`)
- `-k, --api-key <KEY>` — API key for authentication (or `SWERVE_API_KEY` env var)
- `-s, --storage-dir <PATH>` — Encrypted file storage directory (default: system temp)

### Configure the client

```bash
fswerve config set --server-url http://10.0.0.5:9740 --api-key YOUR_SECRET_KEY
```

### Upload a file

```bash
# Upload with original filename
fswerve upload ./payload.bin

# Upload with a spoofed serve name
fswerve upload ./malware.exe --serve-as update.exe
```

### Manage serving

```bash
# List files
fswerve files

# Enable/disable serving
fswerve serve enable payload.bin
fswerve serve disable payload.bin

# Change the spoofed serve name
fswerve serve rename payload.bin --name installer.exe
```

### Manage swerve sockets

```bash
# Bind a serving socket
fswerve sockets bind 0.0.0.0:8080

# List active sockets
fswerve sockets list

# Unbind a socket
fswerve sockets unbind 0.0.0.0:8080
```

### Download files (always via management API)

```bash
fswerve download payload.bin -o ./local_copy.bin
```

### Delete files

```bash
fswerve destroy payload.bin
```

## How It Works

1. Files uploaded via `fswerve upload` are encrypted with AES-256-GCM (random per-file key) and stored on disk with SHA-256 hashed filenames.
2. All metadata (real name, serve name, encryption keys, serving state) is kept **in-memory only** — lost on server restart.
3. Swerve sockets are HTTP listeners created on-demand. They serve only files with `serving: true`, under their configured `serve_name`.
4. The management API is always available for upload, download, and control, authenticated via API key.

## Security Notes

- **Management API should not be exposed over untrusted networks** — the API key is sent in plaintext headers. Use over SSH tunnels, VPNs, or add TLS termination.
- Files on disk are always encrypted. Encryption keys exist only in server memory.
- Server cleans and recreates the storage directory on startup.
