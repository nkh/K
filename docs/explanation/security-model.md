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
| Bind address | `127.0.0.1` (loopback only) | `--host 0.0.0.0` to accept external connections |
| Authentication | None (no token required) | `--auth-token <value>` or `--auth-token-file <path>` |
| TLS | Disabled | `--tls` to enable |
| CORS | All origins rejected | `--cors-origin <origin>` to allow specific origins |

This means that if you simply run `vrunner run -- my-command`, the only clients that
can connect are processes on the same machine, and no authentication is needed. The
security surface is minimal.

**Risk**: If you bind to `0.0.0.0` without enabling authentication, any client on
the network can start, stop, and interact with your commands. vrunner warns on
stderr when `--host` is set to a non-loopback address and `--auth-token` is not
provided.

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

The CORS middleware runs first. If the request's `Origin` header does not match
the configured allowed origin, the middleware returns a `403 Forbidden` response.
If CORS is not configured (the default), all cross-origin requests are rejected.
Same-origin requests (e.g., from the served admin UI) are always allowed.

### Step 2: Authentication Middleware

If an auth token is configured, the middleware extracts the `Bearer` token from
the `Authorization` header:

```
Authorization: Bearer <256-bit-hex-token>
```

If the header is missing or the token does not match, the middleware returns
`401 Unauthorized`. If no auth token is configured, this step is skipped
(pass-through).

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

When you use `--auth-token-file <path>` (or the auto-generated token), vrunner
creates a **256-bit (32-byte)** cryptographically random token encoded as **64
hex characters**:

```
a3f1b2c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2
```

The token is generated using `rand::rngs::OsRng`, which draws from the operating
system's CSPRNG (`getrandom` on Linux/macOS, `BCryptGenRandom` on Windows).

### Token File Permissions

When a token is written to a file (e.g., at `~/.config/vrunner/auth-token`), the
file is created with `0600` permissions (owner read/write only):

```
-rw------- 1 user user 64 Jun 15 10:23 ~/.config/vrunner/auth-token
```

This ensures that other users on the same system cannot read the token.

### Token Lifecycle

```
┌──────────────┐     vrunner run --tls --auth-token     ┌──────────────┐
│  Generation  │     (or --auth-token-file)            │  Storage     │
│  (OsRng)     │──────────────────────────────────────►│  (file 0600) │
└──────────────┘                                        └──────┬───────┘
                                                              │
                    ┌─────────────────────────────────────────┘
                    │
                    ▼
           ┌──────────────┐
           │  Loading      │  vrunner reads token from:
           │               │  1. CLI flag value
           │               │  2. File contents
           │               │  3. (error if neither)
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
vrunner run --tls --auth-token secret -- my-app
```

vrunner generates a self-signed X.509 certificate using `rcgen`:

```
┌──────────────┐
│  rcgen       │
│  Certificate │  Subject: CN=vrunner
│  Generator   │  SAN:    DNS:localhost, IP:127.0.0.1
│              │  Validity: 365 days
│  Key:        │  ECDSA P-256 (or configurable RSA-2048)
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

The certificate and key are held in memory only—they are **never written to disk**
when auto-generated. This means they are lost when vrunner exits, which is fine
for development and short-lived sessions.

### Option B: Custom Certificates

For production use, you can provide your own certificates:

```
vrunner run --tls --cert /etc/ssl/certs/vrunner.pem --key /etc/ssl/private/vrunner.key -- my-app
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
pool**. Instead of a single shared token, each command can have its own
dedicated access token derived from the server's TLS certificate fingerprint.

### How It Works

1. When TLS is enabled, vrunner computes the **SHA-256 fingerprint** of the
   server certificate.
2. For each command, a per-command access token is derived:

```
per_command_token = SHA-256(fingerprint || command_name)
```

3. Clients that present a specific per-command token can only interact with
   the corresponding command. The global auth token (if set) grants access to
   all commands.

```
┌──────────────────────────────────────────────────────┐
│                  Token Hierarchy                      │
│                                                       │
│  Global Token (auth_token)                            │
│  ├── Access to ALL commands                           │
│  │                                                    │
│  └── Per-Command Tokens (cert pool)                   │
│      ├── token_for("web-server")  → "web-server" only│
│      ├── token_for("database")    → "database" only  │
│      └── token_for("cache")       → "cache" only     │
└──────────────────────────────────────────────────────┘
```

This allows fine-grained access control: you can give a team member access to
the `logs` command without giving them access to the `database` command.

### Token Derivation

```rust
use sha2::{Sha256, Digest};

fn derive_command_token(cert_fingerprint: &[u8], command_name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cert_fingerprint);
    hasher.update(command_name.as_bytes());
    hex::encode(hasher.finalize())
}
```

The derivation is deterministic—given the same certificate and command name,
any party can compute the token without needing to store or transmit it.

---

## CORS Policy

Cross-Origin Resource Sharing (CORS) controls which web origins can make requests
to vrunner's API. The policy is:

| Configuration | Behavior |
|---|---|
| No `--cors-origin` set | All cross-origin requests are rejected (same-origin only) |
| `--cors-origin https://app.example.com` | Only the specified origin is allowed |
| `--cors-origin *` | All origins are allowed (**not recommended for production**) |

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
| Bind to a specific interface (`--host`) | Avoid exposing on all interfaces unnecessarily |
| Use a reverse proxy (nginx, Caddy) | Adds rate limiting, DDoS protection, logging |
| Place behind a VPN or SSH tunnel | Reduces the attack surface to trusted networks |

### Authentication

| Recommendation | Reason |
|---|---|
| Always set `--auth-token` when binding externally | Prevents unauthorized access |
| Use `--auth-token-file` for long-lived tokens | Avoids token in shell history / process list |
| Rotate tokens periodically | Limits the window of compromise |
| Use per-command tokens (cert pool) for multi-user setups | Principle of least privilege |

### TLS

| Recommendation | Reason |
|---|---|
| Use `--tls` for all remote access | Encrypts terminal I/O and API traffic |
| Use custom certificates from a trusted CA for production | Avoids browser warnings |
| Set `--tls-verify-client` for mutual TLS (mTLS) | Ensures only trusted clients can connect |
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
| Use `--chroot` or namespace isolation (if available) | Further isolates child processes |

---

## Threat Model Summary

| Threat | Mitigation |
|---|---|
| Unauthorized local access | Default loopback binding; no auth needed locally |
| Unauthorized remote access | Explicit `--host` + mandatory `--auth-token` warning |
| Token theft from process list | Support for `--auth-token-file` (token not in argv) |
| Token theft from filesystem | File permissions `0600` |
| Eavesdropping on network | TLS with `rustls` (TLS 1.2 + 1.3) |
| Cross-site attacks | CORS policy with explicit origin allowlisting |
| Certificate compromise | Per-command token derivation from cert fingerprint |
| Compromised child process | Isolated PTY; child cannot access vrunner internals |
| Denial of service | Rate limiting via reverse proxy; bounded channels |

---

*This document is part of the [Diátaxis](https://diataxis.fr/) documentation framework
for vrunner. See the [explanation index](./) for related topics.*
