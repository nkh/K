# Remote TLS Access

Learn how to set up secure remote access to your vrunner instance — from a quick local test to a production-grade deployment with custom certificates and reverse proxies.

## Quick Setup

For fast local or LAN testing, use the built-in TLS flag to auto-generate a self-signed certificate:

```bash
vrunner --remote --tls --port 8443
```

- `--remote` binds to `0.0.0.0` instead of `localhost`.
- `--tls` enables HTTPS and WSS with an auto-generated certificate.

Open `https://your-host:8443/admin` in a browser. You will see a certificate warning because the cert is self-signed — accept it to continue.

> **Warning:** Self-signed certificates are suitable for development only. Production deployments should use certificates from a trusted CA.

## Step-by-Step Production Setup

### 1. Obtain Certificates

Use Let's Encrypt with certbot:

```bash
sudo certbot certonly --standalone -d vrunner.example.com
# Certificates saved to:
#   /etc/letsencrypt/live/vrunner.example.com/fullchain.pem
#   /etc/letsencrypt/live/vrunner.example.com/privkey.pem
```

Or use your organization's internal CA to issue certificates.

### 2. Configure the Firewall

Open only the port you need:

```bash
# Allow HTTPS traffic on port 8443
sudo ufw allow 8443/tcp
sudo ufw reload
```

### 3. Start vrunner with Custom Certificates

```bash
vrunner --remote \
  --tls \
  --port 8443 \
  --cert /etc/letsencrypt/live/vrunner.example.com/fullchain.pem \
  --key /etc/letsencrypt/live/vrunner.example.com/privkey.pem \
  --daemon
```

### 4. Verify the Connection

```bash
curl -v https://vrunner.example.com:8443/api/commands
```

You should receive a JSON response listing any spawned commands.

## Connecting from Remote Clients

### curl

```bash
curl https://vrunner.example.com:8443/api/commands

# With client certificate (if required)
curl --cert client.pem --key client-key.pem \
  https://vrunner.example.com:8443/api/commands
```

### Browser

Navigate to `https://vrunner.example.com:8443/admin`. The dashboard works identically over TLS — all traffic (including WebSocket) is encrypted.

### WebSocket

```javascript
const ws = new WebSocket('wss://vrunner.example.com:8443/ws');
ws.onmessage = (event) => console.log(JSON.parse(event.data));
```

## Reverse Proxy with nginx

Place nginx in front of vrunner for load balancing, rate limiting, and easier certificate management:

```nginx
server {
    listen 443 ssl;
    server_name vrunner.example.com;

    ssl_certificate     /etc/letsencrypt/live/vrunner.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/vrunner.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Timeouts for long-lived WebSocket connections
        proxy_read_timeout 86400s;
        proxy_send_timeout 86400s;
    }
}
```

Start vrunner bound to localhost only (nginx handles external TLS):

```bash
vrunner --port 8080 --web --daemon
```

## Reverse Proxy with Caddy

Caddy handles TLS automatically:

```Caddyfile
vrunner.example.com {
    reverse_proxy localhost:8080
}
```

Start vrunner without TLS (Caddy terminates it):

```bash
vrunner --port 8080 --web --daemon
```

## Per-Command Certificates for Multi-Team Access

In multi-team environments, you can issue separate client certificates for different teams or services. Each certificate grants access to specific commands only.

### Generate Team Certificates

```bash
# Team A certificate
vrunner cert generate --name team-a --commands "frontend,backend"
# Outputs: team-a-cert.pem, team-a-key.pem, team-a-token

# Team B certificate
vrunner cert generate --name team-b --commands "database,migrations"
# Outputs: team-b-cert.pem, team-b-key.pem, team-b-token
```

### Bind Certificates to Commands

```bash
vrunner --tls \
  --cert server-cert.pem --key server-key.pem \
  --command "npm run dev" --name "frontend" --cert-team team-a \
  --command "npm run dev" --name "backend" --cert-team team-a \
  --command "./db-migrate" --name "migrations" --cert-team team-b
```

### Connect with Team Certificates

```bash
# Team A connects
curl --cert team-a-cert.pem --key team-a-key.pem \
  https://vrunner.example.com:8443/api/commands

# Team B connects
curl --cert team-b-cert.pem --key team-b-key.pem \
  https://vrunner.example.com:8443/api/commands
```

Each team sees only the commands their certificate permits.

For full certificate management details, see [`certificates.md`](certificates.md). For the complete API reference, see [`../api.md`](../api.md).
