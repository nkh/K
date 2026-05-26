# Remote Access via TLS

This recipe covers setting up vrunner for secure remote access over TLS, including certificate configuration and client connectivity.

## Quick Setup (Self-Signed)

The fastest way to enable secure remote access:

```bash
vrunner --remote --tls -- my-command
```

This single command:
- Binds to `0.0.0.0` (all network interfaces)
- Auto-generates a self-signed TLS certificate (saved to `~/.config/vrunner/`)
- Auto-generates a 256-bit bearer token (saved to `~/.config/vrunner/token`)
- Enables HTTPS and WSS (secure WebSocket)

## Step-by-Step Production Setup

### 1. Generate custom certificates (optional)

For production, use CA-signed certificates instead of self-signed:

```bash
# Using Let's Encrypt (requires a domain)
certbot certonly --standalone -d vrunner.example.com
# Certificates saved to /etc/letsencrypt/live/vrunner.example.com/
```

Or generate your own CA-signed certificate:

```bash
# Generate a private key and CSR
openssl genrsa -out /etc/ssl/private/vrunner.key 4096
openssl req -new -key /etc/ssl/private/vrunner.key -out /etc/ssl/csr/vrunner.csr

# Sign with your CA
openssl x509 -req -in /etc/ssl/csr/vrunner.csr \
  -CA /path/to/ca.crt -CAkey /path/to/ca.key \
  -CAcreateserial -out /etc/ssl/certs/vrunner.crt -days 365
```

### 2. Start vrunner with TLS and custom certificates

```bash
vrunner --bind 0.0.0.0 --port 443 \
  --tls \
  --cert-file /etc/ssl/certs/vrunner.crt \
  --key-file /etc/ssl/private/vrunner.key \
  --auth \
  --daemon \
  -- my-command
```

Or via the config file:

```yaml
server:
  bind: "0.0.0.0"
  port: 443

tls:
  enabled: true
  cert_file: "/etc/ssl/certs/vrunner.crt"
  key_file: "/etc/ssl/private/vrunner.key"

security:
  require_auth: true

daemon:
  enabled: true
  stdout_file: "/var/log/vrunner/stdout"
  stderr_file: "/var/log/vrunner/stderr"
```

```bash
vrunner -c /etc/vrunner/production.yaml
```

### 3. Configure the firewall

Only expose the vrunner port to trusted networks:

```bash
# UFW example
sudo ufw allow from 10.0.0.0/8 to any port 443 proto tcp
sudo ufw deny 443

# iptables example
sudo iptables -A INPUT -p tcp --dport 443 -s 10.0.0.0/8 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 443 -j DROP
```

### 4. Connect from remote clients

#### CLI (curl)

```bash
# With CA-signed certificates
curl https://vrunner.example.com/api/commands \
  -H "Authorization: Bearer $TOKEN"

# With self-signed certificates
curl --cacert ~/.config/vrunner/cert.pem https://server:443/api/commands \
  -H "Authorization: Bearer $TOKEN"

# Skip verification (not recommended for production)
curl -k https://server:443/api/commands \
  -H "Authorization: Bearer $TOKEN"
```

#### Web Browser

Navigate to `https://vrunner.example.com/admin`. For self-signed certificates, the browser will show a security warning — accept it and proceed. The admin UI will prompt for the bearer token.

#### WebSocket (JavaScript)

```javascript
const ws = new WebSocket('wss://vrunner.example.com/api/commands/UUID/ws?token=YOUR_TOKEN');
ws.onmessage = (e) => {
  const msg = JSON.parse(e.data);
  if (msg.type === 'vtty_diff') applyDiff(msg.data);
};
ws.send(JSON.stringify({ type: 'keys', keys: 'ls\r' }));
```

## Named Certificates for Per-Command Access

Use named certificates to give different clients access to different commands:

```bash
# Generate certificates for different teams
vrunner cert generate frontend-team
vrunner cert generate backend-team
vrunner cert generate ops-team

# Show the token for each team
vrunner cert show frontend-team
vrunner cert show backend-team
vrunner cert show ops-team
```

Each team uses their own token to spawn and interact with their commands:

```bash
# Frontend team spawns their dev server
FRONTEND_TOKEN=$(vrunner cert show frontend-team | grep -oP 'Token:\s*\K\S+')
curl -X POST https://vrunner.example.com/api/commands \
  -H "Authorization: Bearer $FRONTEND_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"cmd": "npm", "args": ["run", "dev"], "certificate": "frontend-team"}'
```

## Reverse Proxy Setup

For production deployments behind nginx or Caddy:

### nginx

```nginx
server {
    listen 443 ssl;
    server_name vrunner.example.com;

    ssl_certificate /etc/ssl/certs/vrunner.crt;
    ssl_certificate_key /etc/ssl/private/vrunner.key;

    location / {
        proxy_pass http://127.0.0.1:9090;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_read_timeout 86400;
    }
}
```

### Caddy

```
vrunner.example.com {
    reverse_proxy localhost:9090
}
```

With Caddy, TLS is handled automatically by Caddy itself. Run vrunner without `--tls`:

```bash
vrunner --bind 127.0.0.1 --port 9090 --auth -- my-command
```
