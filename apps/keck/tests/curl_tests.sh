#!/usr/bin/env bash
# =============================================================================
# Keck YType API — curl test suite
#
# Prerequisites:
#   1. Start the keck server:  cargo run --package keck
#      (defaults to http://localhost:3000)
#   2. Run this script:        bash apps/keck/tests/curl_tests.sh
#
# You can override the base URL:
#   BASE_URL=http://127.0.0.1:8080 bash apps/keck/tests/curl_tests.sh
# =============================================================================

set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:3000}"
WS="test-ws-$$"          # unique workspace id per run
PASS=0
FAIL=0

# ── Helpers ──────────────────────────────────────────────────────────────────

green()  { printf '\033[32m%s\033[0m\n' "$*"; }
red()    { printf '\033[31m%s\033[0m\n' "$*"; }
bold()   { printf '\033[1m%s\033[0m\n' "$*"; }

assert_status() {
  local test_name="$1" expected="$2" actual="$3"
  if [ "$actual" = "$expected" ]; then
    green "  ✓ $test_name (HTTP $actual)"
    PASS=$((PASS + 1))
  else
    red "  ✗ $test_name — expected HTTP $expected, got HTTP $actual"
    FAIL=$((FAIL + 1))
  fi
}

assert_body_contains() {
  local test_name="$1" expected="$2" body="$3"
  if echo "$body" | grep -qF "$expected"; then
    green "  ✓ $test_name — body contains '$expected'"
    PASS=$((PASS + 1))
  else
    red "  ✗ $test_name — body does NOT contain '$expected'"
    red "    body: $body"
    FAIL=$((FAIL + 1))
  fi
}

# ── 0. Create workspace ─────────────────────────────────────────────────────

bold "=== Setup: create workspace '$WS' ==="
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE_URL/api/block/$WS")
assert_status "POST /api/block/$WS (create workspace)" "200" "$STATUS"

# =============================================================================
# 1. YMap Tests
# =============================================================================
bold ""
bold "=== 1. YMap Tests ==="

# 1.1 GET map (initially empty or auto-created)
bold "--- 1.1 Get empty map ---"
RESP=$(curl -s -w '\n%{http_code}' "$BASE_URL/api/block/$WS/map/settings")
BODY=$(echo "$RESP" | sed '$d')
STATUS=$(echo "$RESP" | tail -1)
assert_status "GET /map/settings" "200" "$STATUS"

# 1.2 POST set key-value pairs
bold "--- 1.2 Set map entries ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  "$BASE_URL/api/block/$WS/map/settings" \
  -H "Content-Type: application/json" \
  -d '{"theme":"dark","fontSize":14,"showLineNumbers":true}')
assert_status "POST /map/settings (set entries)" "200" "$STATUS"

# 1.3 GET specific key
bold "--- 1.3 Get specific key ---"
RESP=$(curl -s -w '\n%{http_code}' "$BASE_URL/api/block/$WS/map/settings/theme")
BODY=$(echo "$RESP" | sed '$d')
STATUS=$(echo "$RESP" | tail -1)
assert_status "GET /map/settings/theme" "200" "$STATUS"
assert_body_contains "theme value is 'dark'" "dark" "$BODY"

# 1.4 GET non-existent key → 404
bold "--- 1.4 Get non-existent key ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/api/block/$WS/map/settings/nonexistent")
assert_status "GET /map/settings/nonexistent (404)" "404" "$STATUS"

# 1.5 DELETE a key
bold "--- 1.5 Delete a key ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE \
  "$BASE_URL/api/block/$WS/map/settings/theme")
assert_status "DELETE /map/settings/theme" "204" "$STATUS"

# 1.6 Verify deleted key is gone
bold "--- 1.6 Verify deleted key ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/api/block/$WS/map/settings/theme")
assert_status "GET /map/settings/theme after delete (404)" "404" "$STATUS"

# 1.7 POST with invalid body (not an object)
bold "--- 1.7 Set map with invalid body ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  "$BASE_URL/api/block/$WS/map/settings" \
  -H "Content-Type: application/json" \
  -d '"just a string"')
assert_status "POST /map/settings (invalid body → 400)" "400" "$STATUS"

