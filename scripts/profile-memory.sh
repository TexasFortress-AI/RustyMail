#!/bin/bash
# Memory profiling script for RustyMail
# Works with macOS without requiring cargo-instruments

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== RustyMail Memory Profiling Tool ===${NC}"
echo ""

# Check if Xcode Instruments is available
if command -v instruments &> /dev/null; then
    HAS_INSTRUMENTS=true
else
    HAS_INSTRUMENTS=false
fi

# Show options
echo "Available profiling methods:"
echo ""
echo "1. Simple Memory Monitor (watch memory usage in real-time)"
echo "2. Extended Monitor (track memory over 30 minutes)"
echo "3. macOS Instruments - Allocations (requires Xcode)"
echo "4. macOS Instruments - Leaks (requires Xcode)"
echo "5. Process Memory Map (detailed memory breakdown)"
echo ""

# Default to simple monitor if no argument
METHOD=${1:-1}

case $METHOD in
    1)
        echo -e "${GREEN}Starting Simple Memory Monitor...${NC}"
        echo "Press Ctrl+C to stop"
        echo ""
        watch -n 2 'ps aux | grep rustymail-server | grep -v grep | awk "{printf \"PID: %s | Memory: %.2f MB | CPU: %s%%\\n\", \$2, \$6/1024, \$3}"'
        ;;

    2)
        echo -e "${GREEN}Starting Extended Memory Monitor (30 minutes)...${NC}"
        echo "Tracking memory usage every 60 seconds"
        echo "Output will be saved to logs/memory-profile-$(date +%Y%m%d-%H%M%S).log"
        echo ""

        LOG_FILE="logs/memory-profile-$(date +%Y%m%d-%H%M%S).log"
        mkdir -p logs

        echo "Timestamp,PID,Memory_MB,CPU_Percent,VSZ,RSS" > "$LOG_FILE"

        for i in {1..30}; do
            TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
            STATS=$(ps aux | grep rustymail-server | grep -v grep | awk '{print $2","$6/1024","$3","$5","$6}')
            if [ -n "$STATS" ]; then
                echo "$TIMESTAMP,$STATS" >> "$LOG_FILE"
                echo "[$i/30] $TIMESTAMP - Memory: $(echo $STATS | cut -d',' -f2) MB"
            else
                echo "[$i/30] $TIMESTAMP - Process not found"
            fi
            sleep 60
        done

        echo ""
        echo -e "${GREEN}Profiling complete!${NC}"
        echo "Results saved to: $LOG_FILE"
        echo ""
        echo "Memory statistics:"
        tail -n 30 "$LOG_FILE" | awk -F',' 'NR>1 {sum+=$3; count++} END {print "Average Memory: " sum/count " MB"}'
        tail -n 30 "$LOG_FILE" | awk -F',' 'NR>1 {if($3>max) max=$3} END {print "Peak Memory: " max " MB"}'
        tail -n 30 "$LOG_FILE" | awk -F',' 'NR==2 {start=$3} END {print "Memory Growth: " ($3-start) " MB"}'
        ;;

    3)
        if [ "$HAS_INSTRUMENTS" = false ]; then
            echo -e "${RED}Error: Xcode Instruments not found${NC}"
            echo "Install Xcode Command Line Tools: xcode-select --install"
            exit 1
        fi

        echo -e "${GREEN}Starting Allocations profiling with Instruments...${NC}"
        echo ""

        # Stop PM2 process temporarily
        pm2 stop rustymail-backend 2>/dev/null || true

        # Build with debug symbols for better profiling
        cargo build --release --bin rustymail-server

        echo "Starting Instruments Allocations template..."
        echo "This will open Xcode Instruments. Profile for a few minutes, then save the trace."

        instruments -t Allocations ./target/release/rustymail-server

        # Restart PM2
        pm2 start rustymail-backend
        ;;

    4)
        if [ "$HAS_INSTRUMENTS" = false ]; then
            echo -e "${RED}Error: Xcode Instruments not found${NC}"
            echo "Install Xcode Command Line Tools: xcode-select --install"
            exit 1
        fi

        echo -e "${GREEN}Starting Leaks profiling with Instruments...${NC}"
        echo ""

        # Stop PM2 process temporarily
        pm2 stop rustymail-backend 2>/dev/null || true

        # Build with debug symbols
        cargo build --release --bin rustymail-server

        echo "Starting Instruments Leaks template..."
        echo "This will open Xcode Instruments. Profile for a few minutes, then save the trace."

        instruments -t Leaks ./target/release/rustymail-server

        # Restart PM2
        pm2 start rustymail-backend
        ;;

    5)
        echo -e "${GREEN}Process Memory Map Analysis...${NC}"
        echo ""

        PID=$(pgrep -f rustymail-server | head -1)

        if [ -z "$PID" ]; then
            echo -e "${RED}Error: rustymail-server not running${NC}"
            exit 1
        fi

        echo "Process ID: $PID"
        echo ""
        echo "Memory Regions:"
        vmmap $PID | head -50
        echo ""
        echo "Memory Summary:"
        vmmap --summary $PID
        ;;

    *)
        echo -e "${RED}Invalid option: $METHOD${NC}"
        echo "Usage: $0 [1-5]"
        exit 1
        ;;
esac
