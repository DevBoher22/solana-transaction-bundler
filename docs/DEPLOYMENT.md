# Deployment Guide

This guide covers deploying the Solana Transaction Bundler in various environments, from development to production.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Environment Setup](#environment-setup)
- [Configuration Management](#configuration-management)
- [Deployment Methods](#deployment-methods)
  - [Docker Deployment](#docker-deployment)
  - [Kubernetes Deployment](#kubernetes-deployment)
  - [Systemd Service](#systemd-service)
  - [Cloud Deployments](#cloud-deployments)
- [Security Considerations](#security-considerations)
- [Monitoring and Observability](#monitoring-and-observability)
- [Scaling and Load Balancing](#scaling-and-load-balancing)
- [Backup and Recovery](#backup-and-recovery)
- [Troubleshooting](#troubleshooting)

## Prerequisites

### System Requirements

**Minimum Requirements:**
- CPU: 2 cores
- RAM: 4GB
- Storage: 20GB SSD
- Network: 100 Mbps

**Recommended for Production:**
- CPU: 4+ cores
- RAM: 8GB+
- Storage: 100GB+ NVMe SSD
- Network: 1 Gbps+

### Software Dependencies

- **Operating System**: Linux (Ubuntu 20.04+ recommended)
- **Container Runtime**: Docker 20.10+ (if using containers)
- **Orchestrator**: Kubernetes 1.20+ (if using K8s)
- **Monitoring**: Prometheus, Grafana (optional)

### Network Requirements

**Outbound Connections:**
- Solana RPC endpoints (port 443/80)
- AWS KMS (port 443, if using KMS)
- Container registries (port 443)

**Inbound Connections:**
- HTTP API (default port 8080)
- Metrics endpoint (default port 9090)
- Health checks (same as HTTP API)

## Environment Setup

### Development Environment

```bash
# Clone repository
git clone https://github.com/your-org/solana-bundler.git
cd solana-bundler

# Setup development environment
make setup

# Generate development keypair
solana-keygen new --outfile dev-keypair.json

# Set environment variables
export SOLANA_PRIVATE_KEY=$(cat dev-keypair.json)
export RUST_LOG=debug

# Start development server
make dev
```

### Staging Environment

```bash
# Use staging configuration
cp examples/bundler.config.toml staging.config.toml

# Edit configuration for staging
vim staging.config.toml

# Build release binary
make build-release

# Start service
./target/release/bundler-service --config staging.config.toml
```

### Production Environment

Production deployments require additional security and reliability measures:

1. **Use KMS for key management**
2. **Enable comprehensive monitoring**
3. **Implement proper logging**
4. **Set up automated backups**
5. **Configure load balancing**

## Configuration Management

### Configuration Files

Create environment-specific configuration files:

```
configs/
├── development.toml
├── staging.toml
└── production.toml
```

### Environment Variables

Set environment-specific variables:

```bash
# Production environment variables
export ENVIRONMENT=production
export RUST_LOG=info
export SOLANA_PRIVATE_KEY_KMS_ARN=arn:aws:kms:us-east-1:123456789012:key/...
export AWS_REGION=us-east-1
export PROMETHEUS_ENDPOINT=http://prometheus:9090
```

### Secrets Management

**Development:**
```bash
# Use environment variables
export SOLANA_PRIVATE_KEY="base58_encoded_key"
```

**Production:**
```bash
# Use AWS Secrets Manager
aws secretsmanager create-secret \
  --name "solana-bundler/private-key" \
  --secret-string "base58_encoded_key"

# Or use Kubernetes secrets
kubectl create secret generic bundler-secrets \
  --from-literal=private-key="base58_encoded_key"
```

## Deployment Methods

### Docker Deployment

#### Single Container

```bash
# Build image
docker build -t solana-bundler:latest .

# Run container
docker run -d \
  --name solana-bundler \
  -p 8080:8080 \
  -p 9090:9090 \
  -e SOLANA_PRIVATE_KEY="your_private_key" \
  -e RUST_LOG=info \
  -v $(pwd)/production.config.toml:/app/bundler.config.toml:ro \
  solana-bundler:latest
```

#### Docker Compose

```yaml
# docker-compose.prod.yml
version: '3.8'

services:
  bundler:
    image: solana-bundler:latest
    ports:
      - "8080:8080"
      - "9090:9090"
    environment:
      - RUST_LOG=info
      - SOLANA_PRIVATE_KEY=${SOLANA_PRIVATE_KEY}
    volumes:
      - ./production.config.toml:/app/bundler.config.toml:ro
      - bundler-logs:/app/logs
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/v1/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9091:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml:ro
    restart: unless-stopped

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_PASSWORD}
    volumes:
      - grafana-data:/var/lib/grafana
    restart: unless-stopped

volumes:
  bundler-logs:
  grafana-data:
```

Deploy with:
```bash
docker-compose -f docker-compose.prod.yml up -d
```

### Kubernetes Deployment

#### Namespace and ConfigMap

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: solana-bundler

---
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: bundler-config
  namespace: solana-bundler
data:
  bundler.config.toml: |
    [rpc]
    endpoints = [
        { url = "https://api.mainnet-beta.solana.com", weight = 100 }
    ]
    timeout_seconds = 30
    
    [fees]
    strategy = "p75_plus_buffer"
    base_fee_lamports = 5000
    
    [security]
    program_whitelist = [
        "11111111111111111111111111111112"
    ]
    
    [signing]
    fee_payer = { type = "kms", key_id = "${KMS_KEY_ID}", region = "${AWS_REGION}" }
    
    [service]
    port = 8080
    host = "0.0.0.0"
```

#### Secret

```yaml
# secret.yaml
apiVersion: v1
kind: Secret
metadata:
  name: bundler-secrets
  namespace: solana-bundler
type: Opaque
data:
  private-key: <base64_encoded_private_key>
  kms-key-id: <base64_encoded_kms_key_id>
```

#### Deployment

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: solana-bundler
  namespace: solana-bundler
  labels:
    app: solana-bundler
spec:
  replicas: 3
  selector:
    matchLabels:
      app: solana-bundler
  template:
    metadata:
      labels:
        app: solana-bundler
    spec:
      containers:
      - name: bundler
        image: your-registry/solana-bundler:v0.1.0
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 9090
          name: metrics
        env:
        - name: RUST_LOG
          value: "info"
        - name: KMS_KEY_ID
          valueFrom:
            secretKeyRef:
              name: bundler-secrets
              key: kms-key-id
        - name: AWS_REGION
          value: "us-east-1"
        volumeMounts:
        - name: config
          mountPath: /app/bundler.config.toml
          subPath: bundler.config.toml
        resources:
          requests:
            memory: "512Mi"
            cpu: "250m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /v1/health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /v1/health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config
        configMap:
          name: bundler-config
      serviceAccountName: bundler-service-account
```

#### Service and Ingress

```yaml
# service.yaml
apiVersion: v1
kind: Service
metadata:
  name: solana-bundler-service
  namespace: solana-bundler
spec:
  selector:
    app: solana-bundler
  ports:
  - name: http
    port: 80
    targetPort: 8080
  - name: metrics
    port: 9090
    targetPort: 9090
  type: ClusterIP

---
# ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: solana-bundler-ingress
  namespace: solana-bundler
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/rate-limit: "1000"
spec:
  tls:
  - hosts:
    - bundler.yourdomain.com
    secretName: bundler-tls
  rules:
  - host: bundler.yourdomain.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: solana-bundler-service
            port:
              number: 80
```

#### Deploy to Kubernetes

```bash
# Apply all manifests
kubectl apply -f k8s/

# Check deployment status
kubectl get pods -n solana-bundler
kubectl logs -f deployment/solana-bundler -n solana-bundler

# Port forward for testing
kubectl port-forward svc/solana-bundler-service 8080:80 -n solana-bundler
```

### Systemd Service

For bare metal or VM deployments:

```ini
# /etc/systemd/system/solana-bundler.service
[Unit]
Description=Solana Transaction Bundler
After=network.target
Wants=network.target

[Service]
Type=simple
User=bundler
Group=bundler
WorkingDirectory=/opt/solana-bundler
ExecStart=/opt/solana-bundler/bundler-service --config /etc/solana-bundler/bundler.config.toml
Restart=always
RestartSec=5
Environment=RUST_LOG=info
EnvironmentFile=/etc/solana-bundler/environment

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/solana-bundler/logs

[Install]
WantedBy=multi-user.target
```

Setup and start:

```bash
# Create user
sudo useradd -r -s /bin/false bundler

# Create directories
sudo mkdir -p /opt/solana-bundler/{logs,data}
sudo mkdir -p /etc/solana-bundler

# Copy binary and config
sudo cp target/release/bundler-service /opt/solana-bundler/
sudo cp production.config.toml /etc/solana-bundler/bundler.config.toml

# Set permissions
sudo chown -R bundler:bundler /opt/solana-bundler
sudo chmod 600 /etc/solana-bundler/bundler.config.toml

# Enable and start service
sudo systemctl enable solana-bundler
sudo systemctl start solana-bundler
sudo systemctl status solana-bundler
```

### Cloud Deployments

#### AWS ECS

```json
{
  "family": "solana-bundler",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "1024",
  "memory": "2048",
  "executionRoleArn": "arn:aws:iam::123456789012:role/ecsTaskExecutionRole",
  "taskRoleArn": "arn:aws:iam::123456789012:role/solana-bundler-task-role",
  "containerDefinitions": [
    {
      "name": "solana-bundler",
      "image": "your-account.dkr.ecr.us-east-1.amazonaws.com/solana-bundler:latest",
      "portMappings": [
        {
          "containerPort": 8080,
          "protocol": "tcp"
        }
      ],
      "environment": [
        {
          "name": "RUST_LOG",
          "value": "info"
        },
        {
          "name": "AWS_REGION",
          "value": "us-east-1"
        }
      ],
      "secrets": [
        {
          "name": "KMS_KEY_ID",
          "valueFrom": "arn:aws:secretsmanager:us-east-1:123456789012:secret:solana-bundler/kms-key-id"
        }
      ],
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/solana-bundler",
          "awslogs-region": "us-east-1",
          "awslogs-stream-prefix": "ecs"
        }
      },
      "healthCheck": {
        "command": [
          "CMD-SHELL",
          "curl -f http://localhost:8080/v1/health || exit 1"
        ],
        "interval": 30,
        "timeout": 5,
        "retries": 3
      }
    }
  ]
}
```

#### Google Cloud Run

```yaml
# cloudrun.yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: solana-bundler
  annotations:
    run.googleapis.com/ingress: all
spec:
  template:
    metadata:
      annotations:
        autoscaling.knative.dev/maxScale: "10"
        run.googleapis.com/cpu-throttling: "false"
    spec:
      containerConcurrency: 100
      containers:
      - image: gcr.io/your-project/solana-bundler:latest
        ports:
        - containerPort: 8080
        env:
        - name: RUST_LOG
          value: info
        - name: GOOGLE_CLOUD_PROJECT
          value: your-project
        resources:
          limits:
            cpu: "2"
            memory: "4Gi"
        livenessProbe:
          httpGet:
            path: /v1/health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
```

Deploy:
```bash
gcloud run services replace cloudrun.yaml --region=us-central1
```

## Security Considerations

### Key Management

**Development:**
- Use environment variables for testing only
- Never commit keys to version control

**Production:**
- Use AWS KMS, Google Cloud KMS, or Azure Key Vault
- Implement key rotation policies
- Use IAM roles with minimal permissions

### Network Security

```bash
# Firewall rules (iptables example)
# Allow HTTP API
iptables -A INPUT -p tcp --dport 8080 -j ACCEPT

# Allow metrics (restrict to monitoring network)
iptables -A INPUT -p tcp --dport 9090 -s 10.0.1.0/24 -j ACCEPT

# Allow SSH (restrict to management network)
iptables -A INPUT -p tcp --dport 22 -s 10.0.0.0/24 -j ACCEPT

# Drop all other traffic
iptables -A INPUT -j DROP
```

### Container Security

```dockerfile
# Use non-root user
USER bundler

# Read-only root filesystem
--read-only

# Drop capabilities
--cap-drop=ALL

# No new privileges
--security-opt=no-new-privileges
```

### Kubernetes Security

```yaml
# SecurityContext
securityContext:
  runAsNonRoot: true
  runAsUser: 1000
  readOnlyRootFilesystem: true
  allowPrivilegeEscalation: false
  capabilities:
    drop:
    - ALL

# NetworkPolicy
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: solana-bundler-netpol
spec:
  podSelector:
    matchLabels:
      app: solana-bundler
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to: []
    ports:
    - protocol: TCP
      port: 443  # HTTPS to Solana RPC
```

## Monitoring and Observability

### Prometheus Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
- job_name: 'solana-bundler'
  static_configs:
  - targets: ['bundler:9090']
  metrics_path: /metrics
  scrape_interval: 10s
```

### Grafana Dashboard

Import the provided dashboard or create custom panels:

```json
{
  "dashboard": {
    "title": "Solana Bundler Metrics",
    "panels": [
      {
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(bundler_requests_total[5m])"
          }
        ]
      },
      {
        "title": "Success Rate",
        "type": "singlestat",
        "targets": [
          {
            "expr": "bundler_success_rate"
          }
        ]
      }
    ]
  }
}
```

### Alerting Rules

```yaml
# alerts.yml
groups:
- name: solana-bundler
  rules:
  - alert: BundlerHighErrorRate
    expr: rate(bundler_requests_total{status="error"}[5m]) > 0.1
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "High error rate detected"
      
  - alert: BundlerServiceDown
    expr: up{job="solana-bundler"} == 0
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Bundler service is down"
```

### Log Aggregation

**ELK Stack:**
```yaml
# filebeat.yml
filebeat.inputs:
- type: log
  paths:
    - /app/logs/*.log
  json.keys_under_root: true
  json.add_error_key: true

output.elasticsearch:
  hosts: ["elasticsearch:9200"]
```

**Fluentd:**
```xml
<source>
  @type tail
  path /app/logs/*.log
  pos_file /var/log/fluentd/bundler.log.pos
  tag bundler.*
  format json
</source>

<match bundler.**>
  @type elasticsearch
  host elasticsearch
  port 9200
  index_name bundler-logs
</match>
```

## Scaling and Load Balancing

### Horizontal Scaling

**Kubernetes HPA:**
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: solana-bundler-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: solana-bundler
  minReplicas: 3
  maxReplicas: 20
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
```

### Load Balancing

**Nginx Configuration:**
```nginx
upstream solana_bundler {
    least_conn;
    server bundler1:8080 max_fails=3 fail_timeout=30s;
    server bundler2:8080 max_fails=3 fail_timeout=30s;
    server bundler3:8080 max_fails=3 fail_timeout=30s;
}

server {
    listen 80;
    server_name bundler.yourdomain.com;
    
    location / {
        proxy_pass http://solana_bundler;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_connect_timeout 30s;
        proxy_send_timeout 30s;
        proxy_read_timeout 30s;
    }
    
    location /v1/health {
        proxy_pass http://solana_bundler;
        access_log off;
    }
}
```

## Backup and Recovery

### Configuration Backup

```bash
#!/bin/bash
# backup-config.sh

BACKUP_DIR="/backup/solana-bundler"
DATE=$(date +%Y%m%d_%H%M%S)

# Create backup directory
mkdir -p "$BACKUP_DIR/$DATE"

# Backup configuration
cp /etc/solana-bundler/bundler.config.toml "$BACKUP_DIR/$DATE/"

# Backup environment file
cp /etc/solana-bundler/environment "$BACKUP_DIR/$DATE/"

# Create archive
tar -czf "$BACKUP_DIR/config-backup-$DATE.tar.gz" -C "$BACKUP_DIR" "$DATE"

# Clean up old backups (keep 30 days)
find "$BACKUP_DIR" -name "config-backup-*.tar.gz" -mtime +30 -delete
```

### Database Backup (if applicable)

```bash
#!/bin/bash
# backup-data.sh

# Backup metrics data (if using local Prometheus)
docker exec prometheus tar -czf - /prometheus > "/backup/prometheus-$(date +%Y%m%d).tar.gz"

# Backup logs
tar -czf "/backup/logs-$(date +%Y%m%d).tar.gz" /app/logs/
```

### Disaster Recovery Plan

1. **Identify failure scope**
2. **Restore from backup**
3. **Verify configuration**
4. **Restart services**
5. **Validate functionality**
6. **Monitor for issues**

## Troubleshooting

### Common Issues

**Service won't start:**
```bash
# Check logs
journalctl -u solana-bundler -f

# Check configuration
bundler config --validate

# Check permissions
ls -la /etc/solana-bundler/
```

**High latency:**
```bash
# Check RPC endpoint health
curl -X POST https://api.mainnet-beta.solana.com \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'

# Check system resources
top
iostat -x 1
```

**Memory issues:**
```bash
# Check memory usage
free -h
ps aux --sort=-%mem | head

# Check for memory leaks
valgrind --tool=memcheck ./bundler-service
```

### Debug Mode

Enable debug logging:
```bash
export RUST_LOG=debug
bundler-service --config bundler.config.toml
```

### Health Check Debugging

```bash
# Manual health check
curl -v http://localhost:8080/v1/health

# Component-specific checks
curl http://localhost:8080/v1/health?verbose=true | jq .
```

### Performance Profiling

```bash
# CPU profiling
perf record -g ./bundler-service --config bundler.config.toml
perf report

# Memory profiling
valgrind --tool=massif ./bundler-service --config bundler.config.toml
```

## Support

For deployment support:
- **Documentation**: [GitHub Wiki](https://github.com/your-org/solana-bundler/wiki)
- **Issues**: [GitHub Issues](https://github.com/your-org/solana-bundler/issues)
- **Deployment Support**: deployment-support@yourorg.com
