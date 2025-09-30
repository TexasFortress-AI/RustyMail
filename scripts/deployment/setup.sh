#!/usr/bin/env bash

# RustyMail Initial Setup Script
# This script handles initial setup and configuration of RustyMail

set -euo pipefail

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_header() {
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}   RustyMail Setup Wizard${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
}

# Function to detect OS
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        OS="linux"
        if [ -f /etc/debian_version ]; then
            DISTRO="debian"
        elif [ -f /etc/redhat-release ]; then
            DISTRO="redhat"
        else
            DISTRO="unknown"
        fi
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        OS="macos"
        DISTRO="macos"
    elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]]; then
        OS="windows"
        DISTRO="windows"
    else
        OS="unknown"
        DISTRO="unknown"
    fi

    print_info "Detected OS: $OS ($DISTRO)"
}

# Function to check and install dependencies
install_dependencies() {
    print_info "Checking dependencies..."

    local deps_to_install=()

    # Check Docker
    if ! command -v docker >/dev/null 2>&1; then
        deps_to_install+=("docker")
    fi

    # Check Docker Compose
    if ! command -v docker-compose >/dev/null 2>&1; then
        deps_to_install+=("docker-compose")
    fi

    # Check other tools
    for cmd in curl jq openssl; do
        if ! command -v $cmd >/dev/null 2>&1; then
            deps_to_install+=("$cmd")
        fi
    done

    if [ ${#deps_to_install[@]} -eq 0 ]; then
        print_success "All dependencies are installed"
        return
    fi

    print_warning "Missing dependencies: ${deps_to_install[*]}"

    read -p "Would you like to install missing dependencies? (y/n): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_warning "Please install dependencies manually and run setup again"
        exit 1
    fi

    # Install based on OS
    case "$OS" in
        linux)
            if [ "$DISTRO" == "debian" ]; then
                sudo apt-get update
                sudo apt-get install -y "${deps_to_install[@]}"
            elif [ "$DISTRO" == "redhat" ]; then
                sudo yum install -y "${deps_to_install[@]}"
            fi
            ;;
        macos)
            if command -v brew >/dev/null 2>&1; then
                brew install "${deps_to_install[@]}"
            else
                print_error "Homebrew not found. Please install from https://brew.sh"
                exit 1
            fi
            ;;
        *)
            print_error "Automatic dependency installation not supported for $OS"
            print_info "Please install manually: ${deps_to_install[*]}"
            exit 1
            ;;
    esac

    print_success "Dependencies installed successfully"
}

# Function to setup environment configuration
setup_environment() {
    print_header
    print_info "Environment Configuration"
    echo ""

    # Select environment
    echo "Select environment:"
    echo "  1) Development"
    echo "  2) Staging"
    echo "  3) Production"
    read -p "Enter choice [1-3]: " env_choice

    case $env_choice in
        1) ENV_NAME="development" ;;
        2) ENV_NAME="staging" ;;
        3) ENV_NAME="production" ;;
        *) ENV_NAME="development" ;;
    esac

    ENV_FILE="${PROJECT_ROOT}/.env.${ENV_NAME}"

    # Copy example if doesn't exist
    if [ ! -f "$ENV_FILE" ]; then
        cp "${PROJECT_ROOT}/.env.example" "$ENV_FILE"
        print_info "Created $ENV_FILE from template"
    fi

    print_success "Environment: $ENV_NAME"
}

