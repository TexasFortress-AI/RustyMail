# Configuration Guide

RustyMail uses environment variables for all configuration. This guide documents every available configuration option, their defaults, and usage examples.

## Configuration Methods

### Environment Variables

RustyMail can be configured through:

1. **Direct environment variables**: `export VAR=value`
2. **Environment file**: Load from `.env` file
3. **Docker/Kubernetes**: Pass via container configuration
4. **systemd**: Use `EnvironmentFile` directive

### Configuration Precedence

1. Command-line arguments (highest priority)
2. Environment variables
3. Configuration file
4. Default values (lowest priority)

## Core Configuration

### IMAP Connection Settings

| Variable | Description | Default | Required | Example |
|----------|-------------|---------|----------|---------|
| `IMAP_ADAPTER` | IMAP provider adapter | `mock` | No | `gmail`, `outlook`, `godaddy`, `custom` |
| `IMAP_HOST` | IMAP server hostname | - | Yes* | `imap.gmail.com` |
| `IMAP_PORT` | IMAP server port | `993` | No | `993`, `143` |
| `IMAP_USERNAME` | Email account username | - | Yes* | `user@example.com` |
| `IMAP_PASSWORD` | Email account password | - | Yes* | `app-specific-password` |
| `IMAP_USE_TLS` | Use TLS/SSL connection | `true` | No | `true`, `false` |
| `IMAP_VALIDATE_CERTS` | Validate SSL certificates | `true` | No | `true`, `false` |
| `IMAP_TIMEOUT` | Connection timeout (seconds) | `30` | No | `60` |

*Required when not using mock adapter

#### Provider-Specific Settings

##### Gmail
```bash
IMAP_ADAPTER=gmail
IMAP_HOST=imap.gmail.com
IMAP_PORT=993
# Use App Password, not regular password
# Enable "Less secure app access" or use OAuth2
```

##### Outlook/Office365
```bash
IMAP_ADAPTER=outlook
IMAP_HOST=outlook.office365.com
IMAP_PORT=993
# May require app-specific password
```

##### GoDaddy
```bash
IMAP_ADAPTER=godaddy
IMAP_HOST=imap.secureserver.net
IMAP_PORT=993
```

### Server Configuration

| Variable | Description | Default | Required | Example |
|----------|-------------|---------|----------|---------|
| `REST_HOST` | REST API bind address | `127.0.0.1` | No | `0.0.0.0`, `::1` |
| `REST_PORT` | REST API port | `9437` | No | `8080` |
| `SSE_HOST` | SSE server bind address | `127.0.0.1` | No | `0.0.0.0` |
| `SSE_PORT` | SSE server port | `9438` | No | `8081` |
| `DASHBOARD_ENABLED` | Enable web dashboard | `true` | No | `false` |
| `DASHBOARD_HOST` | Dashboard bind address | `127.0.0.1` | No | `0.0.0.0` |
| `DASHBOARD_PORT` | Dashboard port | `9439` | No | `3000` |
| `DASHBOARD_PATH` | Dashboard static files path | `./frontend/dist` | No | `/app/dashboard` |

### Logging Configuration

| Variable | Description | Default | Required | Example |
|----------|-------------|---------|----------|---------|
| `LOG_LEVEL` | Application log level | `info` | No | `debug`, `info`, `warn`, `error` |
| `RUST_LOG` | Rust logging configuration | `rustymail=info` | No | `debug`, `rustymail=debug,tower=warn` |
| `LOG_FORMAT` | Log output format | `json` | No | `json`, `pretty`, `compact` |
| `LOG_FILE` | Log file path | - | No | `/var/log/rustymail.log` |
| `LOG_MAX_SIZE` | Max log file size (MB) | `100` | No | `500` |
| `LOG_MAX_BACKUPS` | Max number of log backups | `5` | No | `10` |
| `LOG_TIMESTAMPS` | Include timestamps | `true` | No | `false` |
| `LOG_COLORS` | Enable colored output | `auto` | No | `always`, `never`, `auto` |

#### Log Levels

- `trace`: Very verbose debugging
- `debug`: Debugging information
- `info`: Informational messages
- `warn`: Warning messages
- `error`: Error messages only

### Performance Configuration

| Variable | Description | Default | Required | Example |
|----------|-------------|---------|----------|---------|
| `MAX_CONNECTIONS` | Max IMAP connections | `10` | No | `50` |
| `CONNECTION_TIMEOUT` | Connection timeout (seconds) | `30` | No | `60` |
| `IDLE_TIMEOUT` | Idle connection timeout | `300` | No | `600` |
| `MAX_RETRIES` | Max retry attempts | `3` | No | `5` |
| `RETRY_DELAY` | Retry delay (seconds) | `5` | No | `10` |
| `WORKER_THREADS` | Number of worker threads | CPU cores | No | `4` |
| `BLOCKING_THREADS` | Blocking task threads | `512` | No | `1024` |
| `KEEP_ALIVE` | TCP keep-alive (seconds) | `60` | No | `120` |

### Security Configuration

