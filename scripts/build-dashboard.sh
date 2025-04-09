#!/bin/bash
# build-dashboard.sh
# Script to build the frontend dashboard and configure it for use with RustyMail

# Exit on any error
set -e

echo "RustyMail Dashboard Build Script"
echo "================================"

# Check if npm is installed
if ! command -v npm &> /dev/null; then
    echo "Error: npm is not installed. Please install Node.js and npm first."
    exit 1
fi

# Navigate to the frontend directory
echo "Building frontend application..."
cd "$(dirname "$0")/../frontend/rustymail-app-main"

# Install dependencies
echo "Installing npm dependencies..."
npm install

# Build the frontend
echo "Building the application..."
npm run build

# Create dashboard directory if it doesn't exist
echo "Setting up dashboard-static directory..."
mkdir -p "../../dashboard-static"

# Copy built files to dashboard-static directory
echo "Copying built files to dashboard-static directory..."
cp -r dist/* ../../dashboard-static/

# Update .env file to include dashboard configuration if not already present
cd "../../"
if ! grep -q "DASHBOARD_ENABLED" .env; then
    echo "Updating .env file with dashboard configuration..."
    echo "" >> .env
    echo "# Dashboard configuration" >> .env
    echo "DASHBOARD_ENABLED=true" >> .env
    echo "DASHBOARD_PATH=./dashboard-static" >> .env
fi

echo "Dashboard build complete!"
echo "To run the server with the dashboard enabled, use:"
echo "cargo run --bin rustymail-server"
echo ""
echo "Then visit: http://localhost:3000/dashboard" 