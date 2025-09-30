# Docker Deployment Guide

This guide covers deploying RustyMail using Docker and Docker Compose for both development and production environments.

## Prerequisites

- Docker Engine 20.10 or later
- Docker Compose 2.0 or later
- 4GB RAM available for Docker
- Network access to pull images and reach IMAP servers

### Install Docker

#### Linux
```bash
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER
```

#### macOS/Windows
Download and install Docker Desktop from [docker.com](https://www.docker.com/products/docker-desktop)

## Quick Start

### Using Docker Compose (Recommended)

```bash
# Clone the repository
git clone https://github.com/yourusername/rustymail.git
cd rustymail

# Copy and configure environment
cp .env.example .env
# Edit .env with your IMAP credentials

# Start the service
docker-compose -f docker-compose.prod.yml up -d

# View logs
docker-compose -f docker-compose.prod.yml logs -f

# Stop the service
docker-compose -f docker-compose.prod.yml down
```

### Using Docker CLI

```bash
# Build the image
docker build -t rustymail:latest .

# Run container with environment variables
docker run -d \
  --name rustymail-server \
  -p 9437:9437 \
  -p 9438:9438 \
  -p 9439:9439 \
  -e IMAP_HOST=imap.gmail.com \
  -e IMAP_USERNAME=your-email@gmail.com \
  -e IMAP_PASSWORD=your-password \
  -e RUST_LOG=info \
  -v $(pwd)/data:/app/data \
  rustymail:latest
```

## Production Docker Compose Configuration

### Complete docker-compose.prod.yml

```yaml
version: '3.8'

services:
  rustymail:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        - BUILD_VERSION=${BUILD_VERSION:-latest}
    image: rustymail:${BUILD_VERSION:-latest}
    container_name: rustymail-server
    restart: unless-stopped

    ports:
      - "${REST_PORT:-9437}:9437"     # REST API
      - "${SSE_PORT:-9438}:9438"       # SSE endpoint
      - "${DASHBOARD_PORT:-9439}:9439" # Dashboard

    environment:
      # IMAP Configuration
      - IMAP_ADAPTER=${IMAP_ADAPTER:-gmail}
      - IMAP_HOST=${IMAP_HOST}
      - IMAP_PORT=${IMAP_PORT:-993}
      - IMAP_USERNAME=${IMAP_USERNAME}
      - IMAP_PASSWORD=${IMAP_PASSWORD}

      # Server Configuration
      - REST_HOST=0.0.0.0
      - REST_PORT=9437
      - SSE_HOST=0.0.0.0
      - SSE_PORT=9438

      # Dashboard
      - DASHBOARD_ENABLED=${DASHBOARD_ENABLED:-true}
      - DASHBOARD_PORT=9439

      # Logging
      - RUST_LOG=${RUST_LOG:-info}
      - LOG_LEVEL=${LOG_LEVEL:-info}

      # Performance
      - MAX_CONNECTIONS=${MAX_CONNECTIONS:-10}
      - CONNECTION_TIMEOUT=${CONNECTION_TIMEOUT:-30}

      # Security
      - REQUIRE_HTTPS=${REQUIRE_HTTPS:-false}
      - RATE_LIMIT_REQUESTS=${RATE_LIMIT_REQUESTS:-100}
      - RATE_LIMIT_PERIOD=${RATE_LIMIT_PERIOD:-60}

      # AI Services (optional)
      - OPENAI_API_KEY=${OPENAI_API_KEY:-}
      - OPENROUTER_API_KEY=${OPENROUTER_API_KEY:-}

    volumes:
      - rustymail_data:/app/data
      - ./config:/app/config:ro
      - ./logs:/app/logs

    networks:
      - rustymail_network

    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:9437/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s

    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M

    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
        labels: "service=rustymail"

  # Optional: Redis for caching
  redis:
    image: redis:7-alpine
    container_name: rustymail-redis
    restart: unless-stopped
    networks:
      - rustymail_network
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes
    deploy:
      resources:
        limits:
          memory: 256M

  # Optional: Nginx reverse proxy
  nginx:
    image: nginx:alpine
    container_name: rustymail-proxy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx/nginx.conf:/etc/nginx/nginx.conf:ro
      - ./nginx/ssl:/etc/nginx/ssl:ro
      - nginx_cache:/var/cache/nginx
    networks:
      - rustymail_network
    depends_on:
      - rustymail

networks:
  rustymail_network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.25.0.0/16

volumes:
  rustymail_data:
    driver: local
  redis_data:
    driver: local
  nginx_cache:
    driver: local
```

## Development Environment

### docker-compose.dev.yml

```yaml
version: '3.8'

services:
  rustymail-dev:
    build:
      context: .
      dockerfile: Dockerfile.dev
      target: development
    image: rustymail:dev
    container_name: rustymail-dev

    ports:
      - "9437:9437"
      - "9438:9438"
      - "9439:9439"
      - "9999:9999"  # Debug port

    environment:
      - RUST_LOG=debug
      - RUST_BACKTRACE=1
      - CARGO_WATCH_POLL=true

    volumes:
      - .:/usr/src/app
      - cargo_cache:/usr/local/cargo/registry
      - target_cache:/usr/src/app/target

    command: cargo watch -x run

  # Test IMAP server for development
  greenmail:
    image: greenmail/standalone:latest
    container_name: rustymail-test-imap
    environment:
      - GREENMAIL_USERS=test@test.com:password
      - GREENMAIL_OPTS=-Dgreenmail.setup.test.all
    ports:
      - "3143:3143"  # IMAP
      - "3993:3993"  # IMAPS
      - "3025:3025"  # SMTP

volumes:
  cargo_cache:
  target_cache:
```

## Multi-Stage Dockerfile

### Optimized Dockerfile

```dockerfile
# Build stage
FROM rust:1.75 AS builder

WORKDIR /usr/src/rustymail

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source and build
COPY src ./src
COPY tests ./tests
RUN touch src/main.rs && \
    cargo build --release --bin rustymail-server

# Frontend build stage
FROM node:18-alpine AS frontend-builder

WORKDIR /app
COPY frontend/package*.json ./
RUN npm ci --only=production

COPY frontend ./
RUN npm run build

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    libssl3 \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1001 -U rustymail && \
    mkdir -p /app/data /app/logs /app/config && \
    chown -R rustymail:rustymail /app

WORKDIR /app

# Copy artifacts
COPY --from=builder --chown=rustymail:rustymail \
    /usr/src/rustymail/target/release/rustymail-server /app/
COPY --from=frontend-builder --chown=rustymail:rustymail \
    /app/dist /app/frontend/dist

USER rustymail

# Runtime configuration
EXPOSE 9437 9438 9439
VOLUME ["/app/data", "/app/logs", "/app/config"]

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:9437/health || exit 1

ENTRYPOINT ["./rustymail-server"]
```

## Container Management

### Basic Operations

```bash
# View running containers
docker ps

# View container logs
docker logs -f rustymail-server

# Execute commands in container
docker exec -it rustymail-server /bin/bash

# Inspect container
docker inspect rustymail-server

# View resource usage
docker stats rustymail-server
```

### Volume Management

```bash
# List volumes
docker volume ls

# Inspect volume
docker volume inspect rustymail_data

# Backup volume
docker run --rm \
  -v rustymail_data:/data \
  -v $(pwd)/backup:/backup \
  alpine tar czf /backup/rustymail-backup-$(date +%Y%m%d).tar.gz -C /data .

# Restore volume
docker run --rm \
  -v rustymail_data:/data \
  -v $(pwd)/backup:/backup \
  alpine tar xzf /backup/rustymail-backup-20240101.tar.gz -C /data
```

## Network Configuration

### Custom Bridge Network

```yaml
networks:
  rustymail_network:
    driver: bridge
    driver_opts:
      com.docker.network.bridge.name: br-rustymail
    ipam:
      driver: default
      config:
        - subnet: 172.25.0.0/16
          gateway: 172.25.0.1
```

### Host Network Mode (Linux only)

```yaml
services:
  rustymail:
    network_mode: host
    environment:
      - REST_HOST=127.0.0.1
      - REST_PORT=9437
```

## Security Hardening

### Read-Only Root Filesystem

```yaml
services:
  rustymail:
    read_only: true
    tmpfs:
      - /tmp
      - /run
    volumes:
      - rustymail_data:/app/data
      - rustymail_logs:/app/logs:rw
```

### Security Options

```yaml
services:
  rustymail:
    security_opt:
      - no-new-privileges:true
      - seccomp:unconfined
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
      - CHOWN
      - SETUID
      - SETGID
```

## Deployment with Docker Swarm

### Initialize Swarm

```bash
# Initialize swarm
docker swarm init

# Deploy stack
docker stack deploy -c docker-compose.prod.yml rustymail

# View services
docker service ls

# Scale service
docker service scale rustymail_rustymail=3

# Update service
docker service update --image rustymail:v2.0 rustymail_rustymail
```

### Swarm Configuration

```yaml
services:
  rustymail:
    deploy:
      replicas: 3
      update_config:
        parallelism: 1
        delay: 10s
        failure_action: rollback
      restart_policy:
        condition: on-failure
        delay: 5s
        max_attempts: 3
      placement:
        constraints:
          - node.role == worker
          - node.labels.rustymail == true
```

## Registry and Image Management

### Using Docker Registry

```bash
# Tag image for registry
docker tag rustymail:latest registry.example.com/rustymail:latest

# Push to registry
docker push registry.example.com/rustymail:latest

# Pull from registry
docker pull registry.example.com/rustymail:latest
```

### Multi-Architecture Builds

```bash
# Setup buildx
docker buildx create --name multiarch --use

# Build for multiple platforms
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --tag rustymail:latest \
  --push .
```

## Monitoring and Logging

### Prometheus Metrics

```yaml
services:
  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
    ports:
      - "9090:9090"
```

### Centralized Logging

```yaml
services:
  rustymail:
    logging:
      driver: fluentd
      options:
        fluentd-address: localhost:24224
        tag: rustymail.{{.Name}}
```

## Troubleshooting

### Common Issues

#### Container Fails to Start

```bash
# Check logs
docker logs rustymail-server

# Check events
docker events --filter container=rustymail-server

# Debug mode
docker run -it --rm \
  --entrypoint /bin/bash \
  rustymail:latest
```

#### Permission Issues

```bash
# Fix volume permissions
docker run --rm \
  -v rustymail_data:/data \
  alpine chown -R 1001:1001 /data
```

#### Network Connectivity

```bash
# Test from container
docker run --rm \
  --network rustymail_network \
  alpine ping rustymail-server

# Inspect network
docker network inspect rustymail_network
```

### Performance Tuning

```bash
# Increase Docker daemon resources
# Edit /etc/docker/daemon.json
{
  "default-ulimits": {
    "nofile": {
      "Name": "nofile",
      "Hard": 64000,
      "Soft": 64000
    }
  },
  "max-concurrent-downloads": 10,
  "max-concurrent-uploads": 10
}

# Restart Docker
sudo systemctl restart docker
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Docker Build and Push

on:
  push:
    tags:
      - 'v*'

jobs:
  docker:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v1

      - name: Login to DockerHub
        uses: docker/login-action@v1
        with:
          username: ${{ secrets.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}

      - name: Build and push
        uses: docker/build-push-action@v2
        with:
          push: true
          tags: |
            user/rustymail:latest
            user/rustymail:${{ github.ref_name }}
```

## Best Practices

1. **Always use specific image tags** in production
2. **Implement health checks** for automatic recovery
3. **Use secrets management** for sensitive data
4. **Limit container resources** to prevent resource exhaustion
5. **Regular security updates** of base images
6. **Implement proper logging** and monitoring
7. **Use multi-stage builds** to minimize image size
8. **Scan images for vulnerabilities** before deployment

## Next Steps

- Set up [Kubernetes deployment](kubernetes-deployment.md)
- Configure [monitoring and alerts](monitoring.md)
- Implement [security best practices](security.md)
- Set up [backup and recovery](backup-recovery.md)