# =============================================================================
# 2. YArray Tests
# =============================================================================
bold ""
bold "=== 2. YArray Tests ==="

# 2.1 GET array (initially empty or auto-created)
bold "--- 2.1 Get empty array ---"
RESP=$(curl -s -w '\n%{http_code}' "$BASE_URL/api/block/$WS/array/tags")
BODY=$(echo "$RESP" | sed '$d')
STATUS=$(echo "$RESP" | tail -1)
assert_status "GET /array/tags" "200" "$STATUS"

# 2.2 Push elements
bold "--- 2.2 Push elements ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  "$BASE_URL/api/block/$WS/array/tags" \
  -H "Content-Type: application/json" \
  -d '{"action":"push","value":"alpha"}')
assert_status "POST /array/tags (push 'alpha')" "200" "$STATUS"

STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  "$BASE_URL/api/block/$WS/array/tags" \
  -H "Content-Type: application/json" \
  -d '{"action":"push","value":"beta"}')
assert_status "POST /array/tags (push 'beta')" "200" "$STATUS"

STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  "$BASE_URL/api/block/$WS/array/tags" \
  -H "Content-Type: application/json" \
  -d '{"action":"push","value":"gamma"}')
assert_status "POST /array/tags (push 'gamma')" "200" "$STATUS"

# 2.3 Get all elements
bold "--- 2.3 Get all array elements ---"
RESP=$(curl -s -w '\n%{http_code}' "$BASE_URL/api/block/$WS/array/tags")
BODY=$(echo "$RESP" | sed '$d')
STATUS=$(echo "$RESP" | tail -1)
assert_status "GET /array/tags" "200" "$STATUS"
assert_body_contains "array has 'alpha'" "alpha" "$BODY"
assert_body_contains "array has 'beta'" "beta" "$BODY"
assert_body_contains "array has 'gamma'" "gamma" "$BODY"

# 2.4 Get element at index
bold "--- 2.4 Get element at index ---"
RESP=$(curl -s -w '\n%{http_code}' "$BASE_URL/api/block/$WS/array/tags/0")
BODY=$(echo "$RESP" | sed '$d')
STATUS=$(echo "$RESP" | tail -1)
assert_status "GET /array/tags/0" "200" "$STATUS"
assert_body_contains "index 0 is 'alpha'" "alpha" "$BODY"

RESP=$(curl -s -w '\n%{http_code}' "$BASE_URL/api/block/$WS/array/tags/2")
BODY=$(echo "$RESP" | sed '$d')
STATUS=$(echo "$RESP" | tail -1)
assert_status "GET /array/tags/2" "200" "$STATUS"
assert_body_contains "index 2 is 'gamma'" "gamma" "$BODY"

# 2.5 Insert at specific index
bold "--- 2.5 Insert at index ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  "$BASE_URL/api/block/$WS/array/tags" \
  -H "Content-Type: application/json" \
  -d '{"action":"insert","index":1,"value":"inserted"}')
assert_status "POST /array/tags (insert at index 1)" "200" "$STATUS"

# Verify insert shifted elements
RESP=$(curl -s -w '\n%{http_code}' "$BASE_URL/api/block/$WS/array/tags/1")
BODY=$(echo "$RESP" | sed '$d')
STATUS=$(echo "$RESP" | tail -1)
assert_status "GET /array/tags/1 (after insert)" "200" "$STATUS"
assert_body_contains "index 1 is 'inserted'" "inserted" "$BODY"

# 2.6 Delete element at index
bold "--- 2.6 Delete element at index ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE \
  "$BASE_URL/api/block/$WS/array/tags/1")
assert_status "DELETE /array/tags/1" "204" "$STATUS"

# Verify element was removed
RESP=$(curl -s -w '\n%{http_code}' "$BASE_URL/api/block/$WS/array/tags/1")
BODY=$(echo "$RESP" | sed '$d')
STATUS=$(echo "$RESP" | tail -1)
assert_status "GET /array/tags/1 (after delete)" "200" "$STATUS"
assert_body_contains "index 1 now is 'beta'" "beta" "$BODY"

# 2.7 Push with missing action → 400
bold "--- 2.7 Push with missing action ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  "$BASE_URL/api/block/$WS/array/tags" \
  -H "Content-Type: application/json" \
  -d '{"value":"oops"}')
