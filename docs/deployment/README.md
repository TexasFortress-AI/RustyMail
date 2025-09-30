# RustyMail Deployment Guide

RustyMail is a comprehensive email management server that can be deployed in multiple ways to suit your infrastructure needs. This guide covers all supported deployment methods and configurations.

## Table of Contents

- [Deployment Methods](#deployment-methods)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Security](#security)
- [Monitoring](#monitoring)

## Deployment Methods

RustyMail supports three primary deployment methods:

1. **[Standalone Binary](standalone-binary.md)** - Direct installation on Linux/macOS/Windows
2. **[Docker](docker-deployment.md)** - Containerized deployment using Docker
3. **[Kubernetes](kubernetes-deployment.md)** - Orchestrated deployment on Kubernetes clusters

## Prerequisites

### Minimum System Requirements

- **CPU**: 2 cores (4 cores recommended)
- **Memory**: 2GB RAM (4GB recommended for production)
- **Storage**: 10GB available space (varies with email volume)
- **Network**: Stable internet connection for IMAP server access

### Required Dependencies

- **Runtime**: Linux/macOS/Windows x86_64
- **For Docker**: Docker Engine 20.10+ and Docker Compose 2.0+
- **For Kubernetes**: kubectl 1.19+ and cluster access
- **SSL/TLS**: Valid certificates for production deployments

## Quick Start

### Using Docker (Recommended)

```bash
# Clone the repository
git clone https://github.com/yourusername/rustymail.git
cd rustymail

# Copy environment configuration
cp .env.example .env
# Edit .env with your IMAP credentials

# Start with Docker Compose
docker-compose -f docker-compose.prod.yml up -d

# Verify deployment
curl http://localhost:9437/health
```

### Using Pre-built Binary

```bash
# Download latest release
wget https://github.com/yourusername/rustymail/releases/latest/download/rustymail-linux-x64.tar.gz
tar -xzf rustymail-linux-x64.tar.gz

# Configure environment
export IMAP_HOST=imap.example.com
export IMAP_USERNAME=your-email@example.com
export IMAP_PASSWORD=your-password

# Run the server
./rustymail-server
```

## Configuration

RustyMail uses environment variables for configuration. Key settings include:

- **IMAP Configuration**: Connection to email servers
- **API Endpoints**: REST API, SSE, and Dashboard ports
- **Security**: HTTPS requirements and rate limiting
- **Performance**: Connection pools and timeouts

See [Configuration Guide](configuration.md) for detailed documentation.

## Security

For production deployments, ensure:

1. **Use HTTPS/TLS** for all external connections
2. **Secure credentials** using secrets management
3. **Network isolation** with proper firewall rules
4. **Regular updates** of RustyMail and dependencies

See [Security Best Practices](security.md) for comprehensive guidelines.

## Monitoring

RustyMail provides:

- Health check endpoint at `/health`
- Prometheus metrics (when enabled)
- Structured JSON logging
- Integration with common monitoring stacks

See [Monitoring Guide](monitoring.md) for setup instructions.

## Support

- **Documentation**: Full docs at `/docs`
- **Issues**: Report bugs on GitHub
- **Updates**: Follow releases for security patches

## Next Steps

1. Choose your [deployment method](#deployment-methods)
2. Review the [configuration guide](configuration.md)
3. Implement [security best practices](security.md)
4. Set up [monitoring](monitoring.md)
5. Test with the [REST API examples](../REST-EXAMPLES.md)