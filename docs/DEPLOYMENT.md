# IMAP API Deployment Guide

This guide provides detailed instructions for deploying the IMAP API service in various environments, from development to production.

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Building the Application](#building-the-application)
3. [Configuration](#configuration)
4. [Deployment Options](#deployment-options)
   - [Standalone Deployment](#standalone-deployment)
   - [Docker Deployment](#docker-deployment)
   - [Docker Compose](#docker-compose)
   - [Kubernetes Deployment](#kubernetes-deployment)
5. [Monitoring and Logging](#monitoring-and-logging)
6. [Backup and Recovery](#backup-and-recovery)
7. [Scaling Considerations](#scaling-considerations)
8. [Security Recommendations](#security-recommendations)
9. [Troubleshooting](#troubleshooting)

## System Requirements

### Hardware Requirements

Minimum specifications for various deployment scales:

| Scale | CPU | RAM | Disk | Network |
|-------|-----|-----|------|---------|
| Development | 2 cores | 2 GB | 10 GB | 100 Mbps |
| Small Production | 4 cores | 8 GB | 20 GB | 1 Gbps |
| Medium Production | 8 cores | 16 GB | 40 GB | 1+ Gbps |
| Large Production | 16+ cores | 32+ GB | 80+ GB | 10+ Gbps |

### Software Requirements

- **Operating System**: Linux (Ubuntu 20.04+, CentOS 8+, RHEL 8+), macOS 11+, or Windows 10/11/Server 2019+
- **Rust**: 1.65.0 or later
- **Cargo**: Latest version
- **OpenSSL**: 1.1.1 or later
- **Docker**: 20.10.0+ (for containerized deployment)
- **Kubernetes**: 1.21+ (for orchestrated deployment)

## Building the Application

### From Source

1. Clone the repository:
   ```bash
   git clone https://github.com/your-organization/imap-api-rust.git
   cd imap-api-rust
   ```

2. Build the application:
   ```bash
   cargo build --release
   ```

3. The compiled binary will be located at `target/release/imap-api-rust`

### Building Docker Image

1. From the project root, build the Docker image:
   ```bash
   docker build -t imap-api-rust:latest .
   ```

## Configuration

The application can be configured using environment variables, a config file, or command-line arguments.

### Environment Variables

Essential environment variables:

```
IMAP_HOST=imap.example.com
IMAP_PORT=993
IMAP_USERNAME=your_username
IMAP_PASSWORD=your_secure_password
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
LOG_LEVEL=info
```

### Configuration File

Create a `config.toml` file in the configuration directory (default: `./config/`):

```toml
[imap]
host = "imap.example.com"
port = 993
username = "your_username"
password = "your_secure_password"

[server]
host = "0.0.0.0"
port = 8080

log_level = "info"
```

### Command-Line Arguments

The application supports the following command-line arguments:

```
USAGE:
    imap-api-rust [OPTIONS]

OPTIONS:
    -c, --config <FILE>              Sets a custom config file
    -h, --host <HOST>                Sets the listening host [default: 0.0.0.0]
    -p, --port <PORT>                Sets the listening port [default: 8080]
        --imap-host <IMAP_HOST>      Sets the IMAP host
        --imap-port <IMAP_PORT>      Sets the IMAP port [default: 993]
        --imap-user <IMAP_USER>      Sets the IMAP username
        --imap-pass <IMAP_PASS>      Sets the IMAP password
    -l, --log-level <LEVEL>          Sets the log level [default: info]
    -h, --help                       Print help information
    -V, --version                    Print version information
```

## Deployment Options

### Standalone Deployment

For simple deployments directly on a server:

1. Copy the built binary and configuration:
   ```bash
   scp target/release/imap-api-rust user@server:/opt/imap-api/
   scp config.toml user@server:/opt/imap-api/
   ```

2. Create a systemd service file `/etc/systemd/system/imap-api.service`:
   ```
   [Unit]
   Description=IMAP API Service
   After=network.target

   [Service]
   Type=simple
   User=imap-service
   WorkingDirectory=/opt/imap-api
   ExecStart=/opt/imap-api/imap-api-rust
   Restart=on-failure
   RestartSec=5
   Environment=RUST_LOG=info

   [Install]
   WantedBy=multi-user.target
   ```

3. Enable and start the service:
   ```bash
   sudo systemctl enable imap-api
   sudo systemctl start imap-api
   ```

4. Check the service status:
   ```bash
   sudo systemctl status imap-api
   ```

### Docker Deployment

1. Run the Docker container:
   ```bash
   docker run -d \
     --name imap-api \
     -p 8080:8080 \
     -e IMAP_HOST=imap.example.com \
     -e IMAP_PORT=993 \
     -e IMAP_USERNAME=your_username \
     -e IMAP_PASSWORD=your_password \
     -e SERVER_HOST=0.0.0.0 \
     -e SERVER_PORT=8080 \
     -e LOG_LEVEL=info \
     imap-api-rust:latest
   ```

2. For volume-mounted configuration:
   ```bash
   docker run -d \
     --name imap-api \
     -p 8080:8080 \
     -v /path/to/config.toml:/app/config/config.toml \
     imap-api-rust:latest
   ```

### Docker Compose

Create a `docker-compose.yml` file:

```yaml
version: '3.8'

services:
  imap-api:
    image: imap-api-rust:latest
    ports:
      - "8080:8080"
    environment:
      IMAP_HOST: imap.example.com
      IMAP_PORT: 993
      IMAP_USERNAME: your_username
      IMAP_PASSWORD: your_password
      SERVER_HOST: 0.0.0.0
      SERVER_PORT: 8080
      LOG_LEVEL: info
    restart: unless-stopped
    volumes:
      - ./logs:/app/logs
    networks:
      - api-network

networks:
  api-network:
    driver: bridge
```

Run with Docker Compose:
```bash
docker-compose up -d
```

### Kubernetes Deployment

Create a Kubernetes deployment manifest `imap-api-deployment.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: imap-api
  labels:
    app: imap-api
spec:
  replicas: 3
  selector:
    matchLabels:
      app: imap-api
  template:
    metadata:
      labels:
        app: imap-api
    spec:
      containers:
      - name: imap-api
        image: your-registry/imap-api-rust:latest
        ports:
        - containerPort: 8080
        env:
        - name: IMAP_HOST
          valueFrom:
            secretKeyRef:
              name: imap-credentials
              key: host
        - name: IMAP_PORT
          value: "993"
        - name: IMAP_USERNAME
          valueFrom:
            secretKeyRef:
              name: imap-credentials
              key: username
        - name: IMAP_PASSWORD
          valueFrom:
            secretKeyRef:
              name: imap-credentials
              key: password
        - name: SERVER_HOST
          value: "0.0.0.0"
        - name: SERVER_PORT
          value: "8080"
        - name: LOG_LEVEL
          value: "info"
        resources:
          limits:
            cpu: "1"
            memory: "1Gi"
          requests:
            cpu: "500m"
            memory: "512Mi"
        livenessProbe:
          httpGet:
            path: /
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: imap-api-service
spec:
  selector:
    app: imap-api
  ports:
  - port: 80
    targetPort: 8080
  type: LoadBalancer
```

Create a secret for IMAP credentials:
```bash
kubectl create secret generic imap-credentials \
  --from-literal=host=imap.example.com \
  --from-literal=username=your_username \
  --from-literal=password=your_password
```

Apply the deployment:
```bash
kubectl apply -f imap-api-deployment.yaml
```

## Monitoring and Logging

### Logging Configuration

The application uses structured JSON logging by default. Logs are written to stdout/stderr and can be customized via the `LOG_LEVEL` setting.

Available log levels: `trace`, `debug`, `info`, `warn`, `error`

### Integrating with Monitoring Systems

#### Prometheus Metrics

The API exposes metrics at the `/metrics` endpoint in Prometheus format, including:

- Request counts and latencies
- Error rates
- IMAP connection pool statistics

#### Grafana Dashboard

A sample Grafana dashboard is available in the `monitoring/` directory. Import it into your Grafana instance for visualization.

#### Alert Configuration

Example Prometheus alerting rules:

```yaml
groups:
- name: imap-api-alerts
  rules:
  - alert: HighErrorRate
    expr: rate(imap_api_errors_total[5m]) / rate(imap_api_requests_total[5m]) > 0.1
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High error rate detected"
      description: "IMAP API error rate is above 10% for 5 minutes"
```

## Backup and Recovery

### What to Back Up

- Configuration files
- SSL certificates
- Custom templates (if any)
- Log files (if persistent)

### Backup Procedure

1. Create a backup script:
   ```bash
   #!/bin/bash
   BACKUP_DIR="/backup/imap-api/$(date +%Y-%m-%d)"
   mkdir -p $BACKUP_DIR
   
   # Backup configuration
   cp /opt/imap-api/config.toml $BACKUP_DIR/
   
   # Backup certs if present
   if [ -d /opt/imap-api/certs ]; then
     cp -r /opt/imap-api/certs $BACKUP_DIR/
   fi
   
   # Backup templates
   if [ -d /opt/imap-api/templates ]; then
     cp -r /opt/imap-api/templates $BACKUP_DIR/
   fi
   
   # Compress the backup
   tar -czf $BACKUP_DIR.tar.gz $BACKUP_DIR
   rm -rf $BACKUP_DIR
   ```

2. Schedule the backup with cron:
   ```
   0 2 * * * /path/to/backup-script.sh
   ```

### Restore Procedure

1. Extract the backup:
   ```bash
   tar -xzf /backup/imap-api/2023-06-15.tar.gz -C /tmp/
   ```

2. Copy files to the appropriate locations:
   ```bash
   cp -r /tmp/2023-06-15/* /opt/imap-api/
   ```

3. Restart the service:
   ```bash
   sudo systemctl restart imap-api
   ```

## Scaling Considerations

### Horizontal Scaling

The IMAP API service is stateless and can be horizontally scaled by:

1. Deploying multiple instances behind a load balancer
2. Increasing Kubernetes replica count
3. Ensuring each instance has a separate connection pool to the IMAP server

### Vertical Scaling

For improved performance on a single node:

1. Increase IMAP connection pool size (default: 10)
2. Allocate more CPU cores and memory
3. Adjust the Actix worker threads (default: number of CPU cores)

### Load Testing

Before scaling, perform load testing:

```bash
# Using ab (Apache Benchmark)
ab -n 1000 -c 50 -H "Authorization: Basic <base64-credentials>" http://your-api-host:8080/folders

# Using k6
k6 run performance-tests/folders-test.js
```

## Security Recommendations

### TLS Configuration

It's highly recommended to run the API behind TLS, either directly configured or using a reverse proxy:

1. **Direct TLS Configuration** - Modify `config.toml`:
   ```toml
   [server]
   host = "0.0.0.0"
   port = 443
   tls_cert_file = "/path/to/cert.pem"
   tls_key_file = "/path/to/key.pem"
   ```

2. **Nginx as TLS Termination Proxy**:
   ```nginx
   server {
       listen 443 ssl;
       server_name api.example.com;
       
       ssl_certificate /etc/nginx/certs/cert.pem;
       ssl_certificate_key /etc/nginx/certs/key.pem;
       
       location / {
           proxy_pass http://localhost:8080;
           proxy_set_header Host $host;
           proxy_set_header X-Real-IP $remote_addr;
       }
   }
   ```

### Authentication

1. Use strong, unique passwords for IMAP accounts
2. Consider implementing API key authentication for additional security
3. Implement rate limiting to prevent brute force attacks

### Secure Configuration

1. Never store credentials in source code
2. Use environment variables or secure secret management tools
3. Restrict file permissions on configuration files containing credentials

## Troubleshooting

### Common Issues

#### Connection Refused

**Symptom**: Application fails to start with "connection refused" error.

**Solution**: 
- Verify the host and port settings
- Check for firewall rules blocking connections
- Ensure the IMAP server is running and accessible

#### Authentication Errors

**Symptom**: Application starts but returns 401 errors for all requests.

**Solution**:
- Verify IMAP credentials in configuration
- Check if the IMAP account has 2FA enabled (may require app-specific password)
- Ensure the account has not been locked due to too many failed attempts

#### High Memory Usage

**Symptom**: Application consumes excessive memory over time.

**Solution**:
- Check for connection leaks in IMAP client code
- Adjust connection pool settings
- Implement appropriate timeouts for idle connections

### Debug Mode

Enable debug logging for more detailed output:

```
LOG_LEVEL=debug ./imap-api-rust
```

### Getting Support

For additional support:

1. Check the [GitHub Issues](https://github.com/your-organization/imap-api-rust/issues) for similar problems
2. Review logs for detailed error messages
3. Contact the development team with detailed information about your deployment and specific errors 