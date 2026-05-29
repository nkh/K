# Security Model

This document describes how vrunner approaches security: from its secure-by-default
philosophy through the authentication flow, token management, TLS setup, the
certificate pool access-control system, CORS policy, and production hardening
recommendations. Read this if you are deploying vrunner on a shared network, exposing
it over the internet, or need to understand how authentication and encryption are
implemented.

---

## Secure-by-Default Philosophy

vrunner is designed so that the **default configuration is safe for local-only use**
and **unsafe configurations require explicit opt-in**. Specifically:

| Behavior | Default | Requires Opt-In |
|---|---|---|
| Bind address | `127.0.0.1` (loopback only) | `--bind 0.0.0.0` (or `--remote`) to accept external connections |
| Authentication | None (no token required) | `--auth` to require a bearer token |
| TLS | Disabled | `--tls` to enable |
| CORS | All origins allowed (`policy: "any"`) | Set `security.cors.policy` in the config file to restrict origins |

This means that if you simply run `vrunner -- my-command`, the only clients that
can connect are processes on the same machine, and no authentication is needed. The
security surface is minimal.

**Risk**: If you bind to `0.0.0.0` without enabling authentication, any client on
the network can start, stop, and interact with your commands. vrunner warns on
stderr when `--bind` is set to a non-loopback address and `--auth` is not
enabled. The `--remote` flag automatically enables both external binding and
authentication to prevent this misconfiguration.

---

## Authentication Flow

When authentication is enabled, every HTTP request passes through the following
middleware pipeline:

```
┌──────────┐          ┌──────────┐          ┌───────────────┐          ┌──────────┐
│  Client   │          │  CORS    │          │  Auth          │          │  Handler  │
│  Request  │─────────►│  Check   │─────────►│  Middleware    │─────────►│  Logic    │
└──────────┘          └────┬─────┘          └───────┬───────┘          └──────────┘
                           │                        │
                           │  Origin not allowed?    │  Token missing/invalid?
                           ▼                        ▼
                     ┌──────────┐             ┌──────────┐
                     │  403     │             │  401     │
                     │  CORS    │             │  Auth    │
                     └──────────┘             └──────────┘
```

### Step 1: CORS Middleware

