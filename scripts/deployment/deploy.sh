#!/usr/bin/env bash

# RustyMail Deployment Script
# This script handles deployment of RustyMail across different environments

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_FILE="/tmp/rustymail_deploy_${TIMESTAMP}.log"

# Default values
ENVIRONMENT="production"
DEPLOYMENT_METHOD="docker"
BACKUP_ENABLED=true
HEALTH_CHECK_ENABLED=true
ROLLBACK_ON_FAILURE=true

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1" | tee -a "$LOG_FILE"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1" | tee -a "$LOG_FILE"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1" | tee -a "$LOG_FILE"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1" | tee -a "$LOG_FILE"
}

# Function to check prerequisites
check_prerequisites() {
    print_info "Checking prerequisites..."

    local missing_deps=()

    # Check for required commands
    command -v docker >/dev/null 2>&1 || missing_deps+=("docker")
    command -v docker-compose >/dev/null 2>&1 || missing_deps+=("docker-compose")
    command -v curl >/dev/null 2>&1 || missing_deps+=("curl")
    command -v jq >/dev/null 2>&1 || missing_deps+=("jq")

    if [ ${#missing_deps[@]} -ne 0 ]; then
        print_error "Missing dependencies: ${missing_deps[*]}"
        print_info "Please install missing dependencies and try again."
        exit 1
    fi

    # Check Docker daemon
    if ! docker info >/dev/null 2>&1; then
        print_error "Docker daemon is not running"
        exit 1
    fi

    print_success "All prerequisites met"
}

# Function to validate environment configuration
validate_config() {
    print_info "Validating configuration..."

    local env_file="${PROJECT_ROOT}/.env.${ENVIRONMENT}"

    if [ ! -f "$env_file" ]; then
        print_warning "Environment file not found: $env_file"
        print_info "Using .env.example as template"
        cp "${PROJECT_ROOT}/.env.example" "$env_file"
    fi

    # Check required variables
    local required_vars=("IMAP_HOST" "IMAP_USERNAME" "IMAP_PASSWORD")
    local missing_vars=()

    for var in "${required_vars[@]}"; do
        if ! grep -q "^${var}=" "$env_file"; then
            missing_vars+=("$var")
        fi
    done

    if [ ${#missing_vars[@]} -ne 0 ]; then
        print_warning "Missing required variables: ${missing_vars[*]}"
        print_info "Please configure these in $env_file"
        return 1
    fi

    print_success "Configuration validated"
}

# Function to backup existing deployment
backup_deployment() {
    if [ "$BACKUP_ENABLED" != true ]; then
        print_info "Backup skipped (disabled)"
        return
    fi

    print_info "Creating backup..."

    local backup_dir="${PROJECT_ROOT}/backups/${TIMESTAMP}"
    mkdir -p "$backup_dir"

    # Backup data volumes
    if docker volume ls | grep -q rustymail_data; then
        docker run --rm \
            -v rustymail_data:/data \
            -v "$backup_dir":/backup \
            alpine tar czf /backup/data.tar.gz -C /data . 2>/dev/null || true
    fi

    # Backup configuration
    cp -r "${PROJECT_ROOT}/.env"* "$backup_dir/" 2>/dev/null || true

    print_success "Backup created at: $backup_dir"
}

# Function to build Docker image
build_image() {
    print_info "Building Docker image..."

    cd "$PROJECT_ROOT"

    # Build with BuildKit for better performance
    DOCKER_BUILDKIT=1 docker build \
        --tag "rustymail:${TIMESTAMP}" \
        --tag "rustymail:latest" \
        --build-arg "BUILD_VERSION=${TIMESTAMP}" \
        --progress=plain \
        . 2>&1 | tee -a "$LOG_FILE"

    if [ $? -ne 0 ]; then
        print_error "Docker build failed"
        return 1
    fi

    print_success "Docker image built: rustymail:${TIMESTAMP}"
}

# Function to deploy with Docker Compose
deploy_docker_compose() {
    print_info "Deploying with Docker Compose..."

    cd "$PROJECT_ROOT"

    # Use appropriate compose file
    local compose_file="docker-compose.${ENVIRONMENT}.yml"
    if [ ! -f "$compose_file" ]; then
        compose_file="docker-compose.prod.yml"
    fi

    # Pull latest images (if using external services)
    docker-compose -f "$compose_file" pull 2>/dev/null || true

    # Deploy services
    docker-compose -f "$compose_file" \
        --env-file ".env.${ENVIRONMENT}" \
        up -d --remove-orphans 2>&1 | tee -a "$LOG_FILE"

    if [ $? -ne 0 ]; then
        print_error "Docker Compose deployment failed"
        return 1
    fi

    print_success "Services deployed with Docker Compose"
}

# Function to deploy standalone binary
deploy_standalone() {
    print_info "Deploying standalone binary..."

    # Build binary if not exists
    if [ ! -f "${PROJECT_ROOT}/target/release/rustymail-server" ]; then
        print_info "Building release binary..."
        cd "$PROJECT_ROOT"
        cargo build --release --bin rustymail-server
    fi

    # Stop existing service
    sudo systemctl stop rustymail 2>/dev/null || true

    # Copy binary
    sudo cp "${PROJECT_ROOT}/target/release/rustymail-server" /opt/rustymail/
    sudo chmod +x /opt/rustymail/rustymail-server

    # Copy configuration
    sudo cp "${PROJECT_ROOT}/.env.${ENVIRONMENT}" /etc/rustymail/rustymail.env

    # Start service
    sudo systemctl start rustymail

    if ! sudo systemctl is-active --quiet rustymail; then
        print_error "Failed to start rustymail service"
        return 1
    fi

    print_success "Standalone binary deployed"
}

# Function to deploy to Kubernetes
deploy_kubernetes() {
    print_info "Deploying to Kubernetes..."

    cd "$PROJECT_ROOT"

    # Check kubectl configuration
    if ! kubectl cluster-info >/dev/null 2>&1; then
        print_error "kubectl not configured or cluster not accessible"
        return 1
    fi

    # Create namespace if not exists
    kubectl create namespace rustymail 2>/dev/null || true

    # Apply configurations
    kubectl apply -f k8s/ -n rustymail

    # Wait for rollout
    kubectl rollout status deployment/rustymail -n rustymail --timeout=300s

    if [ $? -ne 0 ]; then
        print_error "Kubernetes deployment failed"
        return 1
    fi

    print_success "Deployed to Kubernetes"
}

# Function to perform health check
health_check() {
    if [ "$HEALTH_CHECK_ENABLED" != true ]; then
        print_info "Health check skipped (disabled)"
        return 0
    fi

    print_info "Performing health check..."

    local max_attempts=30
    local attempt=1
    local health_url="http://localhost:9437/health"

    while [ $attempt -le $max_attempts ]; do
        if curl -s "$health_url" | jq -e '.status == "healthy"' >/dev/null 2>&1; then
            print_success "Health check passed"
            return 0
        fi

        print_info "Waiting for service to be healthy... (${attempt}/${max_attempts})"
        sleep 5
        ((attempt++))
    done

    print_error "Health check failed after ${max_attempts} attempts"
    return 1
}

# Function to rollback deployment
rollback() {
    if [ "$ROLLBACK_ON_FAILURE" != true ]; then
        print_warning "Rollback disabled"
        return
    fi

    print_warning "Rolling back deployment..."

    case "$DEPLOYMENT_METHOD" in
        docker)
            # Find previous working image
            local previous_image=$(docker images rustymail --format "{{.Tag}}" | grep -v latest | head -n 2 | tail -n 1)
            if [ -n "$previous_image" ]; then
                docker tag "rustymail:${previous_image}" rustymail:latest
                deploy_docker_compose
            fi
            ;;
        kubernetes)
            kubectl rollout undo deployment/rustymail -n rustymail
            ;;
        standalone)
            if [ -f /opt/rustymail/rustymail-server.backup ]; then
                sudo mv /opt/rustymail/rustymail-server.backup /opt/rustymail/rustymail-server
                sudo systemctl restart rustymail
            fi
            ;;
    esac

    print_info "Rollback completed"
}

# Function to display deployment info
display_info() {
    print_info "Deployment Information:"
    echo "========================" | tee -a "$LOG_FILE"
    echo "Environment: $ENVIRONMENT" | tee -a "$LOG_FILE"
    echo "Method: $DEPLOYMENT_METHOD" | tee -a "$LOG_FILE"
    echo "Timestamp: $TIMESTAMP" | tee -a "$LOG_FILE"
    echo "Log file: $LOG_FILE" | tee -a "$LOG_FILE"

    case "$DEPLOYMENT_METHOD" in
        docker)
            echo "" | tee -a "$LOG_FILE"
            docker ps --filter "label=com.docker.compose.project=rustymail" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
            ;;
        kubernetes)
            echo "" | tee -a "$LOG_FILE"
            kubectl get pods -n rustymail
            ;;
        standalone)
            echo "" | tee -a "$LOG_FILE"
            sudo systemctl status rustymail --no-pager
            ;;
    esac

    echo "" | tee -a "$LOG_FILE"
    print_success "Deployment completed successfully!"
    print_info "Access the application at:"
    echo "  REST API: http://localhost:9437" | tee -a "$LOG_FILE"
    echo "  SSE: http://localhost:9438" | tee -a "$LOG_FILE"
    echo "  Dashboard: http://localhost:9439" | tee -a "$LOG_FILE"
}

