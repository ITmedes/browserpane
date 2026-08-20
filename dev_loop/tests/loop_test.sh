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
valid_qualified="$tmp/valid-qualified.json"
valid_no_qualification="$tmp/valid-no-qualification.json"
invalid_qualified="$tmp/invalid-qualified.json"
printf '%s\n' '{"status":"PROPOSED","issue_number":239,"pr_url":"https://github.com/ITmedes/browserpane/pull/999","commit_sha":"abc1234","run_id":null,"reason":null,"summary":"opened"}' > "$valid"
printf '%s\n' '{"status":"UNKNOWN","summary":"bad"}' > "$invalid"
printf '%s\n' '{"status":"PROPOSED","issue_number":239,"pr_url":null,"commit_sha":null,"run_id":null,"reason":null,"summary":"incomplete"}' > "$invalid_proposed"
printf '%s\n' '{"status":"QUALIFIED","issue_number":172,"pr_url":null,"commit_sha":null,"run_id":null,"reason":null,"summary":"promoted"}' > "$valid_qualified"
printf '%s\n' '{"status":"NO_QUALIFICATION","issue_number":null,"pr_url":null,"commit_sha":null,"run_id":null,"reason":"dependency remains","summary":"no mutation"}' > "$valid_no_qualification"
printf '%s\n' '{"status":"QUALIFIED","issue_number":null,"pr_url":null,"commit_sha":null,"run_id":null,"reason":null,"summary":"missing issue"}' > "$invalid_qualified"

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

validate_result "$valid_qualified" || fail "valid qualification result"
pass "valid qualification result"

validate_result "$valid_no_qualification" || fail "valid no-qualification result"
pass "valid no-qualification result"

if validate_result "$invalid_qualified" >/dev/null 2>&1; then
  fail "qualification requires an issue number"
fi
pass "qualification requires an issue number"

assert_eq "PROPOSED" "$(routine_field "$valid" status)" "routine status extraction"
assert_eq "239" "$(routine_field "$valid" issue_number)" "routine issue extraction"

MOCK_DISK_MODE=valid
MOCK_DISK_KIB=$((49 * 1048576))
df() {
  case "$MOCK_DISK_MODE" in
    valid)
      printf '%s\n' \
        'Filesystem 1024-blocks Used Available Capacity Mounted on' \
        "/dev/mock 999999999 1 $MOCK_DISK_KIB 1% /mock"
      ;;
    malformed) printf '%s\n' 'Filesystem output is incomplete' ;;
    failure) return 1 ;;
  esac
}

assert_eq "$MOCK_DISK_KIB" "$(disk_available_kib)" "available disk capacity is parsed from POSIX df output"
assert_eq "49.0" "$(format_disk_gib "$MOCK_DISK_KIB")" "disk capacity is formatted as binary GiB"

MIN_FREE_DISK_GB=50
disk_guard_rc=0
check_disk_space_guard >/dev/null 2>&1 || disk_guard_rc=$?
assert_eq "1" "$disk_guard_rc" "capacity below 50 GiB is rejected"

MOCK_DISK_KIB=$((50 * 1048576))
check_disk_space_guard >/dev/null 2>&1 || fail "capacity equal to 50 GiB is accepted"
pass "capacity equal to 50 GiB is accepted"

MOCK_DISK_KIB=$((51 * 1048576))
check_disk_space_guard >/dev/null 2>&1 || fail "capacity above 50 GiB is accepted"
pass "capacity above 50 GiB is accepted"

MIN_FREE_DISK_GB=0
MOCK_DISK_KIB=0
check_disk_space_guard >/dev/null 2>&1 || fail "zero threshold explicitly disables the minimum"
pass "zero threshold explicitly disables the minimum"

MIN_FREE_DISK_GB=50
MOCK_DISK_MODE=malformed
disk_guard_rc=0
check_disk_space_guard >/dev/null 2>&1 || disk_guard_rc=$?
assert_eq "2" "$disk_guard_rc" "malformed disk capacity fails closed"

MOCK_DISK_MODE=failure
disk_guard_rc=0
check_disk_space_guard >/dev/null 2>&1 || disk_guard_rc=$?
assert_eq "2" "$disk_guard_rc" "disk measurement command failure fails closed"
unset -f df

proposal_log_dir="$tmp/proposal"
mkdir -p "$proposal_log_dir"
LOG_DIR="$proposal_log_dir"
GITHUB_LOGIN="thebackplane"
latest_workflow_line() { printf '%s\n' "completed / success"; }
ready_issues() { printf '%s\n' "(none)"; }
human_prs() { printf '%s\n' "(none)"; }
qualified_issues() { printf '%s\n' "- #172 qualified"; }
MOCK_SESSION_RESULT="$valid_qualified"
run_session() {
  : > "$2"
  cp "$MOCK_SESSION_RESULT" "$3"
}
record_usage() { :; }
qualify "01" || fail "qualification setup is nounset-safe"
[[ -f "$proposal_log_dir/01-qualify.context.md" ]] || fail "qualification context uses the iteration number"
[[ -f "$proposal_log_dir/01-qualify.prompt.md" ]] || fail "qualification prompt uses the iteration number"
pass "qualification setup is nounset-safe"

