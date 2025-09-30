# Security Best Practices and TLS Setup

This guide covers security hardening, TLS/SSL configuration, and best practices for securing RustyMail deployments.

## Security Overview

RustyMail implements multiple layers of security:

- **Transport Security**: TLS/SSL for all connections
- **Authentication**: JWT tokens, API keys, OAuth2
- **Authorization**: Role-based access control
- **Data Protection**: Encryption at rest and in transit
- **Network Security**: Firewalls, network policies
- **Application Security**: Input validation, rate limiting

## TLS/SSL Configuration

### Generating Certificates

#### Self-Signed Certificates (Development)

```bash
# Generate private key
openssl genrsa -out rustymail.key 4096

# Generate certificate signing request
openssl req -new -key rustymail.key -out rustymail.csr \
  -subj "/C=US/ST=State/L=City/O=Organization/CN=rustymail.local"

# Generate self-signed certificate
openssl x509 -req -days 365 -in rustymail.csr \
  -signkey rustymail.key -out rustymail.crt

# Verify certificate
openssl x509 -in rustymail.crt -text -noout
```

#### Let's Encrypt (Production)

```bash
# Install certbot
sudo apt-get update
sudo apt-get install certbot

# Generate certificate
sudo certbot certonly --standalone \
  -d rustymail.example.com \
  -d api.rustymail.example.com \
  --agree-tos \
  --email admin@example.com

# Certificates will be in:
# /etc/letsencrypt/live/rustymail.example.com/fullchain.pem
# /etc/letsencrypt/live/rustymail.example.com/privkey.pem

# Auto-renewal
sudo certbot renew --dry-run
```

#### Certificate Configuration

```bash
# Environment variables
REQUIRE_HTTPS=true
TLS_CERT_PATH=/etc/ssl/certs/rustymail.crt
TLS_KEY_PATH=/etc/ssl/private/rustymail.key
TLS_CA_PATH=/etc/ssl/certs/ca-certificates.crt

# Strong TLS configuration
TLS_MIN_VERSION=TLS1.2
TLS_CIPHERS=TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
```

### Nginx TLS Termination

```nginx
# nginx/nginx.conf
server {
    listen 80;
    server_name rustymail.example.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name rustymail.example.com;

    # SSL Configuration
    ssl_certificate /etc/nginx/ssl/rustymail.crt;
    ssl_certificate_key /etc/nginx/ssl/rustymail.key;

    # Strong SSL Settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;

    # SSL Session
    ssl_session_timeout 1d;
    ssl_session_cache shared:SSL:10m;
    ssl_session_tickets off;

    # OCSP Stapling
    ssl_stapling on;
    ssl_stapling_verify on;
    ssl_trusted_certificate /etc/nginx/ssl/ca.crt;

    # Security Headers
    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Content-Security-Policy "default-src 'self' https:; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline';" always;

    # Proxy to RustyMail
    location / {
        proxy_pass http://rustymail:9439;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket support for SSE
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }

    location /api {
        proxy_pass http://rustymail:9437;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Rate limiting
        limit_req zone=api burst=10 nodelay;
        limit_req_status 429;
    }
}
```

## Authentication and Authorization

### JWT Configuration

```bash
# Generate secure JWT secret
JWT_SECRET=$(openssl rand -hex 32)

# JWT settings
JWT_EXPIRY=3600              # 1 hour
JWT_REFRESH_EXPIRY=604800    # 7 days
JWT_ALGORITHM=HS256
JWT_ISSUER=rustymail
JWT_AUDIENCE=rustymail-api
```

### API Key Authentication

```bash
# Generate API key
API_KEY=$(openssl rand -hex 32)

# Configure API key
API_KEY_HEADER=X-API-Key
API_KEY_REQUIRED=true
API_KEY_WHITELIST=/health,/metrics
```

### OAuth2 Integration

