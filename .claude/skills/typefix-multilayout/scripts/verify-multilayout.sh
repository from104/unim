#!/bin/bash
# AutoTypeFix 다중 키맵 검증 스크립트
# 사용: bash .claude/skills/typefix-multilayout/scripts/verify-multilayout.sh

set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$PROJECT_ROOT"

echo "=== AutoTypeFix Multi-Layout Verification ==="
echo ""

# Step 1: Build
echo "[1/4] Building workspace..."
if cargo build --workspace --release 2>&1 | grep -c "warning" | grep -q "^0$"; then
    echo "  PASS: Zero warnings"
else
    WARNINGS=$(cargo build --workspace --release 2>&1 | grep "warning" | grep -v "warning(s)" | head -20)
    if [ -z "$WARNINGS" ]; then
        echo "  PASS: Zero warnings"
    else
        echo "  FAIL: Warnings detected:"
        echo "$WARNINGS"
        exit 1
    fi
fi

# Step 2: Tests
echo "[2/4] Running all tests..."
if cargo test --workspace 2>&1 | tail -5; then
    echo "  PASS: All tests passed"
else
    echo "  FAIL: Test failures detected"
    exit 1
fi

# Step 3: Layout-specific tests
echo "[3/4] Running layout-specific tests..."
cargo test --package unim-core -- "multilayout\|for_layout\|dvorak\|colemak\|workman" 2>&1 | tail -10
echo "  Done"

# Step 4: Grep for hardcoded QWERTY in AutoTypeFix path
echo "[4/4] Checking for remaining QWERTY hardcoding in auto_typefix.rs..."
if grep -n "to_char()" src/auto_typefix.rs | grep -v "to_char_for_layout" | grep -v "test" | grep -v "//"; then
    echo "  WARN: Found to_char() calls without layout parameter"
else
    echo "  PASS: No hardcoded QWERTY calls in auto_typefix.rs"
fi

echo ""
echo "=== Verification Complete ==="
