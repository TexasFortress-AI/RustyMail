# Kubernetes Deployment Guide

This guide covers deploying RustyMail on Kubernetes clusters using kubectl, Helm, and various Kubernetes distributions.

## Prerequisites

- Kubernetes cluster 1.19+
- kubectl configured with cluster access
- 2+ worker nodes (production)
- 4GB RAM per node minimum
- Storage provisioner for PersistentVolumes
- Ingress controller (optional)

### Verify Cluster Access

```bash
# Check kubectl version and cluster info
kubectl version --short
kubectl cluster-info
kubectl get nodes
```

## Quick Start

### Basic Deployment

```bash
# Create namespace
kubectl create namespace rustymail

# Apply configurations
kubectl apply -f k8s/ -n rustymail

# Verify deployment
kubectl get all -n rustymail

# Check application health
kubectl port-forward -n rustymail svc/rustymail 9437:9437
curl http://localhost:9437/health
```

## Kubernetes Manifests

### Namespace

```yaml
# k8s/00-namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: rustymail
  labels:
    app: rustymail
    environment: production
```

### ConfigMap

```yaml
# k8s/01-configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: rustymail-config
  namespace: rustymail
data:
  # Server configuration
  REST_HOST: "0.0.0.0"
  REST_PORT: "9437"
  SSE_HOST: "0.0.0.0"
  SSE_PORT: "9438"
  DASHBOARD_ENABLED: "true"
  DASHBOARD_PORT: "9439"

  # Logging
  LOG_LEVEL: "info"
  RUST_LOG: "rustymail=info"

  # Performance
  MAX_CONNECTIONS: "20"
  CONNECTION_TIMEOUT: "30"

  # Rate limiting
  RATE_LIMIT_REQUESTS: "100"
  RATE_LIMIT_PERIOD: "60"
```

### Secret

```yaml
# k8s/02-secret.yaml
apiVersion: v1
kind: Secret
metadata:
  name: rustymail-secret
  namespace: rustymail
type: Opaque
stringData:
  # IMAP credentials
  IMAP_HOST: "imap.gmail.com"
  IMAP_PORT: "993"
  IMAP_USERNAME: "your-email@gmail.com"
  IMAP_PASSWORD: "your-app-password"

  # AI Service Keys (optional)
  OPENAI_API_KEY: ""
  OPENROUTER_API_KEY: ""
```

### PersistentVolume and Claim

```yaml
# k8s/03-storage.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: rustymail-data
  namespace: rustymail
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 10Gi
  storageClassName: standard

---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: rustymail-logs
  namespace: rustymail
spec:
  accessModes:
    - ReadWriteMany
  resources:
    requests:
      storage: 5Gi
  storageClassName: standard
```

### Deployment

```yaml
# k8s/04-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rustymail
  namespace: rustymail
  labels:
    app: rustymail
    version: v1
spec:
  replicas: 3
  selector:
    matchLabels:
      app: rustymail
  template:
    metadata:
      labels:
        app: rustymail
        version: v1
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9437"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: rustymail
      securityContext:
        runAsNonRoot: true
        runAsUser: 1001
        fsGroup: 1001

      containers:
      - name: rustymail
        image: rustymail:latest
        imagePullPolicy: IfNotPresent

        ports:
        - name: rest-api
          containerPort: 9437
          protocol: TCP
        - name: sse
          containerPort: 9438
          protocol: TCP
        - name: dashboard
          containerPort: 9439
          protocol: TCP

        envFrom:
        - configMapRef:
            name: rustymail-config
        - secretRef:
            name: rustymail-secret

        resources:
          requests:
            memory: "512Mi"
            cpu: "250m"
          limits:
            memory: "2Gi"
            cpu: "1000m"

        livenessProbe:
          httpGet:
            path: /health
            port: rest-api
          initialDelaySeconds: 30
          periodSeconds: 30
          timeoutSeconds: 5
          failureThreshold: 3

        readinessProbe:
          httpGet:
            path: /health
            port: rest-api
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 3
          failureThreshold: 3

        volumeMounts:
        - name: data
          mountPath: /app/data
        - name: logs
          mountPath: /app/logs
        - name: tmp
          mountPath: /tmp

        securityContext:
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
              - ALL
            add:
              - NET_BIND_SERVICE

      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: rustymail-data
      - name: logs
        persistentVolumeClaim:
          claimName: rustymail-logs
      - name: tmp
        emptyDir: {}

      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchExpressions:
                - key: app
                  operator: In
                  values:
                  - rustymail
              topologyKey: kubernetes.io/hostname
```

