#!/bin/bash
set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

LOGS_DIR="$PROJECT_ROOT/logs/memory-profiles"
mkdir -p "$LOGS_DIR"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)

echo -e "${GREEN}=== RustyMail Backend Restart with Memory Analysis ===${NC}"
echo ""

# Check if the server is currently running and analyze memory before killing
PID=$(pgrep -f "rustymail-server" | head -1)

if [ -n "$PID" ]; then
    echo -e "${CYAN}Found running server (PID: $PID). Analyzing memory before shutdown...${NC}"

    # Get memory stats before shutdown
    MEMORY_BEFORE=$(ps -o rss= -p $PID 2>/dev/null | awk '{printf "%.2f", $1/1024}')
    echo "Current memory usage: ${MEMORY_BEFORE} MB"

    # Run leaks analysis
    LEAKS_FILE="$LOGS_DIR/leaks-$TIMESTAMP.txt"
    echo -e "${CYAN}Running leaks analysis...${NC}"
    if leaks $PID > "$LEAKS_FILE" 2>&1; then
        LEAKS_COUNT=$(grep -c "Leak:" "$LEAKS_FILE" 2>/dev/null || echo "0")
        if [ "$LEAKS_COUNT" -gt 0 ]; then
            echo -e "${RED}WARNING: Found $LEAKS_COUNT memory leaks!${NC}"
            echo "Full report saved to: $LEAKS_FILE"
            # Show summary
            echo ""
            echo -e "${YELLOW}Leak summary:${NC}"
            grep -A 2 "^Process" "$LEAKS_FILE" | head -5 || true
        else
            echo -e "${GREEN}No memory leaks detected.${NC}"
            rm -f "$LEAKS_FILE"  # Remove empty report
        fi
    else
        echo -e "${YELLOW}Note: Could not run leaks analysis (may need sudo or SIP disabled)${NC}"
    fi

    # Get vmmap summary
    VMMAP_FILE="$LOGS_DIR/vmmap-$TIMESTAMP.txt"
    echo -e "${CYAN}Capturing memory map summary...${NC}"
    vmmap --summary $PID > "$VMMAP_FILE" 2>/dev/null || true
    if [ -s "$VMMAP_FILE" ]; then
        echo "Memory map saved to: $VMMAP_FILE"
    fi

    echo ""
fi

echo -e "${CYAN}Stopping rustymail-backend...${NC}"
pm2 stop ecosystem.config.js --only rustymail-backend 2>/dev/null || true
pm2 delete rustymail-backend 2>/dev/null || true

# Also kill any orphaned processes
pkill -f "rustymail-server" 2>/dev/null || true
sleep 1

echo -e "${CYAN}Rebuilding server (release)...${NC}"
cargo build --release --bin rustymail-server

echo -e "${CYAN}Starting rustymail-backend with pm2...${NC}"
pm2 startOrRestart ecosystem.config.js --only rustymail-backend

# Wait for server to start
sleep 2

# Verify server started and show new PID
NEW_PID=$(pgrep -f "rustymail-server" | head -1)
if [ -n "$NEW_PID" ]; then
    NEW_MEMORY=$(ps -o rss= -p $NEW_PID 2>/dev/null | awk '{printf "%.2f", $1/1024}')
    echo ""
    echo -e "${GREEN}Server restarted successfully!${NC}"
    echo "New PID: $NEW_PID"
    echo "Initial memory: ${NEW_MEMORY} MB"

    if [ -n "$MEMORY_BEFORE" ]; then
        echo ""
        echo -e "${CYAN}Memory comparison:${NC}"
        echo "  Before restart: ${MEMORY_BEFORE} MB"
        echo "  After restart:  ${NEW_MEMORY} MB"
    fi
else
    echo -e "${RED}WARNING: Server may not have started properly${NC}"
    pm2 logs rustymail-backend --lines 10
fi

echo ""
echo -e "${GREEN}Done!${NC}"
echo "Memory profile logs: $LOGS_DIR"
