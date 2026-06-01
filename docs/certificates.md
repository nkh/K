# Certificate Management Guide

vrw supports a **certificate pool** — a set of named certificates that can be used for per-command access control. This guide explains how to generate, configure, and use certificates to isolate running applications within a vrw instance.

---

## Table of Contents

1. [Overview](#overview)
2. [Certificate Concepts](#certificate-concepts)
3. [Generating Certificates](#generating-certificates)
4. [Configuring the Certificate Pool](#configuring-the-certificate-pool)
5. [Binding Certificates to Commands](#binding-certificates-to-commands)
6. [Using Certificate Tokens for API Access](#using-certificate-tokens-for-api-access)
7. [Per-Instance Certificates](#per-instance-certificates)
8. [Complete Examples](#complete-examples)
9. [Security Model](#security-model)
10. [API Reference](#api-reference)

---

## Overview

vrw's certificate system provides **per-command isolation** within a single instance. Each certificate in the pool can be bound to a running command, ensuring that only clients presenting the correct certificate (or its derived bearer token) can interact with that command's endpoints (VTTY, keys, kill).

```
┌─────────────────────────────────────────────────────────────┐
│  vrw instance (port 8080)                               │
│                                                              │
│  Certificate Pool:                                           │
│  ┌──────────────────┐  ┌──────────────────┐                  │
│  │ "frontend-app"   │  │ "ci-pipeline"    │                  │
│  │ token: a3f8...2b │  │ token: 7c1e...9d │                  │
│  └────────┬─────────┘  └────────┬─────────┘                  │
│           │                     │                             │
│  Running Commands:                                         │
│  ┌──────────────────┐  ┌──────────────────┐                  │
│  │ htop (bound to   │  │ vim (bound to    │                  │
│  │ "frontend-app")  │  │ "ci-pipeline")   │                  │
│  │                  │  │                  │                  │
│  │ Accessible ONLY │  │ Accessible ONLY │                  │
│  │ by holders of   │  │ by holders of   │                  │
│  │ "frontend-app"  │  │ "ci-pipeline"   │                  │
│  └──────────────────┘  └──────────────────┘                  │
│                                                              │
│  Unbound commands (any authenticated user can access):      │
│  ┌──────────────────┐                                       │
│  │ python server     │                                       │
│  │ (no certificate)  │                                       │
│  └──────────────────┘                                       │
└─────────────────────────────────────────────────────────────┘
```

---

## Certificate Concepts

### Instance Certificate vs. Pool Certificates

| Concept | Purpose | Scope |
|---------|---------|-------|
| **Instance certificate** (`tls.cert_file` / `tls.key_file`) | TLS termination for the HTTPS server | The entire vrw server |
| **Pool certificates** (`certificates.entries[]`) | Per-command access control | Individual running commands |

The instance certificate is used by the server to encrypt connections. Pool certificates are used to control *who* can interact with *which* commands. Both are independent and serve different purposes.

### How Token Derivation Works

Each certificate in the pool has a **derived bearer token** — the SHA-256 hash of the certificate's PEM content, encoded as 64 hex characters. This token is used in the `Authorization` header to authenticate API requests for certificate-bound commands.

```
Certificate PEM → SHA-256 → hex encode → 64-char bearer token
```

You never need to compute this manually. vrw computes and displays it when you generate or list certificates.

### Access Control Flow

```
1. Client sends request with Authorization: Bearer <token>
2. Auth middleware checks:
   a. Is instance-level auth enabled? → validate instance token
   b. Is the command certificate-bound? → validate cert-derived token
   c. Is the command unbound? → any valid auth passes
3. If all checks pass → request proceeds to handler
```

---

## Generating Certificates

### Via CLI

```bash
# Generate a new certificate named "webapp-frontend"
vrw cert generate webapp-frontend
```

Output:
```
Certificate 'webapp-frontend' generated:
  Certificate: ~/.config/vrw/certs/webapp-frontend/cert.pem
  Key:         ~/.config/vrw/certs/webapp-frontend/key.pem
  Token:       a3f8c1e2b7d9400123456789abcdef01...
```

### List All Certificates

```bash
vrw cert list
```

Output:
```
NAME              CERT FILE                                        TOKEN (prefix)
webapp-frontend   ~/.config/vrw/certs/webapp-frontend/cert.pem   a3f8c1e2b7d9...
ci-pipeline       ~/.config/vrw/certs/ci-pipeline/cert.pem       7c1e9d4a8f2...
```

### Show Certificate Details

```bash
vrw cert show webapp-frontend
```

### Remove a Certificate

```bash
vrw cert remove webapp-frontend
```

Note: This removes the certificate from the pool registry but does not delete the PEM files.

---

## Configuring the Certificate Pool

### Via Configuration File

Add a `certificates` section to your `vrw.yaml`:

```yaml
certificates:
  directory: "~/.config/vrw/certs"

  entries:
    - name: "webapp-frontend"
      cert_file: "webapp-frontend/cert.pem"
      key_file: "webapp-frontend/key.pem"

    - name: "ci-pipeline"
      cert_file: "/etc/ssl/certs/ci-pipeline.crt"
      key_file: "/etc/ssl/private/ci-pipeline.key"

    - name: "staging-app"
      cert_file: "staging-app/cert.pem"
      key_file: "staging-app/key.pem"
```

- `directory`: Base directory for resolving relative cert/key paths. Default: `~/.config/vrw/certs/`.
- `entries`: List of named certificates. If the cert and key files don't exist, vrw auto-generates them.

### Via CLI Flags

Use the `--certificate` flag with the format `NAME:CERT_FILE:KEY_FILE`:

```bash
vrw --certificate "webapp-frontend:./certs/frontend/cert.pem:./certs/frontend/key.pem" \
       --certificate "ci-pipeline:/etc/ssl/certs/ci.pem:/etc/ssl/private/ci.key" \
       -- some-command
```

### Via Configuration File Locations

| Priority | Source | Path |
|----------|--------|------|
| Highest | CLI flags | `--certificate` arguments |
| High | Local config | `./vrw.yaml` |
| Medium | Global config | `~/.config/vrw/config.yaml` |
| Lowest | Built-in defaults | No certificates |

---

## Binding Certificates to Commands

When starting a command via the API, specify the `certificate` field to bind it:

```bash
# Start htop, bound to the "webapp-frontend" certificate
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <cert-token>" \
  -d '{
    "cmd": "htop",
    "args": [],
    "certificate": "webapp-frontend"
  }'
```

Response:
```json
{
  "status": "ok",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000"
  },
  "error": null
}
```

After binding, only clients that provide the `webapp-frontend` certificate's derived token can:
- View the VTTY output
- Send keystrokes
- Kill the command
- Add handles

### Starting Without a Certificate

Omit the `certificate` field to start an unbound command. Unbound commands are accessible to any authenticated client (using the instance-level token or no auth in localhost mode):

```bash
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "python",
    "args": ["-m", "http.server", "8000"]
  }'
```

---

## Using Certificate Tokens for API Access

### 1. Generate or Find Your Token

```bash
vrw cert show webapp-frontend
```

The output includes the full 64-character token.

### 2. Use the Token in API Requests

```bash
TOKEN="a3f8c1e2b7d9400123456789abcdef0123456789abcdef0123456789abcdef01"

# View VTTY
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/commands/<id>/vtty

# Send keystrokes
curl -X POST -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"keys": "q"}' \
  http://localhost:8080/api/commands/<id>/keys

# Kill the command
curl -X POST -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/commands/<id>/kill
```

### 3. Use with curl and TLS

```bash
curl --cacert ~/.config/vrw/cert.pem \
     -H "Authorization: Bearer $CERT_TOKEN" \
     https://localhost:8080/api/commands
```

---

## Per-Instance Certificates

Different vrw instances can use completely different certificates. Each instance has its own config, its own certificate pool, and its own TLS server certificate.

### Instance A (port 8080) — Development

```yaml
# /home/user/.config/vrw/config.yaml
tls:
  enabled: true

certificates:
  entries:
    - name: "dev-frontend"
      cert_file: "dev-frontend/cert.pem"
      key_file: "dev-frontend/key.pem"
```

```bash
vrw --port 8080 -- htop
```

### Instance B (port 9090) — Staging

```yaml
# ./vrw.yaml (project directory)
tls:
  enabled: true
  cert_file: "/etc/ssl/staging/cert.pem"
  key_file: "/etc/ssl/staging/key.pem"

certificates:
  entries:
    - name: "staging-app"
      cert_file: "staging-app/cert.pem"
      key_file: "staging-app/key.pem"
```

```bash
vrw --port 9090 --config ./vrw.yaml -- npm run start
```

Each instance is independent — the "dev-frontend" certificate from Instance A cannot be used to access commands in Instance B.

---

## Complete Examples

### Example 1: Multi-Tenant Web Hosting

A server runs three web applications, each managed by a different team. Each team gets their own certificate.

```yaml
# vrw.yaml
server:
  bind: "0.0.0.0"
  port: 8080

security:
  require_auth: true

tls:
  enabled: true

certificates:
  entries:
    - name: "team-alpha"
      cert_file: "certs/team-alpha/cert.pem"
      key_file: "certs/team-alpha/key.pem"
    - name: "team-beta"
      cert_file: "certs/team-beta/cert.pem"
      key_file: "certs/team-beta/key.pem"
    - name: "team-ops"
      cert_file: "certs/team-ops/cert.pem"
      key_file: "certs/team-ops/key.pem"
```

Each team starts their application with their certificate:

```bash
# Team Alpha starts their app
vrw cert generate team-alpha

curl -X POST https://server:8080/api/commands \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <alpha-token>" \
  -d '{"cmd": "npm", "args": ["run", "start:alpha"], "certificate": "team-alpha"}'

# Team Beta starts their app
curl -X POST https://server:8080/api/commands \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <beta-token>" \
  -d '{"cmd": "npm", "args": ["run", "start:beta"], "certificate": "team-beta"}'
```

Team Alpha can only see and control their own app. They cannot access Team Beta's VTTY, send keys to it, or kill it.

### Example 2: CI/CD Pipeline Isolation

A CI server uses vrw to run build jobs. Each job is isolated by certificate.

```bash
# Generate cert for the CI pipeline
vrw cert generate ci-pipeline

# Start the vrw server
vrw --remote --tls -- daemon

# CI script starts a build job
TOKEN=$(vrw cert show ci-pipeline | grep Token | awk '{print $2}')

JOB_ID=$(curl -s -X POST https://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"cmd": "./build.sh", "args": ["--release"], "certificate": "ci-pipeline"}' \
  | jq -r '.data.id')

# Monitor the build output
curl -s -H "Authorization: Bearer $TOKEN" \
  "https://localhost:8080/api/commands/$JOB_ID/vtty"
```

### Example 3: Localhost with Certificate-Bound Commands

Even on localhost (no instance auth), you can still use certificates for logical isolation between commands:

```bash
# Start server (no auth required for localhost)
vrw

# Generate certs for different projects
vrw cert generate project-a
vrw cert generate project-b

# Start project A's server, bound to its cert
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "node", "args": ["server.js"], "certificate": "project-a"}'

# Start project B's server, bound to its cert
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "python", "args": ["-m", "http.server", "3000"], "certificate": "project-b"}'

# List all commands (no auth needed on localhost)
curl http://localhost:8080/api/commands
# → Returns both commands with their certificate names

# Only project-a cert holder can interact with project A
curl -H "Authorization: Bearer <project-a-token>" \
  http://localhost:8080/api/commands/<id>/vtty
```

### Example 4: Mixed Bound and Unbound Commands

You can mix certificate-bound and unbound commands in the same instance:

```bash
# Unbound command (anyone with instance auth can access)
curl -X POST http://localhost:8080/api/commands \
  -d '{"cmd": "htop", "args": []}'

# Certificate-bound command (only cert holder can access)
curl -X POST http://localhost:8080/api/commands \
  -d '{"cmd": "vim", "args": ["notes.txt"], "certificate": "my-notes"}'
```

### Example 5: Using CLI --certificate Flag

Define certificates directly on the command line without a config file:

```bash
vrw \
  --tls \
  --certificate "frontend:./certs/frontend/cert.pem:./certs/frontend/key.pem" \
  --certificate "backend:./certs/backend/cert.pem:./certs/backend/key.pem" \
  -- npm run dev
```

---

## Security Model

### Certificate Properties

| Property | Value |
|----------|-------|
| Key algorithm | EC (via `rcgen` defaults) |
| Key usage | Digital Signature, Key Encipherment |
| Extended key usage | Server Authentication, Client Authentication |
| Validity | 2025-01-01 to 2030-01-01 |
| Key file permissions | `0600` (owner read/write only on Unix) |
| Token derivation | SHA-256 of certificate PEM, hex-encoded (64 chars) |

### Trust Model

1. **Certificate generation is local** — vrw generates certificates on the server machine. They are not issued by a public CA.
2. **Tokens are derived, not random** — The bearer token is a deterministic hash of the certificate content. This means the same certificate always produces the same token.
3. **Certificates are identity documents** — They represent "who you are" in the vrw access control model. Holding a certificate's token proves you are the intended recipient for commands bound to that certificate.
4. **No client certificate verification on the server** — The server does not require mTLS. Instead, the certificate's derived bearer token is used for authentication via the standard `Authorization` header. This simplifies client implementation while maintaining the same security properties.

### Distributing Certificates to Clients

Since pool certificates are self-signed (not issued by a public CA), the certificate files must be distributed to authorized clients out of band. Common methods:

- **Shared filesystem**: NFS mount, shared volume
- **Secret management**: HashiCorp Vault, AWS Secrets Manager, Kubernetes Secrets
- **Secure copy**: `scp`, `sftp`
- **Configuration management**: Ansible, Puppet, Chef

---

## API Reference

### GET /api/certificates

List all certificates in the pool.

**Response:**
```json
{
  "status": "ok",
  "data": [
    {
      "name": "webapp-frontend",
      "cert_file": "/home/user/.config/vrw/certs/webapp-frontend/cert.pem",
      "key_file": "/home/user/.config/vrw/certs/webapp-frontend/key.pem",
      "token_preview": "a3f8c1e2b7d94001"
    }
  ],
  "error": null
}
```

### POST /api/commands (with certificate)

Start a new command, optionally bound to a certificate.

**Request:**
```json
{
  "cmd": "htop",
  "args": [],
  "certificate": "webapp-frontend"
}
```

**Response:**
```json
{
  "status": "ok",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000"
  },
  "error": null
}
```

### GET /api/commands (with certificate info)

List all running commands, including their certificate bindings.

**Response:**
```json
{
  "status": "ok",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "htop",
      "pid": 12345,
      "status": "running",
      "certificate": "webapp-frontend"
    },
    {
      "id": "660e8400-e29b-41d4-a716-446655440001",
      "name": "python",
      "pid": 12346,
      "status": "running",
      "certificate": null
    }
  ],
  "error": null
}
```

### CLI Subcommands

| Command | Description |
|---------|-------------|
| `vrw cert generate <name>` | Generate a new named certificate |
| `vrw cert list` | List all certificates in the pool |
| `vrw cert show <name>` | Show full details including token |
| `vrw cert remove <name>` | Remove a certificate from the pool |
