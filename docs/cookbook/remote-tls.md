# Remote Access via TLS

This recipe covers setting up vrw for secure remote access over TLS, including certificate configuration and client connectivity.

## Quick Setup (Self-Signed)

The fastest way to enable secure remote access:

```bash
vrw --remote --tls -- my-command
```

This single command:
- Binds to `0.0.0.0` (all network interfaces)
- Auto-generates a self-signed TLS certificate (saved to `~/.config/vrw/`)
- Auto-generates a 256-bit bearer token (saved to `~/.config/vrw/token`)
- Enables HTTPS and WSS (secure WebSocket)

## Step-by-Step Production Setup

### 1. Generate custom certificates (optional)

For production, use CA-signed certificates instead of self-signed:

```bash
# Using Let's Encrypt (requires a domain)
certbot certonly --standalone -d vrw.example.com
# Certificates saved to /etc/letsencrypt/live/vrw.example.com/
```

Or generate your own CA-signed certificate:

```bash
# Generate a private key and CSR
openssl genrsa -out /etc/ssl/private/vrw.key 4096
openssl req -new -key /etc/ssl/private/vrw.key -out /etc/ssl/csr/vrw.csr

# Sign with your CA
openssl x509 -req -in /etc/ssl/csr/vrw.csr \
  -CA /path/to/ca.crt -CAkey /path/to/ca.key \
  -CAcreateserial -out /etc/ssl/certs/vrw.crt -days 365
```

### 2. Start vrw with TLS and custom certificates

```bash
vrw --bind 0.0.0.0 --port 443 \
  --tls \
  --cert-file /etc/ssl/certs/vrw.crt \
  --key-file /etc/ssl/private/vrw.key \
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
  cert_file: "/etc/ssl/certs/vrw.crt"
  key_file: "/etc/ssl/private/vrw.key"

security:
  require_auth: true

daemon:
  enabled: true
  stdout_file: "/var/log/vrw/stdout"
  stderr_file: "/var/log/vrw/stderr"
```

```bash
vrw -c /etc/vrw/production.yaml
```

### 3. Configure the firewall

Only expose the vrw port to trusted networks:

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
curl https://vrw.example.com/api/commands \
  -H "Authorization: Bearer $TOKEN"

# With self-signed certificates
curl --cacert ~/.config/vrw/cert.pem https://server:443/api/commands \
  -H "Authorization: Bearer $TOKEN"

# Skip verification (not recommended for production)
curl -k https://server:443/api/commands \
  -H "Authorization: Bearer $TOKEN"
```

#### Web Browser

Navigate to `https://vrw.example.com/admin`. For self-signed certificates, the browser will show a security warning — accept it and proceed. The admin UI will prompt for the bearer token.

#### WebSocket (JavaScript)

```javascript
const ws = new WebSocket('wss://vrw.example.com/api/commands/UUID/ws?token=YOUR_TOKEN');
ws.onmessage = (e) => {
  const msg = JSON.parse(e.data);
  if (msg.type === 'vtty_dirty') {
    // Buffer changed — fetch fresh HTML via HTTP
    fetch('/api/commands/' + msg.data.id + '/vtty/html')
      .then(r => r.json())
      .then(data => renderHtml(data.data.html));
  }
};
ws.send(JSON.stringify({ type: 'keys', keys: 'ls\r' }));
```

## Named Certificates for Per-Command Access

Use named certificates to give different clients access to different commands:

```bash
# Generate certificates for different teams
vrw cert generate frontend-team
vrw cert generate backend-team
vrw cert generate ops-team

# Show the token for each team
vrw cert show frontend-team
vrw cert show backend-team
vrw cert show ops-team
```

Each team uses their own token to spawn and interact with their commands:

```bash
# Frontend team spawns their dev server
FRONTEND_TOKEN=$(vrw cert show frontend-team | grep -oP 'Token:\s*\K\S+')
curl -X POST https://vrw.example.com/api/commands \
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
    server_name vrw.example.com;

    ssl_certificate /etc/ssl/certs/vrw.crt;
    ssl_certificate_key /etc/ssl/private/vrw.key;

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
vrw.example.com {
    reverse_proxy localhost:9090
}
```

With Caddy, TLS is handled automatically by Caddy itself. Run vrw without `--tls`:

```bash
vrw --bind 127.0.0.1 --port 9090 --auth -- my-command
```