| Variable | Description | Default | Required | Example |
|----------|-------------|---------|----------|---------|
| `REQUIRE_HTTPS` | Require HTTPS connections | `false` | No | `true` |
| `TLS_CERT_PATH` | TLS certificate path | - | No* | `/etc/ssl/cert.pem` |
| `TLS_KEY_PATH` | TLS private key path | - | No* | `/etc/ssl/key.pem` |
| `TLS_CA_PATH` | TLS CA certificate path | - | No | `/etc/ssl/ca.pem` |
| `CORS_ENABLED` | Enable CORS | `true` | No | `false` |
| `CORS_ORIGINS` | Allowed CORS origins | `*` | No | `https://example.com,https://app.example.com` |
| `CORS_METHODS` | Allowed CORS methods | `GET,POST,PUT,DELETE,OPTIONS` | No | `GET,POST` |
| `CORS_HEADERS` | Allowed CORS headers | `*` | No | `Content-Type,Authorization` |
| `API_KEY` | API key for authentication | - | No | `secret-api-key-here` |
| `JWT_SECRET` | JWT signing secret | Random | No | `your-256-bit-secret` |
| `JWT_EXPIRY` | JWT expiry time (seconds) | `3600` | No | `7200` |

*Required when REQUIRE_HTTPS is true

### Rate Limiting Configuration

| Variable | Description | Default | Required | Example |
|----------|-------------|---------|----------|---------|
| `RATE_LIMIT_ENABLED` | Enable rate limiting | `true` | No | `false` |
| `RATE_LIMIT_REQUESTS` | Max requests per period | `100` | No | `1000` |
| `RATE_LIMIT_PERIOD` | Period in seconds | `60` | No | `3600` |
| `RATE_LIMIT_BURST` | Burst allowance | `10` | No | `50` |
| `RATE_LIMIT_BY_IP` | Rate limit by IP | `true` | No | `false` |
| `RATE_LIMIT_BY_USER` | Rate limit by user | `false` | No | `true` |
| `RATE_LIMIT_WHITELIST` | Whitelisted IPs | - | No | `127.0.0.1,192.168.1.0/24` |

### Cache Configuration

| Variable | Description | Default | Required | Example |
|----------|-------------|---------|----------|---------|
| `CACHE_ENABLED` | Enable caching | `true` | No | `false` |
| `CACHE_TYPE` | Cache backend type | `memory` | No | `redis`, `memcached` |
| `CACHE_SIZE` | Max cache size (MB) | `100` | No | `500` |
| `CACHE_TTL` | Default TTL (seconds) | `300` | No | `3600` |
| `CACHE_REDIS_URL` | Redis connection URL | - | No* | `redis://localhost:6379/0` |
| `CACHE_REDIS_PASSWORD` | Redis password | - | No | `redis-password` |

*Required when CACHE_TYPE is redis

### Database Configuration (Optional)

| Variable | Description | Default | Required | Example |
|----------|-------------|---------|----------|---------|
| `DATABASE_URL` | Database connection URL | - | No | `postgresql://user:pass@localhost/db` |
| `DATABASE_POOL_SIZE` | Connection pool size | `10` | No | `20` |
| `DATABASE_MAX_LIFETIME` | Max connection lifetime | `30m` | No | `1h` |
| `DATABASE_CONNECT_TIMEOUT` | Connect timeout | `5s` | No | `10s` |

### AI Service Configuration

| Variable | Description | Default | Required | Example |
|----------|-------------|---------|----------|---------|
| `OPENAI_API_KEY` | OpenAI API key | - | No | `sk-...` |
| `OPENAI_MODEL` | OpenAI model to use | `gpt-3.5-turbo` | No | `gpt-4` |
| `OPENAI_TEMPERATURE` | Response temperature | `0.7` | No | `0.9` |
| `OPENAI_MAX_TOKENS` | Max response tokens | `150` | No | `500` |
| `OPENROUTER_API_KEY` | OpenRouter API key | - | No | `sk-or-...` |
| `OPENROUTER_MODEL` | OpenRouter model | `auto` | No | `anthropic/claude-2` |

### Monitoring Configuration

| Variable | Description | Default | Required | Example |
|----------|-------------|---------|----------|---------|
| `METRICS_ENABLED` | Enable metrics endpoint | `true` | No | `false` |
| `METRICS_PATH` | Metrics endpoint path | `/metrics` | No | `/admin/metrics` |
| `METRICS_PORT` | Metrics port (if separate) | Same as REST | No | `9090` |
| `TRACES_ENABLED` | Enable distributed tracing | `false` | No | `true` |
| `TRACES_ENDPOINT` | Tracing collector endpoint | - | No | `http://jaeger:14268/api/traces` |
| `TRACES_SAMPLE_RATE` | Trace sampling rate (0-1) | `0.1` | No | `1.0` |

## Configuration Examples

### Development Configuration

```bash
# .env.development
IMAP_ADAPTER=mock
LOG_LEVEL=debug
RUST_LOG=rustymail=debug,tower=trace
REST_HOST=0.0.0.0
REST_PORT=9437
DASHBOARD_ENABLED=true
RATE_LIMIT_ENABLED=false
CACHE_ENABLED=false
```