# Function to configure IMAP settings
configure_imap() {
    print_header
    print_info "IMAP Configuration"
    echo ""

    echo "Select IMAP provider:"
    echo "  1) Gmail"
    echo "  2) Outlook/Office365"
    echo "  3) GoDaddy"
    echo "  4) Custom"
    echo "  5) Mock (for testing)"
    read -p "Enter choice [1-5]: " imap_choice

    case $imap_choice in
        1)
            IMAP_ADAPTER="gmail"
            IMAP_HOST="imap.gmail.com"
            IMAP_PORT="993"
            print_info "Note: Use App Password, not regular password"
            print_info "Enable 2FA and create App Password at: https://myaccount.google.com/apppasswords"
            ;;
        2)
            IMAP_ADAPTER="outlook"
            IMAP_HOST="outlook.office365.com"
            IMAP_PORT="993"
            print_info "Note: May require app-specific password"
            ;;
        3)
            IMAP_ADAPTER="godaddy"
            IMAP_HOST="imap.secureserver.net"
            IMAP_PORT="993"
            ;;
        4)
            IMAP_ADAPTER="custom"
            read -p "Enter IMAP host: " IMAP_HOST
            read -p "Enter IMAP port [993]: " IMAP_PORT
            IMAP_PORT="${IMAP_PORT:-993}"
            ;;
        5)
            IMAP_ADAPTER="mock"
            IMAP_HOST=""
            IMAP_PORT=""
            print_info "Using mock adapter for testing"
            ;;
        *)
            IMAP_ADAPTER="mock"
            ;;
    esac

    if [ "$IMAP_ADAPTER" != "mock" ]; then
        read -p "Enter email address: " IMAP_USERNAME
        read -s -p "Enter password/app password: " IMAP_PASSWORD
        echo
    fi

    # Update environment file
    sed -i.bak "s|^IMAP_ADAPTER=.*|IMAP_ADAPTER=$IMAP_ADAPTER|" "$ENV_FILE"
    if [ -n "$IMAP_HOST" ]; then
        sed -i.bak "s|^# IMAP_HOST=.*|IMAP_HOST=$IMAP_HOST|" "$ENV_FILE"
        sed -i.bak "s|^IMAP_HOST=.*|IMAP_HOST=$IMAP_HOST|" "$ENV_FILE"
    fi
    if [ -n "${IMAP_USERNAME:-}" ]; then
        sed -i.bak "s|^# IMAP_USERNAME=.*|IMAP_USERNAME=$IMAP_USERNAME|" "$ENV_FILE"
        sed -i.bak "s|^IMAP_USERNAME=.*|IMAP_USERNAME=$IMAP_USERNAME|" "$ENV_FILE"
    fi
    if [ -n "${IMAP_PASSWORD:-}" ]; then
        sed -i.bak "s|^# IMAP_PASSWORD=.*|IMAP_PASSWORD=$IMAP_PASSWORD|" "$ENV_FILE"
        sed -i.bak "s|^IMAP_PASSWORD=.*|IMAP_PASSWORD=$IMAP_PASSWORD|" "$ENV_FILE"
    fi

    print_success "IMAP configuration saved"
}

# Function to configure server settings
configure_server() {
    print_header
    print_info "Server Configuration"
    echo ""

    read -p "REST API port [9437]: " REST_PORT
    REST_PORT="${REST_PORT:-9437}"

    read -p "SSE port [9438]: " SSE_PORT
    SSE_PORT="${SSE_PORT:-9438}"

    read -p "Enable dashboard? (y/n) [y]: " DASHBOARD_ENABLED
    DASHBOARD_ENABLED="${DASHBOARD_ENABLED:-y}"

    if [[ $DASHBOARD_ENABLED =~ ^[Yy]$ ]]; then
        read -p "Dashboard port [9439]: " DASHBOARD_PORT
        DASHBOARD_PORT="${DASHBOARD_PORT:-9439}"
        DASHBOARD_ENABLED="true"
    else
        DASHBOARD_ENABLED="false"
    fi

    # Update environment file
    sed -i.bak "s|^REST_PORT=.*|REST_PORT=$REST_PORT|" "$ENV_FILE"
    sed -i.bak "s|^SSE_PORT=.*|SSE_PORT=$SSE_PORT|" "$ENV_FILE"
    sed -i.bak "s|^DASHBOARD_ENABLED=.*|DASHBOARD_ENABLED=$DASHBOARD_ENABLED|" "$ENV_FILE"
    if [ "$DASHBOARD_ENABLED" == "true" ]; then
        sed -i.bak "s|^DASHBOARD_PORT=.*|DASHBOARD_PORT=$DASHBOARD_PORT|" "$ENV_FILE"
    fi

    print_success "Server configuration saved"
}

# Function to configure security settings
configure_security() {
    print_header
    print_info "Security Configuration"
    echo ""

    read -p "Require HTTPS? (y/n) [n]: " REQUIRE_HTTPS
    REQUIRE_HTTPS="${REQUIRE_HTTPS:-n}"

    if [[ $REQUIRE_HTTPS =~ ^[Yy]$ ]]; then
        read -p "Path to TLS certificate: " TLS_CERT_PATH
        read -p "Path to TLS private key: " TLS_KEY_PATH

        # Add TLS configuration
        echo "" >> "$ENV_FILE"
        echo "# TLS Configuration" >> "$ENV_FILE"
        echo "REQUIRE_HTTPS=true" >> "$ENV_FILE"
        echo "TLS_CERT_PATH=$TLS_CERT_PATH" >> "$ENV_FILE"
        echo "TLS_KEY_PATH=$TLS_KEY_PATH" >> "$ENV_FILE"
    else
        sed -i.bak "s|^REQUIRE_HTTPS=.*|REQUIRE_HTTPS=false|" "$ENV_FILE"
    fi

    # Generate random JWT secret
    JWT_SECRET=$(openssl rand -hex 32)
    echo "JWT_SECRET=$JWT_SECRET" >> "$ENV_FILE"

    print_success "Security configuration saved"
}

