#!/usr/bin/env bash

set -euo pipefail

test_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
loop="$test_dir/../loop.sh"

# shellcheck source=../loop.sh
source "$loop"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

tests=0

pass() {
  tests=$((tests + 1))
  printf 'ok %d - %s\n' "$tests" "$1"
}

fail() {
  printf 'not ok %d - %s\n' "$((tests + 1))" "$1" >&2
  exit 1
}

assert_eq() {
  local expected="$1" actual="$2" label="$3"
  [[ "$actual" == "$expected" ]] || fail "$label (expected '$expected', got '$actual')"
  pass "$label"
}

valid="$tmp/valid.json"
invalid="$tmp/invalid.json"
invalid_proposed="$tmp/invalid-proposed.json"
printf '%s\n' '{"status":"PROPOSED","issue_number":239,"pr_url":"https://github.com/ITmedes/browserpane/pull/999","commit_sha":"abc1234","run_id":null,"reason":null,"summary":"opened"}' > "$valid"
printf '%s\n' '{"status":"UNKNOWN","summary":"bad"}' > "$invalid"
printf '%s\n' '{"status":"PROPOSED","issue_number":239,"pr_url":null,"commit_sha":null,"run_id":null,"reason":null,"summary":"incomplete"}' > "$invalid_proposed"

validate_result "$valid" || fail "valid structured result"
pass "valid structured result"

if validate_result "$invalid" >/dev/null 2>&1; then
  fail "invalid structured result is rejected"
fi
pass "invalid structured result is rejected"

if validate_result "$invalid_proposed" >/dev/null 2>&1; then
  fail "status-specific result requirements are enforced"
fi
pass "status-specific result requirements are enforced"

assert_eq "PROPOSED" "$(routine_field "$valid" status)" "routine status extraction"
assert_eq "239" "$(routine_field "$valid" issue_number)" "routine issue extraction"

jsonl="$tmp/session.jsonl"
printf '%s\n' \
  '{"type":"thread.started","thread_id":"thread-1"}' \
  '{"type":"turn.completed","usage":{"input_tokens":120,"cached_input_tokens":80,"output_tokens":45,"reasoning_output_tokens":12}}' \
  > "$jsonl"
assert_eq $'120\t80\t45\t12' "$(session_meta "$jsonl")" "Codex usage extraction"

ids="$(printf '%s\n' \
  'https://github.com/ITmedes/browserpane/actions/runs/123/job/1' \
  'https://github.com/ITmedes/browserpane/actions/runs/123/job/2' \
  'https://github.com/ITmedes/browserpane/actions/runs/456' \
  'https://example.invalid/not-a-run' | run_ids_from_links)"
assert_eq $'123\n456' "$ids" "unique Actions run id extraction"

original_stop="$STOP_FILE"
STOP_FILE="$tmp/STOP"
touch "$STOP_FILE"
stop_requested >/dev/null || fail "STOP is detected"
[[ ! -e "$STOP_FILE" ]] || fail "STOP is consumed"
pass "STOP is detected and consumed"
STOP_FILE="$original_stop"

if AUTO_MERGE=invalid bash -c "source '$loop'; validate_config" >/dev/null 2>&1; then
  fail "invalid boolean configuration is rejected"
fi
pass "invalid boolean configuration is rejected"

if MERGE_METHOD=octopus bash -c "source '$loop'; validate_config" >/dev/null 2>&1; then
  fail "invalid merge method is rejected"
fi
pass "invalid merge method is rejected"

ALLOWED_GITHUB_LOGINS=thebackplane
if github_identity_allowed personal-user >/dev/null 2>&1; then
  fail "unapproved GitHub identity is rejected"
fi
pass "unapproved GitHub identity is rejected"

github_identity_allowed thebackplane || fail "approved GitHub identity is accepted"
pass "approved GitHub identity is accepted"

github_permission_allowed ADMIN || fail "admin repository permission is accepted"
pass "admin repository permission is accepted"

if github_permission_allowed READ; then
  fail "read-only repository permission is rejected"
fi
pass "read-only repository permission is rejected"

jq -e '.properties.status.enum | index("PROPOSED") and index("REPAIRED") and index("HALT")' "$RESULT_SCHEMA" >/dev/null \
  || fail "result schema status contract"
pass "result schema status contract"

MOCK_GH_JSON='[]'
MOCK_GH_RC=0
gh() {
  printf '%s\n' "$MOCK_GH_JSON"
  return "$MOCK_GH_RC"
}

MOCK_GH_JSON='[{"name":"Validation","bucket":"pass","state":"SUCCESS","link":"https://github.com/ITmedes/browserpane/actions/runs/1"}]'
wait_for_checks 999 || fail "green check set is accepted"
pass "green check set is accepted"

MOCK_GH_JSON='[{"name":"Validation","bucket":"fail","state":"FAILURE","link":"https://github.com/ITmedes/browserpane/actions/runs/2"}]'
MOCK_GH_RC=1
check_rc=0
wait_for_checks 999 || check_rc=$?
assert_eq "1" "$check_rc" "failed check JSON is parsed despite gh exit status"

AUTO_RERUN_CANCELLED=0
MOCK_GH_JSON='[{"name":"Validation","bucket":"cancel","state":"CANCELLED","link":"https://github.com/ITmedes/browserpane/actions/runs/3"}]'
MOCK_GH_RC=1
check_rc=0
wait_for_checks 999 || check_rc=$?
assert_eq "2" "$check_rc" "exhausted cancelled check reruns are inconclusive"

MOCK_GH_JSON='[{"databaseId":4,"status":"completed","conclusion":"success","url":"https://github.com/ITmedes/browserpane/actions/runs/4"}]'
MOCK_GH_RC=0
wait_for_post_merge_workflow validation.yml abcdef1234567890 || fail "successful post-merge workflow is accepted"
pass "successful post-merge workflow is accepted"

MOCK_GH_JSON='[{"databaseId":5,"status":"completed","conclusion":"failure","url":"https://github.com/ITmedes/browserpane/actions/runs/5"}]'
post_merge_rc=0
wait_for_post_merge_workflow compose.yml abcdef1234567890 || post_merge_rc=$?
assert_eq "1" "$post_merge_rc" "failed post-merge workflow stops delivery"

printf '%s\n' '["Cargo.lock","README.md"]' | ci_builder_paths_changed \
  || fail "CI Rust builder path change is detected"
pass "CI Rust builder path change is detected"

if printf '%s\n' '["README.md","docs/CURRENT_CONTEXT.md"]' | ci_builder_paths_changed; then
  fail "unrelated paths do not trigger the CI Rust builder"
fi
pass "unrelated paths do not trigger the CI Rust builder"

unset -f gh

original_lock="$LOCK_DIR"
LOCK_DIR="$tmp/lock"
mkdir "$LOCK_DIR"
printf '%s\n' 99999999 > "$LOCK_DIR/pid"
acquire_lock
assert_eq "$$" "$(cat "$LOCK_DIR/pid")" "stale PID lock is replaced"
release_lock
[[ ! -e "$LOCK_DIR" ]] || fail "owned PID lock is released"
pass "owned PID lock is released"
LOCK_DIR="$original_lock"

printf '1..%d\n' "$tests"
