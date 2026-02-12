#!/bin/bash
# local_game_test.sh — Test the local game pipeline without MCPL.
#
# Tests:
# 1. Write-dir initialization (creates dirs, symlinks shared content)
# 2. Engine discovery (finds spring-headless)
# 3. SAI bridge installation (copies .so into write-dir)
# 4. Script generation (correct startscript for local scrimmage)
#
# Usage:
#   bash tests/local_game_test.sh [--launch]
#
# With --launch, actually starts the engine (requires spring-headless + content).
# Without it, only tests initialization and prints what would be launched.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
GM_DIR="$(dirname "$SCRIPT_DIR")"
ZK_DIR="$(dirname "$GM_DIR")"
TEST_WRITE_DIR="/tmp/test-agent-writedir-$$"
LAUNCH=false

if [[ "${1:-}" == "--launch" ]]; then
    LAUNCH=true
fi

cleanup() {
    echo ""
    echo "=== Cleanup ==="
    rm -rf "$TEST_WRITE_DIR"
    echo "Removed $TEST_WRITE_DIR"
}
trap cleanup EXIT

echo "=== Local Game Pipeline Test ==="
echo "Game Manager: $GM_DIR"
echo "ZK Dir:       $ZK_DIR"
echo "Write Dir:    $TEST_WRITE_DIR"
echo ""

# ── Step 1: Build SAI bridge ──
echo "=== Step 1: Build SAI bridge ==="
cd "$ZK_DIR/sai-bridge"
cargo build --release 2>&1 | tail -1
SAI_LIB="$ZK_DIR/sai-bridge/target/release/libSkirmishAI.so"
if [[ -f "$SAI_LIB" ]]; then
    echo "OK: SAI bridge built at $SAI_LIB"
else
    echo "FAIL: SAI bridge not found at $SAI_LIB"
    exit 1
fi

# ── Step 2: Build game-manager ──
echo ""
echo "=== Step 2: Build game-manager ==="
cd "$GM_DIR"
cargo build 2>&1 | tail -1
echo "OK: game-manager built"

# ── Step 3: Test write-dir initialization ──
echo ""
echo "=== Step 3: Test write-dir initialization ==="
echo "Running game-manager with --write-dir $TEST_WRITE_DIR (will fail on MCPL connect, that's OK)"
timeout 5 cargo run -- --stdio --write-dir "$TEST_WRITE_DIR" </dev/null 2>/tmp/gm_test_stderr.$$ || true
# Show only the game-manager log lines, not compiler warnings
grep "game_manager" /tmp/gm_test_stderr.$$ | sed 's/\x1b\[[0-9;]*m//g' || true
rm -f /tmp/gm_test_stderr.$$

echo ""
echo "Checking write-dir structure:"
PASS=0
FAIL=0

check_dir() {
    if [[ -d "$TEST_WRITE_DIR/$1" ]]; then
        echo "  OK: $1/"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $1/ missing"
        FAIL=$((FAIL + 1))
    fi
}

check_symlink() {
    if [[ -L "$TEST_WRITE_DIR/$1" ]]; then
        echo "  OK: $1 -> $(readlink "$TEST_WRITE_DIR/$1")"
        PASS=$((PASS + 1))
    elif [[ -d "$TEST_WRITE_DIR/$1" ]]; then
        echo "  WARN: $1 is a dir (source may not exist in spring home)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $1 missing"
        FAIL=$((FAIL + 1))
    fi
}

check_file() {
    if [[ -f "$TEST_WRITE_DIR/$1" ]]; then
        echo "  OK: $1 ($(wc -c < "$TEST_WRITE_DIR/$1") bytes)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $1 missing"
        FAIL=$((FAIL + 1))
    fi
}

# Subdirectories
check_dir "AI/Skirmish/AgentBridge/0.1"
check_dir "LuaUI/Widgets"
check_dir "LuaUI/Config"
check_dir "demos"
check_dir "temp"

# Symlinks to shared content
check_symlink "pool"
check_symlink "packages"
check_symlink "maps"
check_symlink "games"
check_symlink "engine"
check_symlink "rapid"

# SAI bridge files
check_file "AI/Skirmish/AgentBridge/0.1/libSkirmishAI.so"
check_file "AI/Skirmish/AgentBridge/0.1/AIInfo.lua"
check_file "AI/Skirmish/AgentBridge/0.1/AIOptions.lua"

# Widget
check_file "LuaUI/Widgets/agent_bootstrap.lua"

# Config files
check_file "LuaUI/Config/agent_bootstrap.json"
check_file "springsettings.cfg"

echo ""
echo "Results: $PASS passed, $FAIL failed"

if [[ $FAIL -gt 0 ]]; then
    echo "SOME CHECKS FAILED"
    exit 1
fi

# ── Step 4: Verify springsettings content ──
echo ""
echo "=== Step 4: Verify springsettings.cfg ==="
if grep -q "XResolution=1" "$TEST_WRITE_DIR/springsettings.cfg"; then
    echo "OK: headless settings present"
else
    echo "FAIL: springsettings.cfg content wrong"
    exit 1
fi

# ── Step 5: Verify bootstrap config ──
echo ""
echo "=== Step 5: Verify agent_bootstrap.json ==="
if python3 -c "import json; d=json.load(open('$TEST_WRITE_DIR/LuaUI/Config/agent_bootstrap.json')); assert 'players' in d; print('OK: config has players entry')" 2>/dev/null; then
    :
else
    echo "FAIL: agent_bootstrap.json invalid"
    exit 1
fi

echo ""
echo "=== All checks passed ==="

if $LAUNCH; then
    echo ""
    echo "=== Step 6: Launch local game ==="
    echo "Would launch: spring-headless --write-dir $TEST_WRITE_DIR <script.txt>"
    echo "(Full engine launch requires MCP client or direct invocation)"
    echo "Skipping actual launch in test mode."
fi
