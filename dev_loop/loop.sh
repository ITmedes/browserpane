#!/usr/bin/env bash

# BrowserPane's closed local Codex implementation loop.
# Codex qualifies one issue, specifies one requirements gap set, proposes one
# implementation PR, or repairs one PR in separate sessions. This driver waits,
# updates, and optionally merges. Every external wait and repair budget is
# bounded.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"

ITERATIONS="${ITERATIONS:-0}"
MAX_REPAIRS="${MAX_REPAIRS:-4}"
MAX_SPECIFICATION_CYCLES="${MAX_SPECIFICATION_CYCLES:-3}"
MAX_UPDATE_BRANCH="${MAX_UPDATE_BRANCH:-3}"
CI_TIMEOUT_SECONDS="${CI_TIMEOUT_SECONDS:-5400}"
POLL_SECONDS="${POLL_SECONDS:-30}"
FAIL_FAST="${FAIL_FAST:-1}"
FAIL_FAST_GRACE="${FAIL_FAST_GRACE:-120}"
AUTO_RERUN_CANCELLED="${AUTO_RERUN_CANCELLED:-2}"
SETTLE_SECONDS="${SETTLE_SECONDS:-180}"
SESSION_TIMEOUT_SECONDS="${SESSION_TIMEOUT_SECONDS:-10800}"
POST_MERGE_TIMEOUT_SECONDS="${POST_MERGE_TIMEOUT_SECONDS:-7200}"
MIN_FREE_DISK_GB="${MIN_FREE_DISK_GB:-50}"
AUTO_QUALIFY="${AUTO_QUALIFY:-1}"
AUTO_MERGE="${AUTO_MERGE:-0}"
ADMIN_MERGE="${ADMIN_MERGE:-0}"
MERGE_METHOD="${MERGE_METHOD:-squash}"
DEFAULT_BRANCH="${DEFAULT_BRANCH:-main}"
BRANCH_PREFIX="${BRANCH_PREFIX:-codex/BPANE-}"
POST_MERGE_WORKFLOWS="${POST_MERGE_WORKFLOWS:-auto}"

CODEX_BIN="${CODEX_BIN:-codex}"
CODEX_SANDBOX="${CODEX_SANDBOX:-danger-full-access}"
APPROVAL_POLICY="${APPROVAL_POLICY:-never}"
ALLOWED_GITHUB_LOGINS="${ALLOWED_GITHUB_LOGINS:-thebackplane}"
MODEL="${MODEL:-}"
CODEX_PROFILE="${CODEX_PROFILE:-}"

STOP_FILE="$here/STOP"
LOCK_DIR="$here/.lock"
RUNS_DIR="$here/runs"
RESULT_SCHEMA="$here/schemas/routine-result.schema.json"
RUN_ID="$(date +%Y%m%d-%H%M%S)"
LOG_DIR="$RUNS_DIR/$RUN_ID"
JOURNAL="$LOG_DIR/journal.tsv"

if [[ -t 1 ]]; then
  c_reset=$'\033[0m'; c_b=$'\033[1m'; c_dim=$'\033[2m'
  c_red=$'\033[31m'; c_grn=$'\033[32m'; c_ylw=$'\033[33m'; c_cyn=$'\033[36m'
else
  c_reset=; c_b=; c_dim=; c_red=; c_grn=; c_ylw=; c_cyn=
fi

ts() { date +'%H:%M:%S'; }
say() { printf '%s[%s]%s %s\n' "$c_dim" "$(ts)" "$c_reset" "$*"; }
step() { printf '\n%s[%s] -- %s%s\n' "$c_b$c_cyn" "$(ts)" "$*" "$c_reset"; }
good() { printf '%s[%s] %s%s\n' "$c_grn" "$(ts)" "$*" "$c_reset"; }
warn() { printf '%s[%s] %s%s\n' "$c_ylw" "$(ts)" "$*" "$c_reset" >&2; }
err() { printf '%s[%s] %s%s\n' "$c_red" "$(ts)" "$*" "$c_reset" >&2; }
die() { err "$*"; exit 1; }

require() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is required but is not on PATH."
}

is_uint() { [[ "$1" =~ ^[0-9]+$ ]]; }
is_positive_uint() { is_uint "$1" && (( 10#$1 > 0 )); }

validate_config() {
  local name value
  for name in ITERATIONS MAX_REPAIRS MAX_UPDATE_BRANCH AUTO_RERUN_CANCELLED MIN_FREE_DISK_GB; do
    value="${!name}"
    is_uint "$value" || die "$name must be a non-negative integer, got '$value'."
  done
  for name in MAX_SPECIFICATION_CYCLES CI_TIMEOUT_SECONDS POLL_SECONDS FAIL_FAST_GRACE SETTLE_SECONDS SESSION_TIMEOUT_SECONDS POST_MERGE_TIMEOUT_SECONDS; do
    value="${!name}"
    is_positive_uint "$value" || die "$name must be a positive integer, got '$value'."
  done
  for name in FAIL_FAST AUTO_QUALIFY AUTO_MERGE ADMIN_MERGE; do
    value="${!name}"
    [[ "$value" == "0" || "$value" == "1" ]] || die "$name must be 0 or 1, got '$value'."
  done
  if [[ "$ADMIN_MERGE" == "1" && "$AUTO_MERGE" != "1" ]]; then
    die "ADMIN_MERGE=1 requires AUTO_MERGE=1."
  fi
  case "$MERGE_METHOD" in squash|merge|rebase) ;; *) die "MERGE_METHOD must be squash, merge, or rebase." ;; esac
  [[ -n "$DEFAULT_BRANCH" ]] || die "DEFAULT_BRANCH must not be empty."
  [[ -n "$BRANCH_PREFIX" ]] || die "BRANCH_PREFIX must not be empty."
}

disk_available_kib() {
  local available
  available="$(df -Pk "$repo" 2>/dev/null | awk 'NR == 2 { print $4; found = 1; exit } END { if (!found) exit 1 }')" || return 1
  is_uint "$available" || return 1
  printf '%s\n' "$available"
}

format_disk_gib() {
  LC_ALL=C awk -v available_kib="$1" 'BEGIN { printf "%.1f", available_kib / 1048576 }'
}

disk_space_sufficient() {
  local available_kib="$1"
  LC_ALL=C awk -v available_kib="$available_kib" -v minimum_gib="$MIN_FREE_DISK_GB" \
    'BEGIN { exit !(minimum_gib == 0 || available_kib >= minimum_gib * 1048576) }'
}

check_disk_space_guard() {
  local available_kib available_gib
  if ! available_kib="$(disk_available_kib)"; then
    err "disk space guard could not measure available capacity for $repo"
    return 2
  fi
  available_gib="$(format_disk_gib "$available_kib")"
  if disk_space_sufficient "$available_kib"; then
    if [[ "$MIN_FREE_DISK_GB" =~ ^0+$ ]]; then
      say "disk space: ${available_gib} GiB available (minimum disabled)"
    else
      say "disk space: ${available_gib} GiB available (minimum ${MIN_FREE_DISK_GB} GiB)"
    fi
    return 0
  fi
  err "disk space guard blocked the loop: ${available_gib} GiB available; minimum is ${MIN_FREE_DISK_GB} GiB"
  return 1
}

github_identity_allowed() {
  local login="$1" allowed
  [[ -n "$login" ]] || return 1
  while IFS= read -r allowed; do
    allowed="${allowed//[[:space:]]/}"
    [[ -z "$allowed" ]] && continue
    [[ "$login" == "$allowed" ]] && return 0
  done < <(tr ',' '\n' <<< "$ALLOWED_GITHUB_LOGINS")
  return 1
}

github_permission_allowed() {
  case "$1" in ADMIN|MAINTAIN|WRITE) return 0 ;; *) return 1 ;; esac
}