### Production Configuration

```bash
# .env.production
IMAP_ADAPTER=gmail
IMAP_HOST=imap.gmail.com
IMAP_PORT=993
IMAP_USERNAME=production@example.com
IMAP_PASSWORD=${IMAP_PASSWORD_SECRET}

REST_HOST=0.0.0.0
REST_PORT=9437
SSE_HOST=0.0.0.0
SSE_PORT=9438

DASHBOARD_ENABLED=true
DASHBOARD_PORT=9439

LOG_LEVEL=info
LOG_FORMAT=json
LOG_FILE=/var/log/rustymail/app.log

REQUIRE_HTTPS=true
TLS_CERT_PATH=/etc/ssl/certs/rustymail.crt
TLS_KEY_PATH=/etc/ssl/private/rustymail.key

RATE_LIMIT_ENABLED=true
RATE_LIMIT_REQUESTS=1000
RATE_LIMIT_PERIOD=60

CACHE_ENABLED=true
CACHE_TYPE=redis
CACHE_REDIS_URL=redis://redis:6379/0

MAX_CONNECTIONS=50
CONNECTION_TIMEOUT=60
WORKER_THREADS=8

METRICS_ENABLED=true
TRACES_ENABLED=true
TRACES_ENDPOINT=http://jaeger:14268/api/traces
```

### Docker Compose Override

```yaml
version: '3.8'

services:
  rustymail:
    env_file:
      - .env.production
    environment:
      - IMAP_PASSWORD=${IMAP_PASSWORD_SECRET}
      - JWT_SECRET=${JWT_SECRET}
      - DATABASE_URL=postgresql://${DB_USER}:${DB_PASS}@postgres/rustymail
```

### Kubernetes ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: rustymail-config
data:
  REST_HOST: "0.0.0.0"
  REST_PORT: "9437"
  LOG_LEVEL: "info"
  LOG_FORMAT: "json"
  MAX_CONNECTIONS: "50"
  RATE_LIMIT_REQUESTS: "1000"
  CACHE_TTL: "300"
```

## Validation and Testing

### Configuration Validation

```bash
# Test configuration without starting server
./rustymail-server --validate-config

# Test IMAP connection
./rustymail-server --test-connection

# Dry run with configuration
./rustymail-server --dry-run
```

### Environment Variable Debugging

```bash
# Print resolved configuration
./rustymail-server --print-config

# Debug environment loading
RUST_LOG=rustymail::config=debug ./rustymail-server

# Validate specific settings
./rustymail-server --check-imap
./rustymail-server --check-tls
```

## Configuration Best Practices

### Security

1. **Never commit credentials** to version control
2. **Use secrets management** (Kubernetes Secrets, Docker Secrets, Vault)
3. **Rotate credentials regularly**
4. **Use strong passwords and API keys**
5. **Enable TLS in production**
6. **Restrict CORS origins** in production

### Performance

1. **Tune connection pools** based on load
2. **Set appropriate timeouts** to prevent hanging
3. **Enable caching** for better performance
4. **Use connection pooling** for databases
5. **Monitor resource usage** and adjust limits

### Operations

1. **Use structured logging** (JSON format) in production
2. **Enable metrics** for monitoring
3. **Set up proper health checks**
4. **Use configuration management** tools
5. **Document all custom configurations**
6. **Test configuration changes** in staging first

## Troubleshooting Configuration Issues

### Common Problems

#### Missing Required Variables

```bash
Error: IMAP_HOST is required when IMAP_ADAPTER is not 'mock'
Solution: Set IMAP_HOST=imap.gmail.com
```

#### Invalid Port Numbers

```bash
Error: REST_PORT must be between 1 and 65535
Solution: Use a valid port number like 9437
```

#### Permission Denied on Ports

```bash
Error: Permission denied binding to port 80
Solution: Use a port above 1024 or run with appropriate permissions
```

#### TLS Certificate Issues

```bash
Error: Failed to load TLS certificate
Solution: Check file paths and permissions for TLS_CERT_PATH and TLS_KEY_PATH
```

### Configuration Debugging

```bash
# Enable debug logging for configuration
RUST_LOG=rustymail::config=trace ./rustymail-server

# Check environment variables
env | grep IMAP
env | grep REST

# Test with minimal configuration
IMAP_ADAPTER=mock ./rustymail-server

# Validate JSON in LOG_FORMAT
echo '{"test": "value"}' | jq .
```

## Migration Guide

### From Version 0.x to 1.x

```bash
# Old configuration
EMAIL_HOST=imap.gmail.com
EMAIL_USER=user@gmail.com

# New configuration
IMAP_HOST=imap.gmail.com
IMAP_USERNAME=user@gmail.com
```

### From Environment File to Kubernetes

1. Extract variables from `.env`
2. Create ConfigMap for non-sensitive values
3. Create Secret for sensitive values
4. Reference in Deployment spec

## Next Steps

- Review [security best practices](security.md)
- Set up [monitoring](monitoring.md)
- Configure [TLS/SSL](security.md#tls-configuration)
- Implement [backup strategies](backup-recovery.md)