```yaml
# OAuth2 configuration
oauth2:
  enabled: true
  providers:
    - name: google
      client_id: ${GOOGLE_CLIENT_ID}
      client_secret: ${GOOGLE_CLIENT_SECRET}
      redirect_uri: https://rustymail.example.com/auth/google/callback
      scopes:
        - openid
        - email
        - profile

    - name: github
      client_id: ${GITHUB_CLIENT_ID}
      client_secret: ${GITHUB_CLIENT_SECRET}
      redirect_uri: https://rustymail.example.com/auth/github/callback
      scopes:
        - user:email
```

## Network Security

### Firewall Rules

```bash
# iptables rules
#!/bin/bash

# Default policies
iptables -P INPUT DROP
iptables -P FORWARD DROP
iptables -P OUTPUT ACCEPT

# Allow loopback
iptables -A INPUT -i lo -j ACCEPT
iptables -A OUTPUT -o lo -j ACCEPT

# Allow established connections
iptables -A INPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT

# Allow SSH (restricted)
iptables -A INPUT -p tcp --dport 22 -s 10.0.0.0/8 -j ACCEPT

# Allow HTTPS
iptables -A INPUT -p tcp --dport 443 -j ACCEPT

# Allow HTTP (redirect to HTTPS)
iptables -A INPUT -p tcp --dport 80 -j ACCEPT

# RustyMail ports (internal only)
iptables -A INPUT -p tcp --dport 9437 -s 172.25.0.0/16 -j ACCEPT
iptables -A INPUT -p tcp --dport 9438 -s 172.25.0.0/16 -j ACCEPT
iptables -A INPUT -p tcp --dport 9439 -s 172.25.0.0/16 -j ACCEPT

# Rate limiting
iptables -A INPUT -p tcp --dport 443 -m conntrack --ctstate NEW -m limit --limit 10/sec --limit-burst 20 -j ACCEPT
iptables -A INPUT -p tcp --dport 443 -m conntrack --ctstate NEW -j DROP

# Save rules
iptables-save > /etc/iptables/rules.v4
```

### Kubernetes Network Policies

```yaml
# network-policy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: rustymail-network-policy
  namespace: rustymail
spec:
  podSelector:
    matchLabels:
      app: rustymail
  policyTypes:
  - Ingress
  - Egress

  ingress:
  # Allow from ingress controller
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
    ports:
    - protocol: TCP
      port: 9437
    - protocol: TCP
      port: 9438
    - protocol: TCP
      port: 9439

  # Allow from monitoring
  - from:
    - namespaceSelector:
        matchLabels:
          name: monitoring
    ports:
    - protocol: TCP
      port: 9437

  egress:
  # Allow DNS
  - to:
    - namespaceSelector: {}
      podSelector:
        matchLabels:
          k8s-app: kube-dns
    ports:
    - protocol: UDP
      port: 53

  # Allow IMAPS
  - to:
    - ipBlock:
        cidr: 0.0.0.0/0
        except:
        - 10.0.0.0/8
        - 192.168.0.0/16
        - 172.16.0.0/12
    ports:
    - protocol: TCP
      port: 993

  # Allow internal Redis
  - to:
    - podSelector:
        matchLabels:
          app: redis
    ports:
    - protocol: TCP
      port: 6379
```

## Secrets Management

### Kubernetes Secrets

```bash
# Create secrets
kubectl create secret generic rustymail-secrets \
  --from-literal=imap-password='${IMAP_PASSWORD}' \
  --from-literal=jwt-secret='${JWT_SECRET}' \
  --from-literal=api-key='${API_KEY}' \
  --namespace=rustymail

# Seal secrets with Sealed Secrets
kubeseal --format=yaml < secret.yaml > sealed-secret.yaml
```

### HashiCorp Vault Integration

```bash
# Store secrets in Vault
vault kv put secret/rustymail \
  imap_password="${IMAP_PASSWORD}" \
  jwt_secret="${JWT_SECRET}" \
  api_key="${API_KEY}"

# Vault agent configuration
cat > vault-agent.hcl <<EOF
auto_auth {
  method {
    type = "kubernetes"
    config {
      role = "rustymail"
    }
  }
}

template {
  source      = "/vault/templates/env.tpl"
  destination = "/vault/secrets/env"
}
EOF
```

