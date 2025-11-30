#!/bin/bash
set -ex

echo "Stopping rustymail-backend..."
pm2 stop ecosystem.config.js --only rustymail-backend || true
pm2 delete rustymail-backend || true

echo "Rebuilding server (release)..."
cargo build --release --bin rustymail-server

echo "Starting rustymail-backend with pm2..."
pm2 startOrRestart ecosystem.config.js --only rustymail-backend
