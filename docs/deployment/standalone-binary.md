# Standalone Binary Deployment

This guide covers deploying RustyMail as a standalone binary on Linux, macOS, and Windows systems.

## Prerequisites

- 64-bit operating system (Linux, macOS, or Windows)
- OpenSSL libraries installed (Linux/macOS)
- Network access to IMAP servers
- Sufficient permissions to bind to configured ports

## Installation Methods

### Method 1: Pre-built Binaries

#### Linux

```bash
# Download the latest release
wget https://github.com/yourusername/rustymail/releases/latest/download/rustymail-linux-x64.tar.gz

# Extract the archive
tar -xzf rustymail-linux-x64.tar.gz

# Make executable
chmod +x rustymail-server

# Verify installation
./rustymail-server --version
```

#### macOS

```bash
# Download the latest release
curl -L https://github.com/yourusername/rustymail/releases/latest/download/rustymail-macos-x64.tar.gz -o rustymail-macos-x64.tar.gz

# Extract the archive
tar -xzf rustymail-macos-x64.tar.gz

# Make executable
chmod +x rustymail-server

# Remove quarantine attribute (macOS Gatekeeper)
xattr -d com.apple.quarantine rustymail-server

# Verify installation
./rustymail-server --version
```

#### Windows

```powershell
# Download the latest release
Invoke-WebRequest -Uri "https://github.com/yourusername/rustymail/releases/latest/download/rustymail-windows-x64.zip" -OutFile "rustymail-windows-x64.zip"

# Extract the archive
Expand-Archive -Path "rustymail-windows-x64.zip" -DestinationPath "."

# Verify installation
.\rustymail-server.exe --version
```

### Method 2: Build from Source

#### Requirements

- Rust 1.75 or later
- Cargo build tool
- Git

#### Build Steps

```bash
# Clone the repository
git clone https://github.com/yourusername/rustymail.git
cd rustymail

# Build in release mode
cargo build --release --bin rustymail-server

# Binary will be at target/release/rustymail-server
ls -la target/release/rustymail-server

# Install to system (optional)
sudo cp target/release/rustymail-server /usr/local/bin/
```

## Configuration

### Environment Variables

Create a configuration file for environment variables:

```bash
# Create config directory
mkdir -p /etc/rustymail

# Create environment file
cat > /etc/rustymail/rustymail.env << EOF
# IMAP Configuration
IMAP_ADAPTER=gmail
IMAP_HOST=imap.gmail.com
IMAP_PORT=993
IMAP_USERNAME=your-email@gmail.com
IMAP_PASSWORD=your-app-password

# Server Configuration
REST_HOST=0.0.0.0
REST_PORT=9437
SSE_HOST=0.0.0.0
SSE_PORT=9438

# Dashboard Configuration
DASHBOARD_ENABLED=true
DASHBOARD_PORT=9439
DASHBOARD_PATH=/usr/local/share/rustymail/dashboard

# Logging
LOG_LEVEL=info
RUST_LOG=rustymail=info

# Security
REQUIRE_HTTPS=false
RATE_LIMIT_REQUESTS=100
RATE_LIMIT_PERIOD=60

# Performance
MAX_CONNECTIONS=10
CONNECTION_TIMEOUT=30
EOF

# Secure the configuration file
chmod 600 /etc/rustymail/rustymail.env
```

### Running the Server

#### Direct Execution

```bash
# Load environment and run
source /etc/rustymail/rustymail.env
./rustymail-server

# Or use env file directly
env $(cat /etc/rustymail/rustymail.env | xargs) ./rustymail-server

# Run in background
nohup ./rustymail-server > /var/log/rustymail/server.log 2>&1 &
```

## System Service Setup

### systemd (Linux)

Create a systemd service for automatic startup and management:

```bash
# Create service file
sudo cat > /etc/systemd/system/rustymail.service << EOF
[Unit]
Description=RustyMail Email Management Server
Documentation=https://github.com/yourusername/rustymail
After=network.target

[Service]
Type=simple
User=rustymail
Group=rustymail
WorkingDirectory=/opt/rustymail
EnvironmentFile=/etc/rustymail/rustymail.env
ExecStart=/opt/rustymail/rustymail-server
ExecReload=/bin/kill -HUP \$MAINPID
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=rustymail

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/rustymail /var/log/rustymail

[Install]
WantedBy=multi-user.target
EOF

# Create user and directories
sudo useradd -r -s /bin/false rustymail
sudo mkdir -p /opt/rustymail /var/lib/rustymail /var/log/rustymail
sudo chown -R rustymail:rustymail /opt/rustymail /var/lib/rustymail /var/log/rustymail

# Copy binary
sudo cp rustymail-server /opt/rustymail/

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable rustymail
sudo systemctl start rustymail

# Check status
sudo systemctl status rustymail
sudo journalctl -u rustymail -f
```

### launchd (macOS)

Create a launchd plist for automatic startup:

```bash
# Create plist file
sudo cat > /Library/LaunchDaemons/com.rustymail.server.plist << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.rustymail.server</string>

    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/rustymail-server</string>
    </array>

    <key>EnvironmentVariables</key>
    <dict>
        <key>IMAP_HOST</key>
        <string>imap.gmail.com</string>
        <key>IMAP_USERNAME</key>
        <string>your-email@gmail.com</string>
        <key>IMAP_PASSWORD</key>
        <string>your-password</string>
        <key>REST_PORT</key>
        <string>9437</string>
        <key>SSE_PORT</key>
        <string>9438</string>
        <key>DASHBOARD_PORT</key>
        <string>9439</string>
    </dict>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>

    <key>StandardOutPath</key>
    <string>/var/log/rustymail/stdout.log</string>

    <key>StandardErrorPath</key>
    <string>/var/log/rustymail/stderr.log</string>
</dict>
</plist>
EOF

# Load and start service
sudo launchctl load /Library/LaunchDaemons/com.rustymail.server.plist
sudo launchctl start com.rustymail.server

# Check status
sudo launchctl list | grep rustymail
```

### Windows Service

Use NSSM (Non-Sucking Service Manager) to create a Windows service:

```powershell
# Download NSSM
Invoke-WebRequest -Uri "https://nssm.cc/release/nssm-2.24.zip" -OutFile "nssm.zip"
Expand-Archive -Path "nssm.zip" -DestinationPath "."

# Install service
.\nssm\win64\nssm.exe install RustyMail "C:\Program Files\RustyMail\rustymail-server.exe"

# Configure service
.\nssm\win64\nssm.exe set RustyMail AppDirectory "C:\Program Files\RustyMail"
.\nssm\win64\nssm.exe set RustyMail AppEnvironmentExtra "IMAP_HOST=imap.gmail.com" "IMAP_USERNAME=email@gmail.com" "IMAP_PASSWORD=password"

# Start service
.\nssm\win64\nssm.exe start RustyMail

# Check status
Get-Service RustyMail
```

## Directory Structure

Recommended directory layout for production:

```
/opt/rustymail/              # Application directory
├── rustymail-server         # Main binary
├── config/                  # Configuration files
│   └── rustymail.env       # Environment variables
├── data/                    # Application data
│   ├── cache/              # Email cache
│   └── sessions/           # Session data
└── logs/                    # Log files
    ├── access.log          # Access logs
    └── error.log           # Error logs

/var/lib/rustymail/          # Persistent data
└── database/                # Local database (if used)

/var/log/rustymail/          # System logs
├── rustymail.log           # Application logs
└── audit.log               # Audit logs
```

## Health Checks

Verify the server is running correctly:

```bash
# Check health endpoint
curl http://localhost:9437/health

# Expected response:
# {"status":"healthy","version":"1.0.0","uptime":12345}

# Check API endpoints
curl http://localhost:9437/api/v1/status

# Check Dashboard (if enabled)
curl http://localhost:9439/

# Monitor logs
tail -f /var/log/rustymail/rustymail.log
```

## Troubleshooting

### Common Issues

#### Port Already in Use

```bash
# Check what's using the port
lsof -i :9437  # Linux/macOS
netstat -ano | findstr :9437  # Windows

# Use alternative port
export REST_PORT=9447
```

#### Permission Denied

```bash
# Ports below 1024 require root
# Use ports above 1024 or use capability
sudo setcap 'cap_net_bind_service=+ep' /opt/rustymail/rustymail-server
```

#### IMAP Connection Failed

```bash
# Test IMAP connectivity
openssl s_client -connect imap.gmail.com:993

# Check credentials
./rustymail-server --test-connection

# Enable debug logging
export RUST_LOG=debug
export LOG_LEVEL=debug
```

### Log Locations

- **systemd**: `journalctl -u rustymail`
- **Direct execution**: `./rustymail-server 2>&1 | tee server.log`
- **Windows Event Log**: Event Viewer > Applications

## Performance Tuning

### System Limits

```bash
# Increase file descriptors
ulimit -n 65536

# Add to /etc/security/limits.conf
rustymail soft nofile 65536
rustymail hard nofile 65536
```

### Connection Pool

```bash
# Optimize for your workload
export MAX_CONNECTIONS=50
export CONNECTION_TIMEOUT=60
```

### Memory Settings

```bash
# Limit memory usage (optional)
systemctl set-property rustymail.service MemoryLimit=2G
```

## Backup and Recovery

### Backup Script

```bash
#!/bin/bash
# backup-rustymail.sh

BACKUP_DIR="/backup/rustymail"
DATE=$(date +%Y%m%d-%H%M%S)

# Stop service
systemctl stop rustymail

# Backup data
tar -czf "$BACKUP_DIR/rustymail-data-$DATE.tar.gz" /var/lib/rustymail/

# Backup config
tar -czf "$BACKUP_DIR/rustymail-config-$DATE.tar.gz" /etc/rustymail/

# Start service
systemctl start rustymail

# Keep last 30 days
find $BACKUP_DIR -name "*.tar.gz" -mtime +30 -delete
```

## Updates and Maintenance

### Update Procedure

```bash
# Download new version
wget https://github.com/yourusername/rustymail/releases/latest/download/rustymail-linux-x64.tar.gz

# Stop service
sudo systemctl stop rustymail

# Backup current version
sudo cp /opt/rustymail/rustymail-server /opt/rustymail/rustymail-server.backup

# Install new version
tar -xzf rustymail-linux-x64.tar.gz
sudo cp rustymail-server /opt/rustymail/

# Start service
sudo systemctl start rustymail

# Verify update
curl http://localhost:9437/health
```

## Next Steps

- Configure [TLS/SSL certificates](security.md#tls-configuration)
- Set up [monitoring and alerts](monitoring.md)
- Review [security best practices](security.md)
- Integrate with [reverse proxy](reverse-proxy.md)