# Function to show usage
usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Deploy RustyMail application

OPTIONS:
    -e, --environment ENV     Environment to deploy (development|staging|production) [default: production]
    -m, --method METHOD       Deployment method (docker|kubernetes|standalone) [default: docker]
    -b, --no-backup          Skip backup before deployment
    -h, --no-health-check    Skip health check after deployment
    -r, --no-rollback        Disable automatic rollback on failure
    --build-only             Build Docker image only, don't deploy
    --dry-run                Show what would be deployed without doing it
    --help                   Display this help message

EXAMPLES:
    $0                                    # Deploy to production using Docker
    $0 -e development                     # Deploy to development environment
    $0 -m kubernetes                      # Deploy using Kubernetes
    $0 -e staging -m docker --no-backup   # Deploy to staging without backup

EOF
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -e|--environment)
                ENVIRONMENT="$2"
                shift 2
                ;;
            -m|--method)
                DEPLOYMENT_METHOD="$2"
                shift 2
                ;;
            -b|--no-backup)
                BACKUP_ENABLED=false
                shift
                ;;
            -h|--no-health-check)
                HEALTH_CHECK_ENABLED=false
                shift
                ;;
            -r|--no-rollback)
                ROLLBACK_ON_FAILURE=false
                shift
                ;;
            --build-only)
                BUILD_ONLY=true
                shift
                ;;
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --help)
                usage
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done
}

