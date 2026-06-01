# Managing Certificates

Learn how to create, distribute, and manage per-command certificates that grant fine-grained access control to specific commands in your vrw instance.

## What Are Per-Command Certificates?

Per-command certificates are client TLS certificates that vrw validates on every API request. Each certificate is bound to a specific command (or set of commands), so holders can only interact with the commands they are authorized to access.

This is useful for:

- **Multi-team access** — Frontend team gets certs for frontend commands, DBAs get certs for database commands.
- **Third-party integrations** — CI systems get read-only certs, monitoring tools get log access.
- **Auditable access** — Every request is tied to a named certificate identity.

## Generating Certificates

### CLI Subcommands

Use the `vrw cert` subcommand to manage certificates:

```bash
# Generate a certificate bound to specific commands
vrw cert generate \
  --name team-frontend \
  --commands "frontend,storybook"

# Generate a certificate with an expiry
vrw cert generate \
  --name ci-bot \
  --commands "build,test" \
  --expires 90d

# Generate a wildcard certificate (access to all commands)
vrw cert generate \
  --name admin \
  --commands "*"
```

The command outputs three files:

- `<name>-cert.pem` — Client certificate
- `<name>-key.pem` — Client private key
- `<name>-token` — Bearer token for API authentication (alternative to cert-based auth)

### CA Configuration

For production use, configure vrw with a certificate authority:

```yaml
# ~/.config/vrw/config.yaml
tls:
  ca-cert: /etc/ssl/vrw-ca.crt
  ca-key: /etc/ssl/vrw-ca.key
  cert: /etc/ssl/vrw-server.crt
  key: /etc/ssl/vrw-server.key
```

All generated client certificates will be signed by this CA.

## Listing Certificates

View all issued certificates and their bindings:

```bash
vrw cert list
```

Output:

```
NAME              COMMANDS            ISSUED               EXPIRES
team-frontend     frontend,storybook 2025-01-15 10:00:00  -
ci-bot            build,test          2025-01-14 08:00:00  2025-04-14 08:00:00
admin             *                   2025-01-10 12:00:00  -
```

## Showing Certificate Details

Inspect a specific certificate's full details:

```bash
vrw cert show team-frontend
```

Output:

```
Name:       team-frontend
Commands:   frontend, storybook
Issued:     2025-01-15 10:00:00 UTC
Expires:    never
Fingerprint: SHA256:a1b2c3d4...
Token:       vr_tok_efgh5678
```

## Removing Certificates

Revoke an issued certificate:

```bash
vrw cert remove team-frontend
```

After removal, any requests using that certificate will be rejected with a `403 Forbidden` response.

## Config File Setup

Define certificates in your configuration file for persistent management:

```yaml
# ~/.config/vrw/config.yaml
tls:
  cert: /etc/ssl/vrw-server.crt
  key: /etc/ssl/vrw-server.key

certificates:
  - name: team-frontend
    commands: ["frontend", "storybook"]
    cert-file: /etc/ssl/clients/team-frontend.crt

  - name: team-backend
    commands: ["api", "worker", "migrations"]
    cert-file: /etc/ssl/clients/team-backend.crt

  - name: monitoring
    commands: ["api", "worker"]
    read-only: true
    cert-file: /etc/ssl/clients/monitoring.crt
```

## Binding Certificates to Commands at Spawn

When spawning commands, specify which certificate is required to access them:

```bash
vrw --tls \
  --cert /etc/ssl/server.crt --key /etc/ssl/server.key \
  --cmd "npm run dev" --name "frontend" --cert-name team-frontend \
  --cmd "./server" --name "api" --cert-name team-backend
```

## Using Certificate Tokens for API Access

Each certificate comes with a bearer token that provides an alternative to TLS client certificates. This is useful for HTTP-only clients or environments where certificate management is impractical:

```bash
# Use the token as a Bearer auth header
curl -H "Authorization: Bearer vr_tok_efgh5678" \
  https://vrw.example.com:8443/api/commands

# Use as a query parameter
curl "https://vrw.example.com:8443/api/commands?token=vr_tok_efgh5678"
```

The token inherits the same command-level restrictions as its parent certificate.

## Security Best Practices

- **Never share private keys** — Distribute cert files and key files through secure channels (e.g., HashiCorp Vault, sealed secrets).
- **Set expiry dates** — Use `--expires` for non-permanent access (CI bots, contractors).
- **Use the principle of least privilege** — Bind certificates to the minimum set of commands needed.
- **Rotate regularly** — Remove and re-issue certificates on a schedule.
- **Secure the token** — Tokens are equivalent to private keys; treat them with the same care.

For the underlying certificate architecture, see [`../certificates.md`](../certificates.md). For setting up TLS, see [`remote-tls.md`](remote-tls.md).