admin_merge_permission_allowed() { [[ "$1" == "ADMIN" ]]; }

acquire_lock() {
  if mkdir "$LOCK_DIR" 2>/dev/null; then
    printf '%s\n' "$$" > "$LOCK_DIR/pid"
    return
  fi

  local other
  other="$(cat "$LOCK_DIR/pid" 2>/dev/null || true)"
  if [[ -n "$other" ]] && kill -0 "$other" 2>/dev/null; then
    die "another development loop is running in this checkout (pid $other)."
  fi

  warn "removing stale loop lock from pid ${other:-unknown}"
  rm -rf "$LOCK_DIR"
  mkdir "$LOCK_DIR" || die "could not acquire $LOCK_DIR"
  printf '%s\n' "$$" > "$LOCK_DIR/pid"
}

release_lock() {
  if [[ -f "$LOCK_DIR/pid" && "$(cat "$LOCK_DIR/pid" 2>/dev/null)" == "$$" ]]; then
    rm -rf "$LOCK_DIR"
  fi
}

stop_requested() {
  if [[ -f "$STOP_FILE" ]]; then
    warn "$STOP_FILE exists; stopping between phases."
    rm -f "$STOP_FILE"
    return 0
  fi
  return 1
}

validate_result() {
  local file="$1"
  [[ -f "$file" ]] || return 1
  jq -e '
    type == "object" and
    ((keys | sort) == (["commit_sha","issue_number","pr_url","reason","run_id","status","summary"] | sort)) and
    (.status == "QUALIFIED" or .status == "NEEDS_SPECIFICATION" or
     .status == "NO_QUALIFICATION" or .status == "SPECIFIED" or
     .status == "PROPOSED" or .status == "NO_PROPOSAL" or
     .status == "REPAIRED" or .status == "RERUN_ONLY" or .status == "HALT") and
    ((.issue_number == null) or ((.issue_number | type) == "number" and .issue_number >= 1 and .issue_number == (.issue_number | floor))) and
    ((.pr_url == null) or ((.pr_url | type) == "string")) and
    ((.commit_sha == null) or ((.commit_sha | type) == "string")) and
    ((.run_id == null) or ((.run_id | type) == "string")) and
    ((.reason == null) or ((.reason | type) == "string")) and
    ((.summary | type) == "string" and (.summary | length) > 0) and
    (if .status == "QUALIFIED" then
       .issue_number != null and .pr_url == null and .commit_sha == null and .run_id == null and
       (.reason == null or ((.reason | type) == "string" and (.reason | length) > 0))
     elif .status == "NO_QUALIFICATION" then
       .pr_url == null and .commit_sha == null and .run_id == null and
       (.reason | type) == "string" and (.reason | length) > 0
     elif .status == "NEEDS_SPECIFICATION" then
       .issue_number != null and .pr_url == null and .commit_sha == null and .run_id == null and
       (.reason | type) == "string" and (.reason | length) > 0
     elif .status == "SPECIFIED" or .status == "PROPOSED" then
       .issue_number != null and (.pr_url | type) == "string" and (.pr_url | startswith("https://github.com/")) and
       (.commit_sha | type) == "string" and (.commit_sha | test("^[0-9a-f]{7,64}$")) and
       .run_id == null and .reason == null
     elif .status == "REPAIRED" then
       .issue_number != null and (.pr_url | type) == "string" and
       (.commit_sha | type) == "string" and (.commit_sha | test("^[0-9a-f]{7,64}$"))
     elif .status == "RERUN_ONLY" then
       .issue_number != null and (.pr_url | type) == "string" and
       (.run_id | type) == "string" and (.run_id | test("^[0-9]+$"))
     elif .status == "NO_PROPOSAL" or .status == "HALT" then
       (.reason | type) == "string" and (.reason | length) > 0
     else false end)
  ' "$file" >/dev/null 2>&1
}

result_contract_diagnostic() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    printf '%s\n' 'result-file=missing'
    return
  fi
  if ! jq -e . "$file" >/dev/null 2>&1; then
    printf '%s\n' 'result-json=invalid'
    return
  fi
  jq -r '
    def shape($name):
      .[$name] as $value |
      $name + "=" +
      (if $value == null then "null"
       elif ($value | type) == "string" then
         "string(" + (if ($value | length) > 0 then "non-empty" else "empty" end) + ")"
       else ($value | type)
       end);
    "status=" + (if (.status | type) == "string" then .status else (.status | type) end) +
    " issue_number=" +
      (if .issue_number == null then "null"
       elif (.issue_number | type) == "number" then (.issue_number | tostring)
       else (.issue_number | type)
       end) +
    " " + shape("pr_url") +
    " " + shape("commit_sha") +
    " " + shape("run_id") +
    " " + shape("reason") +
    " " + shape("summary")
  ' "$file" 2>/dev/null || printf '%s\n' 'result-shape=unavailable'
}

routine_field() { jq -r --arg field "$2" '.[$field] // empty' "$1"; }

