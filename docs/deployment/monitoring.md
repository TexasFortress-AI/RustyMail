# Monitoring and Logging Guide

This guide covers monitoring, logging, and observability setup for RustyMail deployments.

## Overview

RustyMail provides comprehensive monitoring and logging capabilities:

- **Metrics**: Prometheus-compatible metrics endpoint
- **Logging**: Structured JSON logging with multiple outputs
- **Tracing**: Distributed tracing support
- **Health Checks**: Multiple health and readiness endpoints
- **Alerting**: Integration with common alerting systems

## Metrics

### Built-in Metrics

RustyMail exposes metrics at `/metrics` endpoint in Prometheus format:

```bash
# View metrics
curl http://localhost:9437/metrics
```

#### Available Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `rustymail_http_requests_total` | Counter | Total HTTP requests |
| `rustymail_http_request_duration_seconds` | Histogram | Request duration |
| `rustymail_http_requests_in_flight` | Gauge | Current requests being processed |
| `rustymail_imap_connections` | Gauge | Active IMAP connections |
| `rustymail_imap_operations_total` | Counter | Total IMAP operations |
| `rustymail_email_processed_total` | Counter | Emails processed |
| `rustymail_email_processing_duration_seconds` | Histogram | Email processing time |
| `rustymail_cache_hits_total` | Counter | Cache hit count |
| `rustymail_cache_misses_total` | Counter | Cache miss count |
| `rustymail_rate_limit_exceeded_total` | Counter | Rate limit violations |

### Prometheus Configuration

#### prometheus.yml

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'rustymail'
    static_configs:
      - targets: ['rustymail:9437']
    metrics_path: /metrics
    scrape_interval: 30s
    scrape_timeout: 10s

  - job_name: 'rustymail-redis'
    static_configs:
      - targets: ['redis:6379']

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']

rule_files:
  - '/etc/prometheus/alerts/*.yml'
```

#### Alert Rules

Create `/etc/prometheus/alerts/rustymail.yml`:

```yaml
groups:
  - name: rustymail
    interval: 30s
    rules:
      - alert: RustyMailDown
        expr: up{job="rustymail"} == 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "RustyMail is down"
          description: "RustyMail has been down for more than 5 minutes"

      - alert: HighErrorRate
        expr: rate(rustymail_http_requests_total{status=~"5.."}[5m]) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"
          description: "Error rate is above 5% for 5 minutes"

      - alert: HighResponseTime
        expr: histogram_quantile(0.95, rate(rustymail_http_request_duration_seconds_bucket[5m])) > 1
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High response time"
          description: "95th percentile response time is above 1 second"

      - alert: IMAPConnectionFailure
        expr: rustymail_imap_connections == 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "No IMAP connections"
          description: "No active IMAP connections for 5 minutes"

      - alert: HighMemoryUsage
        expr: container_memory_usage_bytes{name="rustymail-server"} / container_spec_memory_limit_bytes{name="rustymail-server"} > 0.9
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High memory usage"
          description: "Memory usage is above 90% of limit"
```

### Grafana Dashboard

#### Dashboard JSON

```json
{
  "dashboard": {
    "title": "RustyMail Monitoring",
    "panels": [
      {
        "title": "Request Rate",
        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 0},
        "targets": [
          {
            "expr": "rate(rustymail_http_requests_total[5m])",
            "legendFormat": "{{method}} {{status}}"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Response Time (p95)",
        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 0},
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(rustymail_http_request_duration_seconds_bucket[5m]))",
            "legendFormat": "95th percentile"
          }
        ],
        "type": "graph"
      },
      {
        "title": "IMAP Connections",
        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 8},
        "targets": [
          {
            "expr": "rustymail_imap_connections",
            "legendFormat": "Active Connections"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Email Processing",
        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 8},
        "targets": [
          {
            "expr": "rate(rustymail_email_processed_total[5m])",
            "legendFormat": "Emails/sec"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Cache Performance",
        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 16},
        "targets": [
          {
            "expr": "rate(rustymail_cache_hits_total[5m])",
            "legendFormat": "Cache Hits"
          },
          {
            "expr": "rate(rustymail_cache_misses_total[5m])",
            "legendFormat": "Cache Misses"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Error Rate",
        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 16},
        "targets": [
          {
            "expr": "rate(rustymail_http_requests_total{status=~\"5..\"}[5m])",
            "legendFormat": "5xx Errors"
          },
          {
            "expr": "rate(rustymail_http_requests_total{status=~\"4..\"}[5m])",
            "legendFormat": "4xx Errors"
          }
        ],
        "type": "graph"
      }
    ]
  }
}
```

#### Grafana Provisioning

Create `grafana/provisioning/dashboards/dashboard.yml`:

```yaml
apiVersion: 1

providers:
  - name: 'RustyMail'
    orgId: 1
    folder: ''
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    options:
      path: /etc/grafana/provisioning/dashboards
```

Create `grafana/provisioning/datasources/prometheus.yml`:

```yaml
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090
    isDefault: true
    editable: true