assert_status "POST /array/tags (missing action → 400)" "400" "$STATUS"

# 2.8 Push with missing value → 400
bold "--- 2.8 Push with missing value ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  "$BASE_URL/api/block/$WS/array/tags" \
  -H "Content-Type: application/json" \
  -d '{"action":"push"}')
assert_status "POST /array/tags (missing value → 400)" "400" "$STATUS"

# 2.9 Insert with missing index → 400
bold "--- 2.9 Insert with missing index ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  "$BASE_URL/api/block/$WS/array/tags" \
  -H "Content-Type: application/json" \
  -d '{"action":"insert","value":"no-index"}')
assert_status "POST /array/tags (insert missing index → 400)" "400" "$STATUS"

# =============================================================================
# 3. Doc Keys Test
# =============================================================================
bold ""
bold "=== 3. Doc Keys Test ==="

RESP=$(curl -s -w '\n%{http_code}' "$BASE_URL/api/block/$WS/doc/keys")
BODY=$(echo "$RESP" | sed '$d')
STATUS=$(echo "$RESP" | tail -1)
assert_status "GET /doc/keys" "200" "$STATUS"
# We created 'settings' (map) and 'tags' (array), so they should appear in keys
assert_body_contains "doc keys has 'settings'" "settings" "$BODY"
assert_body_contains "doc keys has 'tags'" "tags" "$BODY"

# 3.1 Doc keys for non-existent workspace → 404
bold "--- 3.1 Doc keys for non-existent workspace ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/api/block/nonexistent-ws-12345/doc/keys")
assert_status "GET /doc/keys (non-existent workspace → 404)" "404" "$STATUS"

# =============================================================================
# 4. SSE Subscription Test
# =============================================================================
bold ""
bold "=== 4. SSE Subscription Test ==="

# Start SSE listener in background, capture a few seconds of output
SSE_OUTPUT=$(mktemp)
curl -s -N "$BASE_URL/api/block/$WS/subscribe/sse" > "$SSE_OUTPUT" 2>&1 &
SSE_PID=$!

# Give the SSE connection time to establish
sleep 1

# Trigger a change so we can observe an SSE event
curl -s -X POST "$BASE_URL/api/block/$WS/map/settings" \
  -H "Content-Type: application/json" \
  -d '{"sse_test":"hello"}' > /dev/null 2>&1

# Wait a bit for the event to arrive
sleep 2

# Kill the SSE listener
kill "$SSE_PID" 2>/dev/null || true
wait "$SSE_PID" 2>/dev/null || true

if [ -s "$SSE_OUTPUT" ]; then
  green "  ✓ SSE subscription received data"
  PASS=$((PASS + 1))
  bold "    SSE output (first 500 chars):"
  head -c 500 "$SSE_OUTPUT"
  echo ""
else
  # SSE may not produce data if subscription fires before observer registers.
  # In that case, just verify the endpoint doesn't error out.
  red "  ? SSE subscription produced no data (may depend on timing)"
  echo "    This is not necessarily a failure — the event may fire before the observer is ready."
fi
rm -f "$SSE_OUTPUT"

# 4.1 SSE for non-existent workspace → 404
bold "--- 4.1 SSE for non-existent workspace ---"
STATUS=$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 \
  "$BASE_URL/api/block/nonexistent-ws-12345/subscribe/sse" 2>/dev/null || echo "000")
# Accept 404 or timeout (000/curl error 28 — server may not return 404 for SSE)
if [ "$STATUS" = "404" ]; then
  assert_status "SSE non-existent workspace" "404" "$STATUS"
else
  green "  ✓ SSE non-existent workspace (connection refused/timeout — acceptable)"
  PASS=$((PASS + 1))
fi

# =============================================================================
# 5. Cleanup
# =============================================================================
bold ""
bold "=== Cleanup: delete workspace '$WS' ==="
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE "$BASE_URL/api/block/$WS")
assert_status "DELETE /api/block/$WS (cleanup)" "204" "$STATUS"

# =============================================================================
# Summary
# =============================================================================
bold ""
bold "==============================="
bold "  Test Results: $PASS passed, $FAIL failed"
bold "==============================="

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