session_meta() {
  jq -Rrs '
    [split("\n")[] | fromjson? | select(.type == "turn.completed") | .usage] | last // {} |
    [(.input_tokens // 0), (.cached_input_tokens // 0), (.output_tokens // 0), (.reasoning_output_tokens // 0)] |
    @tsv
  ' "$1" 2>/dev/null || printf '0\t0\t0\t0\n'
}

stream_view() {
  jq -Rr --unbuffered '
    fromjson? // empty |
    if .type == "thread.started" then "  thread " + (.thread_id // "started")
    elif .type == "item.started" and .item.type == "command_execution" then
      "  command: " + ((.item.command // "") | gsub("\\s+"; " ") | if length > 120 then .[0:120] + "..." else . end)
    elif .type == "item.completed" and .item.type == "agent_message" then (.item.text // "")
    elif .type == "item.completed" and .item.type == "file_change" then "  files changed"
    elif .type == "turn.completed" then
      "  usage: input=" + ((.usage.input_tokens // 0) | tostring) +
      " cached=" + ((.usage.cached_input_tokens // 0) | tostring) +
      " output=" + ((.usage.output_tokens // 0) | tostring)
    elif .type == "turn.failed" then "  turn failed: " + ((.error.message? // .error // "unknown") | tostring)
    elif .type == "error" then "  error: " + ((.message // .error // "unknown") | tostring)
    else empty end
  '
}

build_prompt() {
  local routine="$1" context="$2" output="$3"
  { sed -n '1,$p' "$routine"; printf '\n\n---\n\n'; sed -n '1,$p' "$context"; } > "$output"
}

run_session() {
  local prompt="$1" raw="$2" final="$3" stderr_log="${2%.jsonl}.stderr.log"
  local -a args=()
  [[ -n "$CODEX_PROFILE" ]] && args+=(-p "$CODEX_PROFILE")
  args+=(-a "$APPROVAL_POLICY" exec --json -s "$CODEX_SANDBOX" -C "$repo")
  [[ -n "$MODEL" ]] && args+=(-m "$MODEL")
  args+=(--output-schema "$RESULT_SCHEMA" -o "$final" -)

  : > "$raw"
  : > "$stderr_log"
  rm -f "$final"
  local rc=0 codex_pid watchdog_pid
  set +e
  "$CODEX_BIN" "${args[@]}" < "$prompt" \
    > >(tee "$raw" | stream_view) \
    2> >(tee "$stderr_log" >&2) &
  codex_pid=$!
  (
    local deadline=$(( $(date +%s) + SESSION_TIMEOUT_SECONDS ))
    while kill -0 "$codex_pid" 2>/dev/null; do
      if (( $(date +%s) >= deadline )); then
        warn "Codex session exceeded ${SESSION_TIMEOUT_SECONDS}s; terminating it."
        kill -TERM "$codex_pid" 2>/dev/null || true
        sleep 10
        kill -KILL "$codex_pid" 2>/dev/null || true
        break
      fi
      sleep 5
    done
  ) &
  watchdog_pid=$!
  { wait "$codex_pid"; rc=$?; } 2>/dev/null
  kill "$watchdog_pid" 2>/dev/null || true
  { wait "$watchdog_pid"; } 2>/dev/null || true
  set -e

  local flush_attempt
  for flush_attempt in 1 2 3 4 5 6 7 8 9 10; do
    grep -q '"type":"turn.completed"\|"type": "turn.completed"\|"type":"turn.failed"\|"type": "turn.failed"' "$raw" 2>/dev/null && break
    sleep 0.2
  done

  if ! validate_result "$final"; then
    err "Codex did not produce a valid structured result at $final."
    err "result contract: $(result_contract_diagnostic "$final")"
    return 2
  fi
  return "$rc"
}

run_ids_from_links() {
  grep -Eo '/actions/runs/[0-9]+' | grep -Eo '[0-9]+' | sort -u || true
}

automation_pr() {
  gh pr list --state open --limit 100 \
    --json number,headRefName,headRefOid,isDraft,url,title,mergeable,mergeStateStatus \
    --jq "[.[] | select(.headRefName | startswith(\"$BRANCH_PREFIX\"))] | sort_by(.number) | .[0] // empty"
}

automation_pr_count() {
  gh pr list --state open --limit 100 --json headRefName \
    --jq "[.[] | select(.headRefName | startswith(\"$BRANCH_PREFIX\"))] | length"
}

human_prs() {
  gh pr list --state open --limit 100 --json number,headRefName,url,title,author \
    --jq "[.[] | select(.headRefName | startswith(\"$BRANCH_PREFIX\") | not)] |
      if length == 0 then \"(none)\" else
      map(\"- #\\(.number) \\(.title) | \\(.headRefName) | @\\(.author.login)\") | join(\"\\n\") end"
}

ready_issues() {
  gh issue list --state open --label state:ready --limit 100 \
    --json number,title,labels,url \
    --jq 'if length == 0 then "(none)" else
      sort_by(.number) | map("- #\(.number) \(.title) | " + ([.labels[].name] | join(", ")) + " | \(.url)") | join("\n") end'
}

qualified_issues() {
  gh issue list --state open --label state:qualified --limit 100 \
    --json number,title,labels,url \
    --jq 'if length == 0 then "(none)" else
      sort_by(.number) | map("- #\(.number) \(.title) | " + ([.labels[].name] | join(", ")) + " | \(.url)") | join("\n") end'
}

state_issue_count() {
  local label="$1"
  gh issue list --state open --label "$label" --limit 100 --json number --jq 'length'
}

state_issue_numbers() {
  local label="$1"
  gh issue list --state open --label "$label" --limit 100 --json number \
    --jq 'sort_by(.number) | .[].number'
}

issue_state_labels() {
  local issue_number="$1"
  gh issue view "$issue_number" --json labels \
    | jq -r '[.labels[].name | select(startswith("state:"))] | sort | join(",")'
}

latest_workflow_line() {
  local workflow="$1"
  gh run list --workflow "$workflow" --branch "$DEFAULT_BRANCH" --limit 1 \
    --json status,conclusion,url,headSha \
    --jq 'if length == 0 then "no runs found" else .[0] |
      "\(.status) / \(.conclusion // "pending") | \(.headSha[0:12]) | \(.url)" end' 2>/dev/null || printf 'unknown\n'
}

pr_head_oid() { gh pr view "$1" --json headRefOid --jq .headRefOid 2>/dev/null || true; }

preflight_tools() {
  validate_config
  require awk; require bash; require df; require git; require gh; require jq; require "$CODEX_BIN"
  [[ -f "$RESULT_SCHEMA" ]] || die "missing result schema: $RESULT_SCHEMA"
  [[ -f "$here/routines/qualify.md" ]] || die "missing qualification routine"
  [[ -f "$here/routines/specify.md" ]] || die "missing specification routine"
  [[ -f "$here/routines/propose.md" ]] || die "missing proposal routine"
  [[ -f "$here/routines/repair.md" ]] || die "missing repair routine"
  jq -e . "$RESULT_SCHEMA" >/dev/null || die "result schema is not valid JSON"
  git -C "$repo" rev-parse --git-dir >/dev/null 2>&1 || die "$repo is not a Git repository"
  gh auth status >/dev/null 2>&1 || die "gh is not authenticated; run gh auth login"
  "$CODEX_BIN" --version >/dev/null 2>&1 || die "Codex CLI is not runnable"
  GITHUB_LOGIN="$(gh api user --jq .login 2>/dev/null || true)"
  [[ -n "$GITHUB_LOGIN" ]] || die "could not establish the authenticated GitHub identity"
  github_identity_allowed "$GITHUB_LOGIN" || die "GitHub identity '$GITHUB_LOGIN' is forbidden for this loop"
  GITHUB_PERMISSION="$(gh repo view --json viewerPermission --jq .viewerPermission 2>/dev/null || true)"
  github_permission_allowed "$GITHUB_PERMISSION" || die "GitHub identity '$GITHUB_LOGIN' has insufficient repository permission '${GITHUB_PERMISSION:-unknown}'"
  if [[ "$ADMIN_MERGE" == "1" ]] && ! admin_merge_permission_allowed "$GITHUB_PERMISSION"; then
    die "ADMIN_MERGE=1 requires GitHub repository permission ADMIN, got '${GITHUB_PERMISSION:-unknown}'."
  fi
}

repo_ready_report() {
  local branch dirty ready=0 disk_ready=0
  branch="$(git -C "$repo" branch --show-current)"
  dirty="$(git -C "$repo" status --porcelain)"
  say "repository: $repo"
  say "branch: ${branch:-detached} (required: $DEFAULT_BRANCH)"
  if [[ -n "$dirty" ]]; then
    warn "working tree is dirty; the loop would stop:"
    printf '%s\n' "$dirty" | sed 's/^/    /' >&2
  else
    good "working tree is clean"
  fi
  say "Codex: $($CODEX_BIN --version)"
  say "GitHub identity: $GITHUB_LOGIN"
  say "GitHub repository permission: $GITHUB_PERMISSION"
  say "Admin merge: $ADMIN_MERGE"
  local active
  active="$(automation_pr || true)"
  if [[ -n "$active" ]]; then
    say "active Codex PR: #$(jq -r .number <<< "$active") $(jq -r .url <<< "$active")"
  else
    say "active Codex PR: none"
  fi
  if check_disk_space_guard; then disk_ready=1; fi
  say "Ready issues: $(state_issue_count state:ready 2>/dev/null || printf unknown)"
  say "Qualified issues: $(state_issue_count state:qualified 2>/dev/null || printf unknown)"
  [[ "$branch" == "$DEFAULT_BRANCH" && -z "$dirty" && "$disk_ready" == "1" ]] && ready=1
  if (( ready == 1 )); then
    good "read-only preflight passed; the checkout can enter the loop"
    return 0
  fi
  warn "read-only preflight completed; clean and synchronize the checkout before running the loop"
  return 1
}

sync_default_branch() {
  local branch dirty ahead
  branch="$(git branch --show-current)"
  [[ "$branch" == "$DEFAULT_BRANCH" ]] || die "start the loop on $DEFAULT_BRANCH, not ${branch:-detached}."
  dirty="$(git status --porcelain)"
  [[ -z "$dirty" ]] || { err "the loop will not touch a dirty working tree:"; printf '%s\n' "$dirty" >&2; exit 1; }
  git fetch --prune origin >/dev/null 2>&1 || die "git fetch origin failed"
  ahead="$(git rev-list --count "origin/$DEFAULT_BRANCH..$DEFAULT_BRANCH")"
  (( ahead == 0 )) || die "local $DEFAULT_BRANCH is $ahead commit(s) ahead of origin; publish or move that work first."
  git merge --ff-only "origin/$DEFAULT_BRANCH" >/dev/null 2>&1 || die "could not fast-forward $DEFAULT_BRANCH"
}

up_to_date_with_main() {
  local branch="$1" tip merge_base
  git fetch --prune origin >/dev/null 2>&1 || return 1
  tip="$(git rev-parse "origin/$DEFAULT_BRANCH" 2>/dev/null || true)"
  merge_base="$(git merge-base "$tip" "origin/$branch" 2>/dev/null || true)"
  [[ -n "$tip" && "$merge_base" == "$tip" ]]
}

wait_for_checks() {
  local pr="$1" deadline=$(( $(date +%s) + CI_TIMEOUT_SECONDS ))
  local reruns=0 first_failure=0 previous=""
  while :; do
    (( $(date +%s) <= deadline )) || return 3
    local json
    json="$(gh pr checks "$pr" --json name,bucket,state,link 2>/dev/null || true)"
    if ! jq -e 'type == "array"' <<< "$json" >/dev/null 2>&1; then
      say "no checks reported for #$pr yet"
      sleep "$POLL_SECONDS"
      continue
    fi
    local total passed failed pending cancelled skipped
    total="$(jq 'length' <<< "$json")"
    passed="$(jq '[.[] | select(.bucket == "pass")] | length' <<< "$json")"
    failed="$(jq '[.[] | select(.bucket == "fail")] | length' <<< "$json")"
    pending="$(jq '[.[] | select(.bucket == "pending")] | length' <<< "$json")"
    cancelled="$(jq '[.[] | select(.bucket == "cancel")] | length' <<< "$json")"
    skipped="$(jq '[.[] | select(.bucket == "skipping")] | length' <<< "$json")"
    if (( total == 0 )); then
      say "check set is empty; waiting for GitHub to start it"
      sleep "$POLL_SECONDS"
      continue
    fi
    local line="checks: $passed pass | $pending pending | $failed fail | $cancelled cancelled | $skipped skipped"
    if [[ "$line" != "$previous" ]]; then say "$line"; previous="$line"; fi
    if (( failed > 0 )); then
      (( pending == 0 )) && return 1
      if [[ "$FAIL_FAST" == "1" ]]; then
        if (( first_failure == 0 )); then first_failure="$(date +%s)"; fi
        (( $(date +%s) - first_failure >= FAIL_FAST_GRACE )) && return 1
      fi
      sleep "$POLL_SECONDS"
      continue
    fi
    (( pending > 0 )) && { sleep "$POLL_SECONDS"; continue; }
    if (( cancelled > 0 )); then
      if (( reruns < AUTO_RERUN_CANCELLED )); then
        local ids id
        ids="$(jq -r '.[] | select(.bucket == "cancel") | .link // empty' <<< "$json" | run_ids_from_links)"
        for id in $ids; do
          say "rerunning cancelled Actions run $id"
          gh run rerun "$id" >/dev/null 2>&1 || warn "could not rerun $id"
        done
        reruns=$((reruns + 1))
        sleep "$POLL_SECONDS"
        continue
      fi
      return 2
    fi
    return 0
  done
}

settle_after_change() {
  local pr="$1" old_sha="$2" deadline=$(( $(date +%s) + SETTLE_SECONDS ))
  while (( $(date +%s) < deadline )); do
    local new_sha json
    new_sha="$(pr_head_oid "$pr")"
    if [[ -n "$new_sha" && "$new_sha" != "$old_sha" ]]; then
      say "PR head moved to ${new_sha:0:12}"
      return 0
    fi
    json="$(gh pr checks "$pr" --json bucket 2>/dev/null || true)"
    if jq -e 'type == "array"' <<< "$json" >/dev/null 2>&1 &&
      (( $(jq '[.[] | select(.bucket == "pending")] | length' <<< "$json") > 0 )); then
      say "check set restarted"
      return 0
    fi
    sleep 10
  done
  warn "no changed head or pending check appeared within ${SETTLE_SECONDS}s"
}

failing_checks() {
  local json
  json="$(gh pr checks "$1" --json name,bucket,link 2>/dev/null || true)"
  jq -r '[.[] | select(.bucket == "fail" or .bucket == "cancel")] |
    if length == 0 then "(none)" else map("- \(.name) | \(.link)") | join("\n") end' \
    <<< "$json" 2>/dev/null || printf '(unavailable)\n'
}

failing_run_ids() {
  local json
  json="$(gh pr checks "$1" --json bucket,link 2>/dev/null || true)"
  jq -r '.[] | select(.bucket == "fail" or .bucket == "cancel") | .link // empty' \
    <<< "$json" 2>/dev/null | run_ids_from_links | tr '\n' ' '
}

ci_stall_diagnostics() {
  local pr="$1"
  gh pr checks "$pr" 2>/dev/null | sed 's/^/    /' || true
  gh run list --branch "$(gh pr view "$pr" --json headRefName --jq .headRefName)" --limit 10 \
    --json databaseId,name,status,conclusion,url \
    --jq '.[] | "    #\(.databaseId) \(.name): \(.status) / \(.conclusion // "pending") | \(.url)"' 2>/dev/null || true
}

wait_for_post_merge_workflow() {
  local workflow="$1" commit_sha="$2"
  local deadline=$(( $(date +%s) + POST_MERGE_TIMEOUT_SECONDS )) previous=""
  while (( $(date +%s) <= deadline )); do
    local json count status conclusion line
    json="$(gh run list --workflow "$workflow" --commit "$commit_sha" --limit 1 \
      --json databaseId,status,conclusion,url 2>/dev/null || printf '[]')"
    count="$(jq 'length' <<< "$json")"
    if (( count == 0 )); then
      line="$workflow: waiting for a run on ${commit_sha:0:12}"
      [[ "$line" == "$previous" ]] || say "$line"
      previous="$line"
      sleep "$POLL_SECONDS"
      continue
    fi
    status="$(jq -r '.[0].status' <<< "$json")"
    conclusion="$(jq -r '.[0].conclusion // "pending"' <<< "$json")"
    line="$workflow: $status / $conclusion | $(jq -r '.[0].url' <<< "$json")"
    [[ "$line" == "$previous" ]] || say "$line"
    previous="$line"
    if [[ "$status" != "completed" ]]; then
      sleep "$POLL_SECONDS"
      continue
    fi
    [[ "$conclusion" == "success" || "$conclusion" == "neutral" || "$conclusion" == "skipped" ]]
    return
  done
  return 2
}

ci_builder_paths_changed() {
  jq -e '
    [.[] | select(
      test("^(deploy/(Dockerfile\\.ci-rust|install-rust-toolchain\\.sh)|rust-toolchain\\.toml|Cargo\\.(lock|toml)|code/(shared/bpane-protocol|apps/bpane-host|apps/bpane-gateway)/Cargo\\.toml|scripts/ci/(ci-rust-builder-ref\\.mjs|inspect-ci-rust-builder\\.sh|ci-rust-builder-workflow-contract\\.test\\.mjs)|\\.github/workflows/ci-rust-builder\\.yml)$")
    )] | length > 0
  ' >/dev/null 2>&1
}

post_merge_workflows_for_pr() {
  local pr_number="$1"
  if [[ "$POST_MERGE_WORKFLOWS" != "auto" ]]; then
    printf '%s\n' "$POST_MERGE_WORKFLOWS"
    return
  fi

  local files
  files="$(gh pr view "$pr_number" --json files --jq '[.files[].path]' 2>/dev/null || printf '[]')"
  if ci_builder_paths_changed <<< "$files"; then
    printf '%s\n' 'ci-rust-builder.yml validation.yml compose.yml'
  else
    printf '%s\n' 'validation.yml compose.yml'
  fi
}

wait_for_post_merge_workflows() {
  local commit_sha="$1" workflows="$2" workflow rc
  for workflow in $workflows; do
    rc=0
    wait_for_post_merge_workflow "$workflow" "$commit_sha" || rc=$?
    case "$rc" in
      0) good "$workflow passed for ${commit_sha:0:12}" ;;
      1) return 1 ;;
      2) return 2 ;;
    esac
  done
}

journal_row() {
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$@" >> "$JOURNAL"
}

record_usage() {
  local jsonl="$1" meta
  meta="$(session_meta "$jsonl")"
  IFS=$'\t' read -r meta_input meta_cached meta_output meta_reasoning <<< "$meta"
  iter_input=$((iter_input + ${meta_input:-0}))
  iter_cached=$((iter_cached + ${meta_cached:-0}))
  iter_output=$((iter_output + ${meta_output:-0}))
  iter_reasoning=$((iter_reasoning + ${meta_reasoning:-0}))
}

record_outcome() {
  local outcome="$1"
  journal_row "$n" "${pr:-}" "$outcome" "${attempt:-0}" "$(( $(date +%s) - iter_start ))" \
    "$iter_input" "$iter_cached" "$iter_output" "$iter_reasoning"
}

enforce_iteration_disk_space() {
  local phase="$1"
  if check_disk_space_guard; then return 0; fi
  record_outcome "low-disk"
  die "disk space guard stopped the loop before $phase"
}

reserve_specification_cycle() {
  local used="${specification_cycles:-0}"
  (( used < MAX_SPECIFICATION_CYCLES )) || return 1
  specification_cycles=$((used + 1))
}

qualify() {
  local number="$1"
  local context="$LOG_DIR/$number-qualify.context.md"
  local prompt="$LOG_DIR/$number-qualify.prompt.md" raw="$LOG_DIR/$number-qualify.jsonl"
  local final="$LOG_DIR/$number-qualify.result.json"
  {
    printf '# This run\n\n'
    printf -- '- Repository: `%s`\n' "$repo"
    printf -- '- Default branch: `%s` at `%s`\n' "$DEFAULT_BRANCH" "$(git rev-parse "origin/$DEFAULT_BRANCH")"
    printf -- '- Iteration: `%s`\n' "$number"
    printf -- '- Approved GitHub identity: `%s`\n' "$GITHUB_LOGIN"
    printf -- '- Latest main Validation: %s\n' "$(latest_workflow_line validation.yml)"
    printf -- '- Latest main Compose: %s\n\n' "$(latest_workflow_line compose.yml)"
    printf 'Live Ready issues (must still be empty):\n\n%s\n\n' "$(ready_issues || printf unavailable)"
    printf 'Live Qualified candidates:\n\n%s\n\n' "$(qualified_issues || printf unavailable)"
    printf 'Open non-Codex pull requests:\n\n%s\n\n' "$(human_prs || printf unavailable)"
    printf 'Qualification may comment on and promote at most one fully ready issue, or identify one bounded requirements gap set. '
    printf 'It must not edit Git, implement code, create a PR, wait for CI, or merge.\n'
  } > "$context"
  build_prompt "$here/routines/qualify.md" "$context" "$prompt"
  step "iteration $number | Codex qualification"
  local rc=0
  run_session "$prompt" "$raw" "$final" || rc=$?
  record_usage "$raw"
  return "$rc"
}

qualification_gate() {
  local number="$1" ready_count qualification_rc=0
  local qualified_before qualification_result qualification_status post_ready_count
  QUALIFIED_ISSUE=""
  QUALIFICATION_REASON=""
  SPECIFICATION_ISSUE=""

  if ! ready_count="$(state_issue_count state:ready)" || ! is_uint "$ready_count"; then
    QUALIFICATION_REASON="could not determine the live Ready issue count"
    return 20
  fi
  if (( ready_count > 0 )) || [[ "$AUTO_QUALIFY" == "0" ]]; then
    return 0
  fi

  if ! qualified_before="$(state_issue_numbers state:qualified)"; then
    QUALIFICATION_REASON="could not determine the live Qualified issue queue"
    return 20
  fi
  if [[ -z "$qualified_before" ]]; then
    QUALIFICATION_REASON="no open Qualified issue is available for readiness assessment"
    return 10
  fi

  qualify "$number" || qualification_rc=$?
  qualification_result="$LOG_DIR/$number-qualify.result.json"
  if (( qualification_rc != 0 )) || ! validate_result "$qualification_result"; then
    QUALIFICATION_REASON="qualification session failed; inspect $LOG_DIR/$number-qualify.jsonl and $qualification_result"
    return 20
  fi

  qualification_status="$(routine_field "$qualification_result" status)"
  case "$qualification_status" in
    QUALIFIED)
      QUALIFIED_ISSUE="$(routine_field "$qualification_result" issue_number)"
      if ! post_ready_count="$(state_issue_count state:ready)"; then
        QUALIFICATION_REASON="could not verify the Ready queue after qualification"
        return 21
      fi
      if ! grep -Fxq "$QUALIFIED_ISSUE" <<< "$qualified_before"; then
        QUALIFICATION_REASON="qualification reported issue #$QUALIFIED_ISSUE, which was not in the original Qualified queue"
        return 21
      fi
      if [[ "$post_ready_count" != "1" || "$(issue_state_labels "$QUALIFIED_ISSUE")" != "state:ready" ]]; then
        QUALIFICATION_REASON="qualification reported success but the Ready queue does not contain exactly issue #$QUALIFIED_ISSUE"
        return 21
      fi
      return 0
      ;;
    NO_QUALIFICATION)
      QUALIFICATION_REASON="$(routine_field "$qualification_result" reason)"
      return 10
      ;;
    NEEDS_SPECIFICATION)
      SPECIFICATION_ISSUE="$(routine_field "$qualification_result" issue_number)"
      QUALIFICATION_REASON="$(routine_field "$qualification_result" reason)"
      if ! grep -Fxq "$SPECIFICATION_ISSUE" <<< "$qualified_before"; then
        QUALIFICATION_REASON="qualification requested specification for issue #$SPECIFICATION_ISSUE, which was not in the original Qualified queue"
        return 21
      fi
      if ! post_ready_count="$(state_issue_count state:ready)"; then
        QUALIFICATION_REASON="could not verify the Ready queue after specification routing"
        return 21
      fi
      if [[ "$post_ready_count" != "0" || "$(issue_state_labels "$SPECIFICATION_ISSUE")" != "state:qualified" ]]; then
        QUALIFICATION_REASON="qualification requested specification but issue #$SPECIFICATION_ISSUE did not remain solely Qualified or the Ready queue changed"
        return 21
      fi
      return 12
      ;;
    HALT)
      QUALIFICATION_REASON="$(routine_field "$qualification_result" reason)"
      return 11
      ;;
    *)
      QUALIFICATION_REASON="qualification returned unexpected status $qualification_status"
      return 22
      ;;
  esac
}

specify() {
  local number="$1" issue_number="$2" qualification_reason="$3"
  local context="$LOG_DIR/$number-specify.context.md"
  local prompt="$LOG_DIR/$number-specify.prompt.md" raw="$LOG_DIR/$number-specify.jsonl"
  local final="$LOG_DIR/$number-specify.result.json"
  {
    printf '# This run\n\n'
    printf -- '- Repository: `%s`\n' "$repo"
    printf -- '- Default branch: `%s` at `%s`\n' "$DEFAULT_BRANCH" "$(git rev-parse "origin/$DEFAULT_BRANCH")"
    printf -- '- Iteration: `%s`\n' "$number"
    printf -- '- Approved GitHub identity: `%s`\n' "$GITHUB_LOGIN"
    printf -- '- Selected Qualified issue: `#%s`\n' "$issue_number"
    printf -- '- Latest main Validation: %s\n' "$(latest_workflow_line validation.yml)"
    printf -- '- Latest main Compose: %s\n\n' "$(latest_workflow_line compose.yml)"
    printf 'Qualification gap report:\n\n%s\n\n' "$qualification_reason"
    printf 'Live issue snapshot:\n\n'
    gh issue view "$issue_number" --json number,title,state,labels,milestone,body,url \
      --jq '"# \(.number) \(.title)\n\nState: \(.state)\nLabels: " + ([.labels[].name] | join(", ")) + "\nMilestone: \(.milestone.title // "none")\nURL: \(.url)\n\n" + .body'
    printf '\n\nOpen non-Codex pull requests:\n\n%s\n\n' "$(human_prs || printf unavailable)"
    printf 'Specification may reconcile this issue and its directly related planning documents only. '
    printf 'It must not implement product code, promote lifecycle state, wait for CI, or merge.\n'
  } > "$context"
  build_prompt "$here/routines/specify.md" "$context" "$prompt"
  step "iteration $number | Codex requirements specification for #$issue_number"
  local rc=0
  run_session "$prompt" "$raw" "$final" || rc=$?
  record_usage "$raw"
  return "$rc"
}

verified_specification_pr() {
  local result="$1" expected_issue="$2" reported_issue reported_url reported_sha
  local pr_count pr_json
  reported_issue="$(routine_field "$result" issue_number)"
  reported_url="$(routine_field "$result" pr_url)"
  reported_sha="$(routine_field "$result" commit_sha)"
  [[ "$reported_issue" == "$expected_issue" ]] || return 1
  [[ "$(issue_state_labels "$expected_issue")" == "state:qualified" ]] || return 1
  pr_count="$(automation_pr_count)" || return 1
  [[ "$pr_count" == "1" ]] || return 1
  pr_json="$(automation_pr)" || return 1
  [[ -n "$pr_json" ]] || return 1
  [[ "$(jq -r .url <<< "$pr_json")" == "$reported_url" ]] || return 1
  [[ "$(jq -r .headRefOid <<< "$pr_json")" == "$reported_sha" ]] || return 1
  printf '%s\n' "$pr_json"
}

propose() {
  local number="$1"
  local context="$LOG_DIR/$number-propose.context.md"
  local prompt="$LOG_DIR/$number-propose.prompt.md" raw="$LOG_DIR/$number-propose.jsonl"
  local final="$LOG_DIR/$number-propose.result.json"
  {
    printf '# This run\n\n'
    printf -- '- Repository: `%s`\n' "$repo"
    printf -- '- Default branch: `%s` at `%s`\n' "$DEFAULT_BRANCH" "$(git rev-parse "origin/$DEFAULT_BRANCH")"
    printf -- '- Iteration: `%s`\n' "$number"
    printf -- '- Approved GitHub identity: `%s`\n' "$GITHUB_LOGIN"
    printf -- '- Latest main Validation: %s\n' "$(latest_workflow_line validation.yml)"
    printf -- '- Latest main Compose: %s\n\n' "$(latest_workflow_line compose.yml)"
    printf 'Live Ready issues:\n\n%s\n\n' "$(ready_issues || printf unavailable)"
    printf 'Open non-Codex pull requests:\n\n%s\n\n' "$(human_prs || printf unavailable)"
    printf 'The shell driver owns all CI polling, branch updates, and merging. '
    printf 'Automatic merge is `%s`; admin merge is `%s`. ' "$AUTO_MERGE" "$ADMIN_MERGE"
    printf 'Exit immediately after returning the structured result.\n'
  } > "$context"
  build_prompt "$here/routines/propose.md" "$context" "$prompt"
  step "iteration $number | Codex proposal"
  local rc=0
  run_session "$prompt" "$raw" "$final" || rc=$?
  record_usage "$raw"
  return "$rc"
}

repair() {
  local number="$1" pr_number="$2" repair_number="$3" situation="$4"
  local context="$LOG_DIR/$number-repair-$repair_number.context.md"
  local prompt="$LOG_DIR/$number-repair-$repair_number.prompt.md"
  local raw="$LOG_DIR/$number-repair-$repair_number.jsonl"
  local final="$LOG_DIR/$number-repair-$repair_number.result.json"
  {
    printf '# This run\n\n'
    printf -- '- Repository: `%s`\n' "$repo"
    printf -- '- Approved GitHub identity: `%s`\n' "$GITHUB_LOGIN"
    printf -- '- Pull request: `#%s` (`%s`)\n' "$pr_number" "$(gh pr view "$pr_number" --json url --jq .url)"
    printf -- '- Branch: `%s`\n' "$(gh pr view "$pr_number" --json headRefName --jq .headRefName)"
    printf -- '- Situation: `%s`\n' "$situation"
    printf -- '- Repair budget: `%s/%s`\n\n' "$repair_number" "$MAX_REPAIRS"
    if [[ "$situation" == "ci-red" ]]; then
      printf 'Failing/cancelled checks snapshot:\n\n%s\n\n' "$(failing_checks "$pr_number")"
      printf 'Actions run IDs: `%s`\n\n' "$(failing_run_ids "$pr_number")"
    else
      gh pr view "$pr_number" --json mergeable,mergeStateStatus \
        --jq '"Mergeable: \(.mergeable)\nMerge state: \(.mergeStateStatus)"'
      printf '\n'
    fi
    printf 'The shell driver will re-watch checks after exit. Do not wait or merge.\n'
  } > "$context"
  build_prompt "$here/routines/repair.md" "$context" "$prompt"
  step "iteration $number | Codex repair $repair_number/$MAX_REPAIRS ($situation)"
  local rc=0
  run_session "$prompt" "$raw" "$final" || rc=$?
  record_usage "$raw"
  return "$rc"
}

merge_gate_from_json() {
  jq -er '
    if .mergeable == "CONFLICTING" or .mergeStateStatus == "DIRTY" then
      "merge-conflict"
    elif .reviewDecision == "CHANGES_REQUESTED" then
      "changes-requested"
    elif .reviewDecision == "REVIEW_REQUIRED" then
      "review-required"
    elif .mergeable == "UNKNOWN" or .mergeStateStatus == "UNKNOWN" then
      "merge-state-unknown"
    elif .mergeStateStatus == "BLOCKED" then
      "merge-policy-blocked"
    elif .mergeStateStatus == "BEHIND" then
      "base-behind"
    elif .mergeable == "MERGEABLE" then
      "ready"
    else
      "merge-state-unknown"
    end
  '
}

pr_merge_gate() {
  gh pr view "$1" --json mergeable,mergeStateStatus,reviewDecision \
    | merge_gate_from_json
}

merge_mode_for_gate() {
  case "$1" in
    ready) printf '%s\n' "normal" ;;
    review-required|merge-policy-blocked)
      [[ "$ADMIN_MERGE" == "1" ]] || return 1
      printf '%s\n' "admin"
      ;;
    *) return 1 ;;
  esac
}

land() {
  local pr_number="$1" merge_mode="${2:-normal}"
  local -a args=(pr merge "$pr_number" "--$MERGE_METHOD" --delete-branch)
  case "$merge_mode" in
    normal) ;;
    admin) args+=(--admin) ;;
    *) err "unknown merge mode: $merge_mode"; return 2 ;;
  esac
  gh "${args[@]}"
}

usage() {
  cat <<'EOF'
Usage: dev_loop/loop.sh [--check] [--once] [--help]

  --check  Read-only tool, auth, schema, repository, and active-PR preflight.
  --once   Run one qualification/proposal/PR convergence iteration.
  --help   Show this help.

Configuration is documented in dev_loop/README.md.
EOF
}

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
  return 0
fi

mode=run
while (( $# > 0 )); do
  case "$1" in
    --check) mode=check ;;
    --once) ITERATIONS=1 ;;
    --help|-h) usage; exit 0 ;;
    *) usage >&2; die "unknown argument: $1" ;;
  esac
  shift
done

cd "$repo"
preflight_tools
if [[ "$mode" == "check" ]]; then
  repo_ready_report
  exit $?
fi

acquire_lock
if [[ -f "$STOP_FILE" ]]; then
  warn "removing a leftover STOP file; starting the loop is an explicit go"
  rm -f "$STOP_FILE"
fi
mkdir -p "$LOG_DIR"
printf 'iter\tpr\toutcome\trepairs\twall_s\tinput_tokens\tcached_input_tokens\toutput_tokens\treasoning_output_tokens\n' > "$JOURNAL"

run_start="$(date +%s)"
iteration=0
on_exit() {
  release_lock
  local wall=$(( $(date +%s) - run_start ))
  printf '%srun summary: %dm wall | %s%s\n' "$c_b" "$((wall / 60))" "$JOURNAL" "$c_reset"
}
trap on_exit EXIT
trap 'printf "\n"; warn "interrupted"; exit 130' INT TERM

printf '%sBrowserPane Codex development loop | run %s%s\n' "$c_b" "$RUN_ID" "$c_reset"
say "iterations=$ITERATIONS repairs=$MAX_REPAIRS specification_cycles=$MAX_SPECIFICATION_CYCLES min_free_disk_gb=$MIN_FREE_DISK_GB auto_qualify=$AUTO_QUALIFY auto_merge=$AUTO_MERGE admin_merge=$ADMIN_MERGE logs=dev_loop/runs/$RUN_ID"

specification_cycles=0
while :; do
  if (( ITERATIONS > 0 && iteration >= ITERATIONS )); then
    good "reached $ITERATIONS iteration(s)"
    break
  fi
  stop_requested && break
  iteration=$((iteration + 1))
  n="$(printf '%02d' "$iteration")"
  iter_start="$(date +%s)"
  iter_input=0; iter_cached=0; iter_output=0; iter_reasoning=0
  pr=""; attempt=0

  step "iteration $n | synchronize"
  enforce_iteration_disk_space "synchronization"
  sync_default_branch
  say "$DEFAULT_BRANCH at $(git rev-parse --short "origin/$DEFAULT_BRANCH")"

  pr_json="$(automation_pr || true)"
  if [[ -n "$pr_json" ]]; then
    pr="$(jq -r .number <<< "$pr_json")"
    warn "adopting open Codex PR #$pr"
  else
    next_phase="proposal"
    enforce_iteration_disk_space "qualification"
    qualification_gate_rc=0
    qualification_gate "$n" || qualification_gate_rc=$?
    case "$qualification_gate_rc" in
      0)
        if [[ -n "$QUALIFIED_ISSUE" ]]; then
          good "issue #$QUALIFIED_ISSUE passed qualification and is Ready"
          if stop_requested; then
            record_outcome "qualified-awaiting-proposal"
            exit 0
          fi
        fi
        ;;
      10)
        record_outcome "no-qualification"
        good "no qualification: $QUALIFICATION_REASON"
        break
        ;;
      11)
        record_outcome "halted"
        die "qualification halted: $QUALIFICATION_REASON"
        ;;
      12)
        if ! reserve_specification_cycle; then
          record_outcome "specification-budget-exhausted"
          die "requirements specification budget of $MAX_SPECIFICATION_CYCLES cycle(s) was exhausted in this run"
        fi
        next_phase="specification"
        good "issue #$SPECIFICATION_ISSUE needs bounded requirements specification"
        say "$QUALIFICATION_REASON"
        if stop_requested; then
          record_outcome "qualification-awaiting-specification"
          exit 0
        fi
        ;;
      20)
        record_outcome "qualification-failed"
        die "$QUALIFICATION_REASON"
        ;;
      21)
        record_outcome "qualification-unverified"
        die "$QUALIFICATION_REASON"
        ;;
      *)
        record_outcome "invalid-qualification-status"
        die "$QUALIFICATION_REASON"
        ;;
    esac

    if [[ "$next_phase" == "specification" ]]; then
      enforce_iteration_disk_space "requirements specification"
      specification_rc=0
      specify "$n" "$SPECIFICATION_ISSUE" "$QUALIFICATION_REASON" || specification_rc=$?
      result="$LOG_DIR/$n-specify.result.json"
      if (( specification_rc != 0 )) || ! validate_result "$result"; then
        record_outcome "specification-failed"
        die "specification session failed; inspect $LOG_DIR/$n-specify.jsonl and $result"
      fi
      specification_status="$(routine_field "$result" status)"
      case "$specification_status" in
        SPECIFIED)
          pr_json="$(verified_specification_pr "$result" "$SPECIFICATION_ISSUE" || true)"
          if [[ -z "$pr_json" ]]; then
            record_outcome "specification-unverified"
            die "specification result did not match exactly one Codex PR while issue #$SPECIFICATION_ISSUE remained Qualified"
          fi
          ;;
        HALT)
          record_outcome "halted"
          die "specification halted: $(routine_field "$result" reason)"
          ;;
        *)
          record_outcome "invalid-specification-status"
          die "specification returned unexpected status $specification_status"
          ;;
      esac
      pr="$(jq -r .number <<< "$pr_json")"
    else
      enforce_iteration_disk_space "proposal"
      proposal_rc=0
      propose "$n" || proposal_rc=$?
      result="$LOG_DIR/$n-propose.result.json"
      if (( proposal_rc != 0 )) || ! validate_result "$result"; then
        record_outcome "proposal-failed"
        die "proposal session failed; inspect $LOG_DIR/$n-propose.jsonl and $result"
      fi
      proposal_status="$(routine_field "$result" status)"
      case "$proposal_status" in
        PROPOSED) ;;
        NO_PROPOSAL)
          record_outcome "no-proposal"
          good "no proposal: $(routine_field "$result" reason)"
          break
          ;;
        HALT)
          record_outcome "halted"
          die "proposal halted: $(routine_field "$result" reason)"
          ;;
        *)
          record_outcome "invalid-proposal-status"
          die "proposal returned unexpected status $proposal_status"
          ;;
      esac
      pr_json="$(automation_pr || true)"
      [[ -n "$pr_json" ]] || { record_outcome "missing-pr"; die "proposal reported PROPOSED but no Codex PR exists"; }
      pr="$(jq -r .number <<< "$pr_json")"
    fi
  fi

  pr_url="$(jq -r .url <<< "$pr_json")"
  head_branch="$(jq -r .headRefName <<< "$pr_json")"
  good "tracking PR #$pr | $pr_url"
  if [[ "$(jq -r .isDraft <<< "$pr_json")" == "true" ]]; then
    record_outcome "draft-pr"
    die "PR #$pr is still a draft; inspect the interrupted proposal manually"
  fi

  update_rounds=0
  while :; do
    step "iteration $n | watch PR #$pr"
    check_rc=0
    wait_for_checks "$pr" || check_rc=$?
    situation=""
    if (( check_rc == 3 )); then
      ci_stall_diagnostics "$pr"
      record_outcome "ci-timeout"
      die "checks on #$pr exceeded ${CI_TIMEOUT_SECONDS}s"
    fi
    if (( check_rc == 0 )); then
      good "checks on #$pr are green"
      if up_to_date_with_main "$head_branch"; then
        if [[ "$AUTO_MERGE" == "0" ]]; then
          record_outcome "green-awaiting-review"
          good "#${pr} is green and current; AUTO_MERGE=0 leaves it open for review"
          exit 0
        fi
        if ! merge_gate="$(pr_merge_gate "$pr")"; then
          record_outcome "merge-state-unavailable"
          die "could not read the live merge gate for #$pr"
        fi
        merge_mode="$(merge_mode_for_gate "$merge_gate" || true)"
        if [[ -n "$merge_mode" ]]; then
          if [[ "$merge_mode" == "admin" ]]; then
            step "iteration $n | admin merge PR #$pr"
          else
            step "iteration $n | merge PR #$pr"
          fi
          required_post_merge_workflows="$(post_merge_workflows_for_pr "$pr")"
          if land "$pr" "$merge_mode"; then
            merge_sha="$(gh pr view "$pr" --json mergeCommit --jq '.mergeCommit.oid // empty')"
            [[ -n "$merge_sha" ]] || { record_outcome "merge-sha-missing"; die "#${pr} merged but its merge SHA is unavailable"; }
            good "#${pr} merged at ${merge_sha:0:12}; validating published main"
            post_merge_rc=0
            wait_for_post_merge_workflows "$merge_sha" "$required_post_merge_workflows" || post_merge_rc=$?
            if (( post_merge_rc != 0 )); then
              record_outcome "post-merge-failed"
              die "post-merge workflows did not pass for $merge_sha (status $post_merge_rc)"
            fi
            record_outcome "landed"
            good "#${pr} merged and post-merge workflows passed"
            break
          fi
          if ! merge_gate="$(pr_merge_gate "$pr")"; then
            record_outcome "merge-state-unavailable"
            die "merge command failed and the live merge gate for #$pr is unavailable"
          fi
        fi
        case "$merge_gate" in
          merge-conflict)
            situation="merge-conflict"
            ;;
          review-required|changes-requested|merge-policy-blocked)
            if [[ "$merge_mode" == "admin" ]]; then
              record_outcome "admin-merge-failed"
              die "admin merge failed for #$pr and GitHub still reports $merge_gate"
            fi
            record_outcome "$merge_gate"
            good "#${pr} is green and current but stopped by $merge_gate; leaving it open"
            exit 0
            ;;
          base-behind)
            record_outcome "base-state-stale"
            die "GitHub reports #$pr behind even though origin/$DEFAULT_BRANCH is an ancestor; rerun after merge state settles"
            ;;
          ready)
            if [[ "$merge_mode" == "admin" ]]; then
              record_outcome "admin-merge-failed"
              die "admin merge command failed for #$pr although its refreshed merge gate is ready"
            fi
            record_outcome "merge-command-failed"
            die "merge command failed for #$pr although its refreshed merge gate is ready"
            ;;
          *)
            record_outcome "merge-state-unknown"
            die "GitHub returned an unknown merge gate for #$pr"
            ;;
        esac
      else
        if (( update_rounds >= MAX_UPDATE_BRANCH )); then
          record_outcome "base-keeps-moving"
          die "$DEFAULT_BRANCH moved under #$pr $MAX_UPDATE_BRANCH times"
        fi
        update_rounds=$((update_rounds + 1))
        old_sha="$(pr_head_oid "$pr")"
        say "updating #$pr with current $DEFAULT_BRANCH ($update_rounds/$MAX_UPDATE_BRANCH)"
        if gh pr update-branch "$pr" >/dev/null 2>&1; then
          settle_after_change "$pr" "$old_sha"
          continue
        fi
        if ! merge_gate="$(pr_merge_gate "$pr")"; then
          record_outcome "merge-state-unavailable"
          die "branch update failed and the live merge gate for #$pr is unavailable"
        fi
        case "$merge_gate" in
          merge-conflict)
            situation="merge-conflict"
            ;;
          review-required|changes-requested|merge-policy-blocked)
            record_outcome "$merge_gate"
            good "#${pr} branch update is stopped by $merge_gate; leaving it open"
            exit 0
            ;;
          *)
            record_outcome "update-branch-failed"
            die "could not update #$pr from origin/$DEFAULT_BRANCH (merge gate: $merge_gate)"
            ;;
        esac
      fi
    else
      situation="ci-red"
    fi

    if (( attempt >= MAX_REPAIRS )); then
      record_outcome "repairs-exhausted"
      die "repair budget exhausted for #$pr"
    fi
    stop_requested && exit 0
    attempt=$((attempt + 1))
    enforce_iteration_disk_space "repair"
    old_sha="$(pr_head_oid "$pr")"
    repair_rc=0
    repair "$n" "$pr" "$attempt" "$situation" || repair_rc=$?
    repair_result="$LOG_DIR/$n-repair-$attempt.result.json"
    if (( repair_rc != 0 )) || ! validate_result "$repair_result"; then
      record_outcome "repair-failed"
      die "repair session failed; inspect $repair_result"
    fi
    repair_status="$(routine_field "$repair_result" status)"
    case "$repair_status" in
      REPAIRED|RERUN_ONLY) settle_after_change "$pr" "$old_sha" ;;
      HALT)
        record_outcome "halted"
        die "repair halted: $(routine_field "$repair_result" reason)"
        ;;
      *)
        record_outcome "invalid-repair-status"
        die "repair returned unexpected status $repair_status"
        ;;
    esac
  done

  git switch "$DEFAULT_BRANCH" >/dev/null 2>&1 || die "could not return to $DEFAULT_BRANCH"
  sync_default_branch
done

good "loop finished after $iteration iteration(s)"
