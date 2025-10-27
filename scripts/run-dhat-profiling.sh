#!/bin/bash
# Run RustyMail with DHAT heap profiling enabled
# This will generate dhat-heap.json when you stop the server (Ctrl+C)

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== DHAT Heap Profiling for RustyMail ===${NC}"
echo ""
echo -e "${CYAN}Instructions:${NC}"
echo "1. The server will start with heap profiling enabled"
echo "2. Let it run for a few minutes (use the app if you want)"
echo "3. Press Ctrl+C to stop the server"
echo "4. DHAT will generate 'dhat-heap.json' in the current directory"
echo "5. Upload dhat-heap.json to https://nnethercote.github.io/dh_view/dh_view.html"
echo ""
echo -e "${YELLOW}Note: DHAT adds ~10-20% performance overhead${NC}"
echo ""
echo -e "${GREEN}Starting server...${NC}"
echo ""

# Run the profiled server
./target/release/rustymail-server

echo ""
echo -e "${GREEN}Server stopped. Checking for profile...${NC}"
echo ""

if [ -f "dhat-heap.json" ]; then
    echo -e "${GREEN}✓ Profile generated: dhat-heap.json${NC}"
    echo ""
    echo "File size: $(du -h dhat-heap.json | cut -f1)"
    echo ""
    echo -e "${CYAN}To view the report:${NC}"
    echo "1. Open: https://nnethercote.github.io/dh_view/dh_view.html"
    echo "2. Click 'Load...' and select: $(pwd)/dhat-heap.json"
    echo ""
    echo -e "${CYAN}What to look for:${NC}"
    echo "- 'At t-end' total bytes (should be small if no leaks)"
    echo "- Look for large allocations that weren't freed"
    echo "- Check backtraces to find allocation sites"
    echo ""
    echo -e "${YELLOW}Tip: You can also run 'open https://nnethercote.github.io/dh_view/dh_view.html'${NC}"
else
    echo -e "${YELLOW}⚠ No profile generated (dhat-heap.json not found)${NC}"
    echo "This can happen if the server was killed before it could write the file."
    echo "Make sure to use Ctrl+C to stop the server cleanly."
fi