```

## Logging

### Log Configuration

#### Environment Variables

```bash
# Log level: trace, debug, info, warn, error
LOG_LEVEL=info

# Rust-specific logging
RUST_LOG=rustymail=info,tower=warn,hyper=warn

# Log format: json, pretty, compact
LOG_FORMAT=json

# Log outputs
LOG_FILE=/var/log/rustymail/app.log
LOG_MAX_SIZE=100  # MB
LOG_MAX_BACKUPS=5

# Structured logging fields
LOG_TIMESTAMPS=true
LOG_COLORS=auto  # always, never, auto
```

### Log Aggregation with Loki

#### Loki Configuration

Create `loki/loki-config.yaml`:

```yaml
auth_enabled: false

server:
  http_listen_port: 3100

ingester:
  lifecycler:
    address: 127.0.0.1
    ring:
      kvstore:
        store: inmemory
      replication_factor: 1
  chunk_idle_period: 15m
  chunk_retain_period: 30s

schema_config:
  configs:
    - from: 2020-10-24
      store: boltdb-shipper
      object_store: filesystem
      schema: v11
      index:
        prefix: index_
        period: 24h

storage_config:
  boltdb_shipper:
    active_index_directory: /loki/boltdb-shipper-active
    cache_location: /loki/boltdb-shipper-cache
    shared_store: filesystem
  filesystem:
    directory: /loki/chunks

limits_config:
  enforce_metric_name: false
  reject_old_samples: true
  reject_old_samples_max_age: 168h

chunk_store_config:
  max_look_back_period: 0s

table_manager:
  retention_deletes_enabled: false
  retention_period: 0s
```

#### Promtail Configuration

Create `promtail/promtail-config.yaml`:

```yaml
server:
  http_listen_port: 9080
  grpc_listen_port: 0

positions:
  filename: /tmp/positions.yaml

clients:
  - url: http://loki:3100/loki/api/v1/push