### Service

```yaml
# k8s/05-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: rustymail
  namespace: rustymail
  labels:
    app: rustymail
spec:
  type: ClusterIP
  selector:
    app: rustymail
  ports:
  - name: rest-api
    port: 9437
    targetPort: rest-api
    protocol: TCP
  - name: sse
    port: 9438
    targetPort: sse
    protocol: TCP
  - name: dashboard
    port: 9439
    targetPort: dashboard
    protocol: TCP
  sessionAffinity: ClientIP
  sessionAffinityConfig:
    clientIP:
      timeoutSeconds: 3600

---
# Optional: NodePort service for external access
apiVersion: v1
kind: Service
metadata:
  name: rustymail-nodeport
  namespace: rustymail
spec:
  type: NodePort
  selector:
    app: rustymail
  ports:
  - name: rest-api
    port: 9437
    nodePort: 30437
    protocol: TCP
```

### Ingress

```yaml
# k8s/06-ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: rustymail
  namespace: rustymail
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
    nginx.ingress.kubernetes.io/proxy-body-size: "10m"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "600"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "600"
    cert-manager.io/cluster-issuer: letsencrypt-prod
spec:
  ingressClassName: nginx
  tls:
  - hosts:
    - rustymail.example.com
    secretName: rustymail-tls
  rules:
  - host: rustymail.example.com
    http:
      paths:
      - path: /api
        pathType: Prefix
        backend:
          service:
            name: rustymail
            port:
              number: 9437
      - path: /sse
        pathType: Prefix
        backend:
          service:
            name: rustymail
            port:
              number: 9438
      - path: /
        pathType: Prefix
        backend:
          service:
            name: rustymail
            port:
              number: 9439
```

### ServiceAccount and RBAC

```yaml
# k8s/07-rbac.yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: rustymail
  namespace: rustymail

---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: rustymail
  namespace: rustymail
rules:
- apiGroups: [""]
  resources: ["configmaps", "secrets"]
  verbs: ["get", "list", "watch"]
- apiGroups: [""]
  resources: ["pods"]
  verbs: ["get", "list"]

---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: rustymail
  namespace: rustymail
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: Role
  name: rustymail
subjects:
- kind: ServiceAccount
  name: rustymail
  namespace: rustymail
```

### HorizontalPodAutoscaler

```yaml
# k8s/08-hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: rustymail
  namespace: rustymail
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: rustymail
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 100
        periodSeconds: 60
```

### NetworkPolicy

```yaml
# k8s/09-networkpolicy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: rustymail
  namespace: rustymail
spec:
  podSelector:
    matchLabels:
      app: rustymail
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
    - podSelector:
        matchLabels:
          app: prometheus
    ports:
    - protocol: TCP
      port: 9437
    - protocol: TCP
      port: 9438
    - protocol: TCP
      port: 9439
  egress:
  - to:
    - podSelector: {}
    ports:
    - protocol: TCP
      port: 53
    - protocol: UDP
      port: 53
  - to:
    - namespaceSelector: {}
    ports:
    - protocol: TCP
      port: 993  # IMAPS
    - protocol: TCP
      port: 443  # HTTPS
```

## Helm Chart

### Install with Helm