### Docker Secrets

```bash
# Create Docker secrets
echo "${IMAP_PASSWORD}" | docker secret create imap_password -
echo "${JWT_SECRET}" | docker secret create jwt_secret -
echo "${API_KEY}" | docker secret create api_key -

# Use in docker-compose
services:
  rustymail:
    secrets:
      - imap_password
      - jwt_secret
      - api_key
    environment:
      IMAP_PASSWORD_FILE: /run/secrets/imap_password
      JWT_SECRET_FILE: /run/secrets/jwt_secret
      API_KEY_FILE: /run/secrets/api_key

secrets:
  imap_password:
    external: true
  jwt_secret:
    external: true
  api_key:
    external: true
```

## Container Security

### Docker Security

```dockerfile
# Secure Dockerfile
FROM rust:1.75 AS builder
# Build stage...

FROM gcr.io/distroless/cc-debian12
# Minimal attack surface

# Non-root user
USER 1001:1001

# Read-only root filesystem
# Configured at runtime

# No shell or package manager
# Security scanning passed
```

### Security Scanning

```bash
# Scan Docker image
trivy image rustymail:latest

# Scan with Snyk
snyk container test rustymail:latest

# Scan with Clair
clair-scanner --ip 172.17.0.1 rustymail:latest
```

### Pod Security Standards

```yaml
# pod-security.yaml
apiVersion: v1
kind: Pod
metadata:
  name: rustymail
spec:
  securityContext:
    runAsNonRoot: true
    runAsUser: 1001
    fsGroup: 1001
    seccompProfile:
      type: RuntimeDefault

  containers:
  - name: rustymail
    image: rustymail:latest
    securityContext:
      allowPrivilegeEscalation: false
      readOnlyRootFilesystem: true
      capabilities:
        drop:
          - ALL
        add:
          - NET_BIND_SERVICE
    resources:
      limits:
        cpu: 1000m
        memory: 1Gi
      requests:
        cpu: 100m
        memory: 128Mi
```

## Application Security

### Input Validation

```yaml
# Validation rules
validation:
  email:
    pattern: '^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$'
    max_length: 255

  api_request:
    max_body_size: 10485760  # 10MB
    max_header_size: 8192
    allowed_content_types:
      - application/json
      - application/x-www-form-urlencoded

  rate_limits:
    per_ip: 100/minute
    per_user: 1000/hour
    per_api_key: 10000/hour
```

### SQL Injection Prevention

```rust
// Use parameterized queries
let query = "SELECT * FROM emails WHERE user_id = $1 AND folder = $2";
let rows = client.query(query, &[&user_id, &folder]).await?;

// Never use string concatenation
// BAD: format!("SELECT * FROM emails WHERE user_id = {}", user_id)
```

### XSS Prevention

```html
<!-- Content Security Policy -->
<meta http-equiv="Content-Security-Policy"
      content="default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline';">

<!-- X-XSS-Protection -->
<meta http-equiv="X-XSS-Protection" content="1; mode=block">
```

## Security Monitoring

### Audit Logging

```yaml
# Audit log configuration
audit:
  enabled: true
  log_file: /var/log/rustymail/audit.log
  events:
    - authentication
    - authorization
    - data_access
    - configuration_change
    - security_event

  format:
    timestamp: ISO8601
    user_id: true
    ip_address: true
    user_agent: true
    action: true
    resource: true
    result: true
```

### Intrusion Detection

```bash
# Install AIDE
apt-get install aide

# Configure AIDE
cat > /etc/aide/aide.conf <<EOF
database=file:/var/lib/aide/aide.db
database_out=file:/var/lib/aide/aide.db.new

/opt/rustymail p+u+g+s+m+c+md5+sha256
/etc/rustymail p+u+g+s+m+c+md5+sha256
EOF

# Initialize database
aide --init
mv /var/lib/aide/aide.db.new /var/lib/aide/aide.db

# Check for changes
aide --check
```

### Security Alerts