# Main deployment flow
main() {
    print_info "Starting RustyMail deployment..."
    print_info "Log file: $LOG_FILE"

    # Parse arguments
    parse_args "$@"

    # Dry run mode
    if [ "${DRY_RUN:-false}" == true ]; then
        print_info "DRY RUN MODE - No changes will be made"
        print_info "Would deploy to: $ENVIRONMENT using $DEPLOYMENT_METHOD"
        exit 0
    fi

    # Check prerequisites
    check_prerequisites

    # Validate configuration
    if ! validate_config; then
        print_error "Configuration validation failed"
        exit 1
    fi

    # Create backup
    backup_deployment

    # Build if needed
    if [ "$DEPLOYMENT_METHOD" == "docker" ] || [ "${BUILD_ONLY:-false}" == true ]; then
        if ! build_image; then
            print_error "Build failed"
            exit 1
        fi

        if [ "${BUILD_ONLY:-false}" == true ]; then
            print_success "Build completed successfully"
            exit 0
        fi
    fi

    # Deploy based on method
    case "$DEPLOYMENT_METHOD" in
        docker)
            if ! deploy_docker_compose; then
                rollback
                exit 1
            fi
            ;;
        kubernetes)
            if ! deploy_kubernetes; then
                rollback
                exit 1
            fi
            ;;
        standalone)
            if ! deploy_standalone; then
                rollback
                exit 1
            fi
            ;;
        *)
            print_error "Invalid deployment method: $DEPLOYMENT_METHOD"
            usage
            exit 1
            ;;
    esac

    # Health check
    if ! health_check; then
        print_error "Health check failed"
        rollback
        exit 1
    fi

    # Display deployment information
    display_info
}

# Error handler
trap 'print_error "Deployment failed on line $LINENO"' ERR

# Run main function
main "$@"