The CORS middleware runs first. By default, the policy is `"any"`, which allows
all cross-origin requests. To restrict which origins can access the API, set
`security.cors.policy` in the config file (see the [CORS Policy](#cors-policy)
section below). If the policy does not permit the request's `Origin` header,
the middleware returns a `403 Forbidden` response.

### Step 2: Authentication Middleware

If auth is enabled (`--auth`), the middleware reads the token from the configured
token file (default `~/.config/vrunner/token`) and extracts the `Bearer` token
from the `Authorization` header:

```
Authorization: Bearer <256-bit-hex-token>
```

If the header is missing or the token does not match, the middleware returns
`401 Unauthorized`. If auth is not enabled, this step is skipped (pass-through).

### Step 3: Handler Execution

Only requests that pass both checks reach the handler logic. Handlers are never
exposed to unauthenticated traffic when authentication is enabled.

### WebSocket Authentication

WebSocket connections are authenticated during the HTTP upgrade request. The same
`Authorization: Bearer` header is checked. After the upgrade, the WebSocket
connection is trusted for its lifetime—there is no per-message authentication.

---

## Token Generation and Storage

### Token Format

When you enable auth with `--auth`, vrunner creates a **256-bit (32-byte)**
cryptographically random token encoded as **64 hex characters**:

```
a3f1b2c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2
```

The token is generated using `rand::rngs::OsRng`, which draws from the operating
system's CSPRNG (`getrandom` on Linux/macOS, `BCryptGenRandom` on Windows).

### Token File

When auth is enabled, vrunner looks for the token at the configured path
(default `~/.config/vrunner/token`, overridable with `--token-file <FILE>`).
If the file does not exist, a random token is generated and written to the file.
The file is created with `0600` permissions (owner read/write only):

```
-rw------- 1 user user 64 Jun 15 10:23 ~/.config/vrunner/token
```

This ensures that other users on the same system cannot read the token.

### Token Lifecycle

```
┌──────────────┐     vrunner --tls --auth      ┌──────────────┐
│  Generation  │     (with --token-file or      │  Storage     │
│  (OsRng)     │      default path)              │  (file 0600) │
└──────────────┘──────────────────────────────►└──────┬───────┘
                                                              │
                    ┌─────────────────────────────────────────┘
                    │
                    ▼
           ┌──────────────┐
           │  Loading      │  vrunner reads token from file:
           │               │  ~/.config/vrunner/token
           │               │  (or --token-file <FILE> path)
           └──────┬───────┘
                  │
                  ▼
           ┌──────────────┐
           │  Validation  │  Middleware checks every request's
           │               │  Authorization header against token
           └──────────────┘
```

---

## TLS Setup Flow

vrunner uses **pure-Rust TLS** via the `rustls` crate. There is no dependency on
OpenSSL, LibreSSL, or any system TLS library. This reduces the attack surface
and simplifies cross-platform deployment.

### Option A: Auto-Generated Certificates (Default TLS)

When you pass `--tls` without specifying certificate paths:

```
vrunner --tls --auth -- my-app
```

vrunner generates a self-signed X.509 certificate using `rcgen`:

```
┌──────────────┐
│  rcgen       │
│  Certificate │  Subject: CN=vrunner, O=vrunner
│  Generator   │  SAN:    DNS:localhost, IP:127.0.0.1, IP:::1
│              │  Validity: 2025-01-01 to 2030-01-01 (5 years)
│  Key:        │  ECDSA P-256
│  Algorithm   │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  rustls      │
│  ServerConfig│  Built from generated cert + private key
│              │  Cipher suites: TLS 1.2 + TLS 1.3 only
└──────────────┘
```

The certificate and key are written to `~/.config/vrunner/cert.pem` and
`~/.config/vrunner/key.pem` respectively. On subsequent runs, the existing
files are reused automatically. The key file is created with `0600` permissions
(owner read/write only).

> **Note**: Distribute `~/.config/vrunner/cert.pem` to authorized clients so they
> can trust the self-signed certificate (e.g., `curl --cacert cert.pem`).

### Option B: Custom Certificates

For production use, you can provide your own certificates:

```
vrunner --tls --cert-file /etc/ssl/certs/vrunner.pem --key-file /etc/ssl/private/vrunner.key -- my-app
```

The certificate and key are loaded via `rustls-pemfile`. Supported formats:

- PEM-encoded X.509 certificates
- PEM-encoded PKCS#1 or PKCS#8 private keys
- Certificate chains (leaf + intermediates)

```
┌──────────────┐     PEM files      ┌──────────────┐
│  File System │───────────────────►│  rustls      │
│  *.pem       │                     │  ServerConfig│
└──────────────┘                     └──────────────┘
```

---

## Certificate Pool System

vrunner provides an advanced access-control mechanism called the **certificate
pool**. Instead of a single shared token, each named certificate has its own
dedicated access token derived from the certificate's PEM content.

### How It Works

1. Named certificates are defined in the config file or via `--certificate NAME:CERT_FILE:KEY_FILE`.
2. For each certificate, a bearer token is derived by hashing the PEM-encoded
   certificate file contents:

```
per_certificate_token = SHA-256(certificate_PEM_bytes)
```

3. Clients that present a specific certificate's derived token are identified as
   holders of that certificate. The global auth token (if set) grants access to
   all commands regardless of certificate membership.

```
┌──────────────────────────────────────────────────────┐
│                  Token Hierarchy                      │
│                                                       │
│  Global Token (from auth token file)                  │
│  ├── Access to ALL commands                           │
│  │                                                    │
│  └── Per-Certificate Tokens (cert pool)               │
│      ├── token_for("webapp-frontend")  → frontend only│
│      ├── token_for("database")         → database only│
│      └── token_for("cache")            → cache only   │
└──────────────────────────────────────────────────────┘
```

This allows fine-grained access control: you can give a team member access to
the `logs` command by giving them the `logs` certificate token without giving
them the global token.

### Token Derivation

The derivation is deterministic—given the same certificate file, any party can
compute the token without needing to store or transmit it:

```rust
use sha2::{Sha256, Digest};

fn derive_certificate_token(cert_pem: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cert_pem);
    hex::encode(hasher.finalize())
}
```

### Managing Certificates

Certificates in the pool can be managed via the `vrunner cert` subcommand:

| Command | Description |
|---|---|
| `vrunner cert generate <name>` | Generate a new named certificate |
| `vrunner cert list` | List all certificates in the pool |
| `vrunner cert show <name>` | Show details of a specific certificate |
| `vrunner cert remove <name>` | Remove a certificate from the pool |

Auto-generated certificates are stored in `~/.config/vrunner/certs/<name>/` as
`cert.pem` and `key.pem`. Private key files are created with `0600` permissions.

---

## CORS Policy

Cross-Origin Resource Sharing (CORS) controls which web origins can make requests
to vrunner's API. The policy is configured via the config file under
`security.cors.policy`:

| Configuration | Behavior |
|---|---|
| `policy: "any"` (default) | All origins are allowed |
| `policy: "none"` | No `Access-Control-Allow-Origin` header is set; same-origin only |
| `policy: "https://app.example.com"` | Only the specified origin is allowed |
| `policy: "https://app.example.com,https://admin.example.com"` | Comma-separated list of allowed origins |

> **Note**: There is no CLI flag for CORS. Use the config file (`~/.config/vrunner/config.toml`
> or `--config <FILE>`) to set `security.cors.policy`.

The CORS middleware sets the following headers on allowed requests:

```
Access-Control-Allow-Origin: <configured-origin>
Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS
Access-Control-Allow-Headers: Authorization, Content-Type
Access-Control-Max-Age: 3600
```

Pre-flight `OPTIONS` requests are handled automatically by the middleware.

---

## Security Best Practices for Production

When deploying vrunner on a shared network or the internet, follow these
guidelines:

### Network

| Recommendation | Reason |
|---|---|
| Use `--remote` for simple external access (binds to `0.0.0.0` and enables auth) | Prevents accidental exposure without auth |
| Or bind to a specific interface with `--bind` | Avoid exposing on all interfaces unnecessarily |
| Use a reverse proxy (nginx, Caddy) | Adds rate limiting, DDoS protection, logging |
| Place behind a VPN or SSH tunnel | Reduces the attack surface to trusted networks |

### Authentication

| Recommendation | Reason |
|---|---|
| Always set `--auth` when binding externally | Prevents unauthorized access |
| Use `--token-file <FILE>` to point to a pre-generated token | Avoids the token appearing in shell history |
| Rotate tokens periodically | Limits the window of compromise |
| Use per-certificate tokens (cert pool) for multi-user setups | Principle of least privilege |

### TLS

| Recommendation | Reason |
|---|---|
| Use `--tls` for all remote access | Encrypts terminal I/O and API traffic |
| Use custom certificates from a trusted CA for production | Avoids browser warnings |
| Rotate certificates before expiry | Prevents connection failures |

### File Permissions

| Recommendation | Reason |
|---|---|
| Token file: `0600` | Only the vrunner user can read the token |
| Config file: `0600` or `0640` | Prevents unauthorized modification |
| PID file: `0644` | Readable for monitoring, writable by owner |
| Certificate/key: `0600` | Only the vrunner user can read private keys |

### Process Isolation

| Recommendation | Reason |
|---|---|
| Run vrunner as a dedicated user | Limits the impact of a compromised process |
| Use container isolation (Docker/Podman) | Adds filesystem and network isolation |
| Set resource limits (ulimit, cgroups) | Prevents resource exhaustion |

---

## Threat Model Summary

| Threat | Mitigation |
|---|---|
| Unauthorized local access | Default loopback binding; no auth needed locally |
| Unauthorized remote access | `--remote` binds to `0.0.0.0` with mandatory `--auth`; manual `--bind` warns if `--auth` is not set |
| Token theft from process list | Token is read from a file (`--token-file`), not passed as a CLI argument |
| Token theft from filesystem | File permissions `0600` |
| Eavesdropping on network | TLS with `rustls` (TLS 1.2 + 1.3) |
| Cross-site attacks | Configurable CORS policy (default: allow all; restrict via `security.cors.policy`) |
| Certificate compromise | Per-certificate token derivation from cert PEM content |
| Compromised child process | Isolated PTY; child cannot access vrunner internals |
| Denial of service | Rate limiting via reverse proxy; bounded channels |

---

*This document is part of the [Diátaxis](https://diataxis.fr/) documentation framework
for vrunner. See the [explanation index](./) for related topics.*