# Function to test configuration
test_configuration() {
    print_header
    print_info "Testing Configuration"
    echo ""

    # Test IMAP connection
    if [ "$IMAP_ADAPTER" != "mock" ]; then
        print_info "Testing IMAP connection..."

        # Simple connection test using openssl
        if timeout 5 openssl s_client -connect "$IMAP_HOST:$IMAP_PORT" -quiet </dev/null >/dev/null 2>&1; then
            print_success "IMAP server is reachable"
        else
            print_warning "Could not connect to IMAP server"
            print_info "Please verify your IMAP settings"
        fi
    fi

    # Test Docker
    if command -v docker >/dev/null 2>&1; then
        if docker info >/dev/null 2>&1; then
            print_success "Docker is running"
        else
            print_warning "Docker is not running"
            print_info "Please start Docker daemon"
        fi
    fi

    print_success "Configuration test completed"
}

# Function to create necessary directories
create_directories() {
    print_info "Creating necessary directories..."

    local dirs=(
        "${PROJECT_ROOT}/data"
        "${PROJECT_ROOT}/logs"
        "${PROJECT_ROOT}/backups"
        "${PROJECT_ROOT}/config"
        "/tmp/rustymail"
    )

    for dir in "${dirs[@]}"; do
        if [ ! -d "$dir" ]; then
            mkdir -p "$dir"
            print_info "Created: $dir"
        fi
    done

    print_success "Directories created"
}

# Function to generate systemd service file
generate_systemd_service() {
    print_info "Generating systemd service file..."

    cat > /tmp/rustymail.service <<EOF
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

[Install]
WantedBy=multi-user.target
EOF

    print_success "Systemd service file generated at: /tmp/rustymail.service"
    print_info "To install: sudo cp /tmp/rustymail.service /etc/systemd/system/"
}

# Function to show summary
show_summary() {
    print_header
    print_success "Setup Complete!"
    echo ""
    print_info "Configuration Summary:"
    echo "  Environment: $ENV_NAME"
    echo "  Config file: $ENV_FILE"
    echo "  IMAP Provider: $IMAP_ADAPTER"
    if [ "$IMAP_ADAPTER" != "mock" ]; then
        echo "  IMAP Host: $IMAP_HOST"
        echo "  IMAP User: ${IMAP_USERNAME:-[not set]}"
    fi
    echo "  REST API Port: $REST_PORT"
    echo "  SSE Port: $SSE_PORT"
    if [ "$DASHBOARD_ENABLED" == "true" ]; then
        echo "  Dashboard Port: $DASHBOARD_PORT"
    fi
    echo ""
    print_info "Next steps:"
    echo "  1. Review configuration in $ENV_FILE"
    echo "  2. Run deployment: ./scripts/deployment/deploy.sh -e $ENV_NAME"
    echo "  3. Access the application:"
    echo "     - REST API: http://localhost:$REST_PORT"
    echo "     - SSE: http://localhost:$SSE_PORT"
    if [ "$DASHBOARD_ENABLED" == "true" ]; then
        echo "     - Dashboard: http://localhost:$DASHBOARD_PORT"
    fi
}

# Main setup flow
main() {
    print_header
    print_info "Welcome to RustyMail Setup"
    echo ""

    # Detect OS
    detect_os

    # Check and install dependencies
    install_dependencies

    # Setup environment
    setup_environment

    # Configure IMAP
    configure_imap

    # Configure server
    configure_server

    # Configure security
    configure_security

    # Create directories
    create_directories

    # Test configuration
    test_configuration

    # Generate systemd service (Linux only)
    if [ "$OS" == "linux" ]; then
        read -p "Generate systemd service file? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            generate_systemd_service
        fi
    fi

    # Show summary
    show_summary
}

# Run main function
main "$@"