```yaml
# AlertManager configuration
global:
  resolve_timeout: 5m

route:
  group_by: ['alertname', 'severity']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 1h
  receiver: 'security-team'

  routes:
  - match:
      severity: critical
      category: security
    receiver: 'security-critical'
    continue: true

receivers:
- name: 'security-team'
  email_configs:
  - to: 'security@example.com'
    from: 'alerts@example.com'
    headers:
      Subject: 'Security Alert: {{ .GroupLabels.alertname }}'

- name: 'security-critical'
  pagerduty_configs:
  - service_key: '${PAGERDUTY_SERVICE_KEY}'
    severity: 'critical'
```

## Compliance and Hardening

### CIS Benchmarks

```bash
# Docker CIS Benchmark
docker run --rm --net host --pid host --cap-add audit_control \
  -v /var/lib:/var/lib \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v /etc:/etc --label docker_bench_security \
  docker/docker-bench-security

# Kubernetes CIS Benchmark
kube-bench run --targets=node,policies
```

### OWASP Top 10 Mitigation

1. **Injection**: Parameterized queries, input validation
2. **Broken Authentication**: Strong passwords, MFA, session management
3. **Sensitive Data Exposure**: Encryption, TLS, secure storage
4. **XML External Entities**: Disable XXE, use safe parsers
5. **Broken Access Control**: RBAC, least privilege
6. **Security Misconfiguration**: Hardened configs, security headers
7. **Cross-Site Scripting**: CSP, output encoding
8. **Insecure Deserialization**: Validate input, use safe libraries
9. **Using Components with Known Vulnerabilities**: Regular updates, scanning
10. **Insufficient Logging**: Comprehensive audit logs, monitoring

### Security Checklist

#### Pre-Deployment

- [ ] TLS certificates installed and configured
- [ ] Secrets stored securely (not in code)
- [ ] Security headers configured
- [ ] Rate limiting enabled
- [ ] Input validation implemented
- [ ] Authentication required for all endpoints
- [ ] Authorization checks in place
- [ ] Security scanning passed
- [ ] Dependency vulnerabilities checked
- [ ] Network policies configured

#### Post-Deployment

- [ ] Security monitoring active
- [ ] Audit logging enabled
- [ ] Intrusion detection configured
- [ ] Backup encryption verified
- [ ] Incident response plan ready
- [ ] Security alerts configured
- [ ] Regular security updates scheduled
- [ ] Penetration testing planned
- [ ] Compliance requirements met
- [ ] Security documentation updated

## Incident Response

### Response Plan

```yaml
incident_response:
  detection:
    - Monitor security alerts
    - Check audit logs
    - Analyze metrics anomalies

  containment:
    - Isolate affected systems
    - Revoke compromised credentials
    - Block malicious IPs

  eradication:
    - Remove malicious code
    - Patch vulnerabilities
    - Update configurations

  recovery:
    - Restore from clean backups
    - Rebuild affected systems
    - Reset all credentials

  lessons_learned:
    - Document incident
    - Update security measures
    - Train team members
```

### Security Contacts

```yaml
contacts:
  security_team:
    email: security@example.com
    phone: +1-555-SECURITY
    oncall: https://oncall.example.com

  external:
    cert: cert@cert.org
    vendor: support@vendor.com
```

## Security Tools

### Security Testing

```bash
# OWASP ZAP scan
docker run -t owasp/zap2docker-stable zap-baseline.py \
  -t https://rustymail.example.com

# Nikto scan
nikto -h https://rustymail.example.com

# SQLMap test
sqlmap -u "https://rustymail.example.com/api/v1/search?q=test" \
  --batch --random-agent
```

### Vulnerability Management

```bash
# Check for CVEs
grype rustymail:latest

# Dependency check
cargo audit

# SAST scanning
semgrep --config=auto .

# License compliance
license-checker --production --summary
```

## Next Steps

- Implement [security monitoring](monitoring.md#security-monitoring)
- Configure [backup encryption](backup-recovery.md#encryption)
- Set up [compliance reporting](compliance.md)
- Review [disaster recovery](disaster-recovery.md)