scrape_configs:
  - job_name: rustymail
    static_configs:
      - targets:
          - localhost
        labels:
          job: rustymail
          __path__: /var/log/rustymail/*.log
    pipeline_stages:
      - json:
          expressions:
            level: level
            timestamp: timestamp
            message: message
            module: module
      - labels:
          level:
          module:
      - timestamp:
          source: timestamp
          format: RFC3339
```

### Fluentd Configuration

Create `fluentd/fluent.conf`:

```conf
<source>
  @type forward
  port 24224
  bind 0.0.0.0
</source>

<filter rustymail.**>
  @type parser
  key_name log
  <parse>
    @type json
  </parse>
</filter>

<match rustymail.**>
  @type elasticsearch
  host elasticsearch
  port 9200
  index_name rustymail
  type_name _doc
  logstash_format true
  logstash_prefix rustymail
  <buffer>
    @type file
    path /var/log/fluentd/buffer/rustymail
    flush_interval 10s
  </buffer>
</match>
```

### ELK Stack Integration

#### Logstash Configuration

Create `logstash/pipeline/rustymail.conf`:

```conf
input {
  tcp {
    port => 5000
    codec => json
  }
}

filter {
  if [service] == "rustymail" {
    mutate {
      add_field => { "[@metadata][target_index]" => "rustymail-%{+YYYY.MM.dd}" }
    }

    date {
      match => [ "timestamp", "ISO8601" ]
      target => "@timestamp"
    }

    if [level] == "ERROR" {
      mutate {
        add_tag => [ "error" ]
      }
    }
  }
}

output {
  elasticsearch {
    hosts => ["elasticsearch:9200"]
    index => "%{[@metadata][target_index]}"
  }
}
```

## Health Checks

### Endpoints

| Endpoint | Description | Response |
|----------|-------------|----------|
| `/health` | Basic health check | `{"status":"healthy","version":"1.0.0"}` |
| `/health/live` | Liveness probe | HTTP 200 if alive |
| `/health/ready` | Readiness probe | HTTP 200 if ready to serve |
| `/health/detailed` | Detailed health info | Full system status |

### Health Check Configuration

```yaml
# Docker Compose
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:9437/health"]
  interval: 30s
  timeout: 5s
  retries: 3
  start_period: 10s

# Kubernetes
livenessProbe:
  httpGet:
    path: /health/live
    port: 9437
  initialDelaySeconds: 30
  periodSeconds: 30

readinessProbe:
  httpGet:
    path: /health/ready
    port: 9437
  initialDelaySeconds: 10
  periodSeconds: 10
```

### Custom Health Checks

```bash
#!/bin/bash
# health-check.sh

# Check REST API
if ! curl -sf http://localhost:9437/health > /dev/null; then
  echo "REST API health check failed"
  exit 1
fi

# Check IMAP connection
if ! curl -sf http://localhost:9437/health/detailed | jq -e '.imap.connected' > /dev/null; then
  echo "IMAP connection check failed"
  exit 1
fi

# Check cache
if ! curl -sf http://localhost:9437/health/detailed | jq -e '.cache.available' > /dev/null; then
  echo "Cache check failed"
  exit 1
fi

echo "All health checks passed"
```

## Distributed Tracing

### Jaeger Setup

```yaml
# docker-compose.yml
services:
  jaeger:
    image: jaegertracing/all-in-one:latest
    container_name: rustymail-jaeger
    ports:
      - "16686:16686"  # Jaeger UI
      - "14268:14268"  # Collector HTTP
    environment:
      - COLLECTOR_ZIPKIN_HTTP_PORT=9411
    networks:
      - rustymail_network
```

### Application Configuration

```bash
# Enable tracing
TRACES_ENABLED=true
TRACES_ENDPOINT=http://jaeger:14268/api/traces
TRACES_SAMPLE_RATE=0.1  # Sample 10% of requests
TRACES_SERVICE_NAME=rustymail
```

### OpenTelemetry Integration

```yaml
# otel-collector-config.yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

processors:
  batch:

exporters:
  jaeger:
    endpoint: jaeger:14250
    tls:
      insecure: true

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: [batch]
      exporters: [jaeger]
```

## Application Performance Monitoring (APM)

### New Relic Integration

```bash
# Environment variables
NEW_RELIC_APP_NAME=RustyMail
NEW_RELIC_LICENSE_KEY=your-license-key
NEW_RELIC_LOG_LEVEL=info
```

### Datadog Integration

```yaml
# datadog-agent.yaml
services:
  datadog-agent:
    image: datadog/agent:latest
    container_name: rustymail-datadog
    environment:
      - DD_API_KEY=${DD_API_KEY}
      - DD_SITE=datadoghq.com
      - DD_APM_ENABLED=true
      - DD_LOGS_ENABLED=true
      - DD_LOGS_CONFIG_CONTAINER_COLLECT_ALL=true
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - /proc/:/host/proc/:ro
      - /sys/fs/cgroup/:/host/sys/fs/cgroup:ro
    networks:
      - rustymail_network
```

## Monitoring Best Practices

### Key Metrics to Monitor

1. **Application Metrics**
   - Request rate and latency
   - Error rates (4xx, 5xx)
   - Active connections
   - Queue depths

2. **Infrastructure Metrics**
   - CPU usage
   - Memory consumption
   - Disk I/O
   - Network traffic

3. **Business Metrics**
   - Emails processed per minute
   - User sessions
   - API usage by endpoint

### Alert Thresholds

| Metric | Warning | Critical |
|--------|---------|----------|
| CPU Usage | >70% | >90% |
| Memory Usage | >80% | >95% |
| Error Rate | >1% | >5% |
| Response Time (p95) | >500ms | >1s |
| IMAP Connections | <2 | 0 |
| Disk Usage | >80% | >90% |

### Log Retention Policies

```yaml
# Log retention settings
logging:
  policies:
    - name: application
      pattern: "*.log"
      retention_days: 30
      compress_after_days: 7

    - name: errors
      pattern: "*error*.log"
      retention_days: 90
      compress_after_days: 1

    - name: audit
      pattern: "*audit*.log"
      retention_days: 365
      compress_after_days: 30
```

## Troubleshooting Monitoring

### Common Issues

#### No Metrics Available

```bash
# Check metrics endpoint
curl -v http://localhost:9437/metrics

# Check Prometheus configuration
promtool check config /etc/prometheus/prometheus.yml

# Verify scrape targets
curl http://localhost:9090/api/v1/targets
```

#### Missing Logs

```bash
# Check log file permissions
ls -la /var/log/rustymail/

# Verify logging configuration
echo $LOG_LEVEL
echo $RUST_LOG

# Test log output
RUST_LOG=debug ./rustymail-server
```

#### High Cardinality Metrics

```bash
# Identify high cardinality metrics
curl -s http://localhost:9437/metrics | grep -E "^[^#]" | cut -d'{' -f1 | sort | uniq -c | sort -rn

# Limit label cardinality in configuration
```

## Monitoring Scripts

### metrics-collector.sh

```bash
#!/bin/bash
# Collect and store metrics locally

METRICS_DIR="/var/lib/rustymail/metrics"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

mkdir -p "$METRICS_DIR"

# Collect metrics
curl -s http://localhost:9437/metrics > "$METRICS_DIR/metrics_${TIMESTAMP}.txt"

# Compress old metrics
find "$METRICS_DIR" -name "*.txt" -mtime +7 -exec gzip {} \;

# Delete very old metrics
find "$METRICS_DIR" -name "*.gz" -mtime +30 -delete
```

### alert-test.sh

```bash
#!/bin/bash
# Test alerting pipeline

# Generate high error rate
for i in {1..100}; do
  curl -X POST http://localhost:9437/api/v1/invalid-endpoint
done

# Check if alert fired
sleep 60
curl -s http://localhost:9093/api/v1/alerts | jq '.[] | select(.labels.alertname=="HighErrorRate")'
```

## Next Steps

- Configure [security monitoring](security.md#security-monitoring)
- Set up [backup monitoring](backup-recovery.md)
- Implement [SLA monitoring](sla-monitoring.md)
- Review [performance tuning](performance.md)