```bash
# Add Helm repository
helm repo add rustymail https://charts.rustymail.io
helm repo update

# Install with custom values
helm install rustymail rustymail/rustymail \
  --namespace rustymail \
  --create-namespace \
  --values values.yaml
```

### values.yaml

```yaml
# Helm values.yaml
replicaCount: 3

image:
  repository: rustymail
  tag: latest
  pullPolicy: IfNotPresent

imagePullSecrets: []

service:
  type: ClusterIP
  ports:
    restApi: 9437
    sse: 9438
    dashboard: 9439

ingress:
  enabled: true
  className: nginx
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
  hosts:
    - host: rustymail.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: rustymail-tls
      hosts:
        - rustymail.example.com

resources:
  limits:
    cpu: 1000m
    memory: 2Gi
  requests:
    cpu: 250m
    memory: 512Mi

autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80

persistence:
  enabled: true
  storageClass: standard
  size: 10Gi
  accessMode: ReadWriteOnce

config:
  imap:
    adapter: gmail
    host: imap.gmail.com
    port: 993
    username: ""  # Set via secrets
    password: ""  # Set via secrets

  server:
    logLevel: info
    maxConnections: 20
    connectionTimeout: 30

  security:
    requireHttps: true
    rateLimitRequests: 100
    rateLimitPeriod: 60

monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s

nodeSelector: {}
tolerations: []
affinity:
  podAntiAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 100
        podAffinityTerm:
          labelSelector:
            matchExpressions:
              - key: app
                operator: In
                values:
                  - rustymail
          topologyKey: kubernetes.io/hostname
```

## Deployment Strategies

### Rolling Update

```yaml
spec:
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 1
```

### Blue-Green Deployment

```bash
# Deploy green version
kubectl apply -f k8s/deployment-green.yaml

# Switch traffic to green
kubectl patch service rustymail -p '{"spec":{"selector":{"version":"green"}}}'

# Remove blue version
kubectl delete deployment rustymail-blue
```

### Canary Deployment

```yaml
# Using Flagger
apiVersion: flagger.app/v1beta1
kind: Canary
metadata:
  name: rustymail
  namespace: rustymail
spec:
  targetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: rustymail
  progressDeadlineSeconds: 600
  service:
    port: 9437
  analysis:
    interval: 30s
    threshold: 5
    maxWeight: 50
    stepWeight: 10
    metrics:
    - name: request-success-rate
      thresholdRange:
        min: 99
      interval: 1m
```

## Kubernetes Distributions

### Amazon EKS

```bash
# Create cluster
eksctl create cluster \
  --name rustymail-cluster \
  --version 1.27 \
  --region us-west-2 \
  --nodegroup-name standard-workers \
  --node-type t3.medium \
  --nodes 3

# Install AWS Load Balancer Controller
kubectl apply -k "github.com/aws/eks-charts/stable/aws-load-balancer-controller/crds"
helm install aws-load-balancer-controller \
  eks/aws-load-balancer-controller \
  -n kube-system

# Deploy application
kubectl apply -f k8s/
```

### Google GKE

```bash
# Create cluster
gcloud container clusters create rustymail-cluster \
  --zone us-central1-a \
  --num-nodes 3 \
  --machine-type n1-standard-2 \
  --enable-autoscaling \
  --min-nodes 2 \
  --max-nodes 10

# Get credentials
gcloud container clusters get-credentials rustymail-cluster \
  --zone us-central1-a

# Deploy application
kubectl apply -f k8s/
```

### Azure AKS

```bash
# Create resource group
az group create --name rustymail-rg --location eastus

# Create AKS cluster
az aks create \
  --resource-group rustymail-rg \
  --name rustymail-cluster \
  --node-count 3 \
  --enable-addons monitoring \
  --generate-ssh-keys

# Get credentials
az aks get-credentials \
  --resource-group rustymail-rg \
  --name rustymail-cluster

# Deploy application
kubectl apply -f k8s/
```