MOCK_SESSION_RESULT="$valid"
propose "01" || fail "proposal setup is nounset-safe"
[[ -f "$proposal_log_dir/01-propose.context.md" ]] || fail "proposal context uses the iteration number"
[[ -f "$proposal_log_dir/01-propose.prompt.md" ]] || fail "proposal prompt uses the iteration number"
pass "proposal setup is nounset-safe"

MOCK_READY_BEFORE=0
MOCK_READY_AFTER=1
MOCK_QUALIFIED_NUMBERS=172
MOCK_STATE_LABEL="state:ready"
MOCK_READY_MARKER="$tmp/ready-counted"
gh() {
  if [[ "$*" == *"issue list"* && "$*" == *"state:qualified"* ]]; then
    printf '%s\n' "$MOCK_QUALIFIED_NUMBERS"
  elif [[ "$*" == *"issue list"* ]]; then
    if [[ -f "$MOCK_READY_MARKER" ]]; then
      printf '%s\n' "$MOCK_READY_AFTER"
    else
      : > "$MOCK_READY_MARKER"
      printf '%s\n' "$MOCK_READY_BEFORE"
    fi
  else
    printf '{"labels":[{"name":"%s"}]}\n' "$MOCK_STATE_LABEL"
  fi
}
AUTO_QUALIFY=1
MOCK_SESSION_RESULT="$valid_qualified"
rm -f "$MOCK_READY_MARKER"
qualification_gate_rc=0
qualification_gate "02" || qualification_gate_rc=$?
assert_eq "0" "$qualification_gate_rc" "empty Ready queue promotes one verified candidate"
assert_eq "172" "$QUALIFIED_ISSUE" "qualified candidate is returned to the driver"

MOCK_STATE_LABEL="state:qualified"
rm -f "$MOCK_READY_MARKER"
qualification_gate_rc=0
qualification_gate "03" || qualification_gate_rc=$?
assert_eq "21" "$qualification_gate_rc" "unverified qualification blocks proposal"

MOCK_STATE_LABEL="state:ready"
MOCK_READY_AFTER=2
rm -f "$MOCK_READY_MARKER"
qualification_gate_rc=0
qualification_gate "04" || qualification_gate_rc=$?
assert_eq "21" "$qualification_gate_rc" "multiple Ready promotions block proposal"

MOCK_READY_AFTER=1
MOCK_SESSION_RESULT="$valid_no_qualification"
rm -f "$MOCK_READY_MARKER"
qualification_gate_rc=0
qualification_gate "05" || qualification_gate_rc=$?
assert_eq "10" "$qualification_gate_rc" "unqualifiable queue stops cleanly"
assert_eq "dependency remains" "$QUALIFICATION_REASON" "qualification stop reason is retained"

AUTO_QUALIFY=0
MOCK_SESSION_RESULT="$invalid"
rm -f "$MOCK_READY_MARKER"
qualification_gate_rc=0
qualification_gate "06" || qualification_gate_rc=$?
assert_eq "0" "$qualification_gate_rc" "manual qualification mode bypasses the qualifier"
assert_eq "" "$QUALIFIED_ISSUE" "manual qualification mode returns no promoted issue"

AUTO_QUALIFY=1
MOCK_QUALIFIED_NUMBERS=""
rm -f "$MOCK_READY_MARKER"
qualification_gate_rc=0
qualification_gate "07" || qualification_gate_rc=$?
assert_eq "10" "$qualification_gate_rc" "empty Qualified queue stops without a model session"
assert_eq "no open Qualified issue is available for readiness assessment" "$QUALIFICATION_REASON" "empty Qualified queue reason is explicit"
unset -f gh

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

if AUTO_QUALIFY=invalid bash -c "source '$loop'; validate_config" >/dev/null 2>&1; then
  fail "invalid auto-qualification configuration is rejected"
fi
pass "invalid auto-qualification configuration is rejected"

if MIN_FREE_DISK_GB=invalid bash -c "source '$loop'; validate_config" >/dev/null 2>&1; then
  fail "invalid disk threshold configuration is rejected"
fi
pass "invalid disk threshold configuration is rejected"

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

jq -e '.properties.status.enum | index("QUALIFIED") and index("NO_QUALIFICATION") and index("PROPOSED") and index("REPAIRED") and index("HALT")' "$RESULT_SCHEMA" >/dev/null \
  || fail "result schema status contract"
pass "result schema status contract"

MOCK_GH_JSON='[]'
MOCK_GH_RC=0
gh() {
  printf '%s\n' "$MOCK_GH_JSON"
  return "$MOCK_GH_RC"
}

MOCK_GH_JSON='{"labels":[{"name":"priority:P0"},{"name":"state:ready"}]}'
assert_eq "state:ready" "$(issue_state_labels 172)" "qualified issue lifecycle verification"

MOCK_GH_JSON='0'
assert_eq "0" "$(state_issue_count state:ready)" "empty Ready queue count"

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