## Monitoring and Observability

### Prometheus Monitoring

```yaml
# ServiceMonitor for Prometheus Operator
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: rustymail
  namespace: rustymail
spec:
  selector:
    matchLabels:
      app: rustymail
  endpoints:
  - port: rest-api
    interval: 30s
    path: /metrics
```

### Grafana Dashboard

```json
{
  "dashboard": {
    "title": "RustyMail Metrics",
    "panels": [
      {
        "title": "Request Rate",
        "targets": [
          {
            "expr": "rate(http_requests_total[5m])"
          }
        ]
      },
      {
        "title": "Error Rate",
        "targets": [
          {
            "expr": "rate(http_requests_total{status=~\"5..\"}[5m])"
          }
        ]
      }
    ]
  }
}
```

## Backup and Disaster Recovery

### Velero Backup

```bash
# Install Velero
velero install \
  --provider aws \
  --bucket rustymail-backups \
  --secret-file ./credentials

# Create backup
velero backup create rustymail-backup \
  --include-namespaces rustymail \
  --wait

# Schedule daily backups
velero schedule create daily-backup \
  --schedule="0 2 * * *" \
  --include-namespaces rustymail
```

## Security Best Practices

### Pod Security Policy

```yaml
apiVersion: policy/v1beta1
kind: PodSecurityPolicy
metadata:
  name: rustymail
spec:
  privileged: false
  allowPrivilegeEscalation: false
  requiredDropCapabilities:
    - ALL
  volumes:
    - 'configMap'
    - 'secret'
    - 'persistentVolumeClaim'
    - 'emptyDir'
  runAsUser:
    rule: 'MustRunAsNonRoot'
  seLinux:
    rule: 'RunAsAny'
  fsGroup:
    rule: 'RunAsAny'
```

### Network Policies

```yaml
# Deny all traffic by default
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: default-deny-all
  namespace: rustymail
spec:
  podSelector: {}
  policyTypes:
  - Ingress
  - Egress
```

## Troubleshooting

### Common Issues

#### Pods Not Starting

```bash
# Check pod status
kubectl get pods -n rustymail
kubectl describe pod <pod-name> -n rustymail

# Check logs
kubectl logs <pod-name> -n rustymail
kubectl logs <pod-name> -n rustymail --previous

# Check events
kubectl get events -n rustymail --sort-by='.lastTimestamp'
```

#### Storage Issues

```bash
# Check PVC status
kubectl get pvc -n rustymail

# Check PV
kubectl get pv

# Debug volume mount
kubectl exec -it <pod-name> -n rustymail -- ls -la /app/data
```

#### Network Issues

```bash
# Test connectivity
kubectl run test-pod --image=busybox -it --rm --restart=Never -- wget -O- http://rustymail:9437/health

# Check service endpoints
kubectl get endpoints -n rustymail

# DNS resolution
kubectl run test-pod --image=busybox -it --rm --restart=Never -- nslookup rustymail.rustymail.svc.cluster.local
```

## Performance Optimization

### Resource Tuning

```yaml
resources:
  requests:
    memory: "1Gi"
    cpu: "500m"
  limits:
    memory: "2Gi"
    cpu: "1000m"
```

### JVM-style Tuning (if applicable)

```yaml
env:
  - name: RUST_THREADS
    value: "4"
  - name: TOKIO_WORKER_THREADS
    value: "4"
```

## CI/CD Integration

### GitOps with ArgoCD

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: rustymail
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/yourusername/rustymail
    targetRevision: HEAD
    path: k8s
  destination:
    server: https://kubernetes.default.svc
    namespace: rustymail
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
```

## Next Steps

- Configure [monitoring and alerts](monitoring.md)
- Implement [security best practices](security.md)
- Set up [CI/CD pipelines](cicd.md)
- Configure [backup and disaster recovery](backup-recovery.md)