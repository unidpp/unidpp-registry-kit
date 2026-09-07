#!/usr/bin/env bash
# run-registry.sh — the jurisdictional-registry launcher.
#
# Part of unidpp-registry-kit: "your country's DPP registry in a box"
# (memo work item R3). Builds and runs the unidpp-registry service
# (path dependency ../unidpp-registry) as a federation peer:
#
#   - configurable listen port, persistent append-only journal
#   - admin mutations always Bearer-guarded (token generated once,
#     stored under data/admin-token, mode 600)
#   - base discovery dataset seeded through the API (POST /admin/seed)
#   - optional cloudflared quick tunnel for a public URL (see --tunnel;
#     a tunnel is documentation-grade convenience, never a requirement)
#
# Usage:
#   bin/run-registry.sh                 # run in the foreground (Ctrl-C stops)
#   bin/run-registry.sh --daemon        # run in the background (data/registry.pid)
#   bin/run-registry.sh --tunnel        # daemonized + cloudflared quick tunnel
#   bin/run-registry.sh stop            # stop a --daemon instance
#   bin/run-registry.sh status          # healthz + journal + service count
#
# Configuration (environment):
#   KIT_PORT            listen port            (default 8391; the UniDPP
#                       pilot registry lives on 8390 — pick your own)
#   KIT_BIND            listen address         (default 127.0.0.1)
#   KIT_HOME            runtime state dir      (default <kit>/data)
#   KIT_REGISTRY_DIR    unidpp-registry source (default <kit>/../unidpp-registry)
#   KIT_FORCE_BUILD     1 = rebuild even if the binary exists
#   KIT_ADMIN_TOKEN     admin Bearer token     (default: generated once,
#                       persisted in $KIT_HOME/admin-token)
#
# The journal is append-only and never removed by this script: stopping
# and restarting replays it (durability and auditability are the same
# mechanism — see the unidpp-registry README, "Storage choice").
set -euo pipefail

KIT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KIT_HOME="${KIT_HOME:-$KIT_ROOT/data}"
KIT_PORT="${KIT_PORT:-8391}"
KIT_BIND="${KIT_BIND:-127.0.0.1}"
KIT_REGISTRY_DIR="${KIT_REGISTRY_DIR:-$KIT_ROOT/../unidpp-registry}"
BIN="$KIT_REGISTRY_DIR/target/release/unidpp-registry"
JOURNAL="$KIT_HOME/jurisdiction-journal.jsonl"
LOG="$KIT_HOME/registry.log"
PIDFILE="$KIT_HOME/registry.pid"
TOKEN_FILE="$KIT_HOME/admin-token"
URL="http://$KIT_BIND:$KIT_PORT"

mkdir -p "$KIT_HOME"

die() { echo "run-registry.sh: $*" >&2; exit 1; }

healthz() { curl -sf -m 2 "$URL/healthz" >/dev/null 2>&1; }

ensure_admin_token() {
  # Always guard admin mutations: a jurisdictional registry is a peer
  # that other peers audit, not an open dev service. The token is
  # generated once and reused so journal history stays attributable to
  # one operator credential across restarts.
  if [[ -n "${KIT_ADMIN_TOKEN:-}" ]]; then return; fi
  if [[ ! -s "$TOKEN_FILE" ]]; then
    ( umask 077
      openssl rand -hex 24 > "$TOKEN_FILE" 2>/dev/null \
        || head -c 24 /dev/urandom | xxd -p -c 48 > "$TOKEN_FILE" )
  fi
  [[ -s "$TOKEN_FILE" ]] || die "cannot create admin token at $TOKEN_FILE"
  chmod 600 "$TOKEN_FILE" 2>/dev/null || true
  KIT_ADMIN_TOKEN="$(cat "$TOKEN_FILE")"
  export KIT_ADMIN_TOKEN
}

ensure_binary() {
  if [[ -x "$BIN" && "${KIT_FORCE_BUILD:-0}" != "1" ]]; then return; fi
  command -v cargo >/dev/null 2>&1 \
    || die "cargo not found and no prebuilt binary at $BIN"
  echo "==> building unidpp-registry (release) in $KIT_REGISTRY_DIR"
  (cd "$KIT_REGISTRY_DIR" && cargo build --release)
  [[ -x "$BIN" ]] || die "build finished but $BIN is missing"
}

wait_healthy() {
  local i
  for i in $(seq 1 50); do
    healthz && return 0
    sleep 0.2
  done
  die "registry did not become healthy on $URL (see $LOG)"
}

seed_base_dataset() {
  # The base dataset (UniDPP's own C3/C4/C5 items + C1 units) is what a
  # jurisdictional descriptor references: protocol bindings and units.
  # Idempotent per process — but on a replayed journal the records
  # already exist and re-running would conflict, so check first: units
  # only enter the store through this seed.
  local units
  units="$(curl -s -m 5 "$URL/units" | jq -r '.items | length' 2>/dev/null || echo 0)"
  if [[ "${units:-0}" != "0" ]]; then
    echo "==> base discovery dataset already present (journal replay; $units units)"
    return 0
  fi
  echo "==> seeding base discovery dataset via the API"
  local resp
  resp="$(curl -s -m 15 -X POST "$URL/admin/seed" \
    -H "Authorization: Bearer $KIT_ADMIN_TOKEN" || true)"
  echo "$resp" | head -c 400; echo
}

start_daemon() {
  ensure_binary
  ensure_admin_token
  if healthz; then
    echo "==> a registry is already listening on $URL (reusing it)"
  else
    echo "==> starting unidpp-registry (daemon) on $URL"
    echo "    journal: $JOURNAL"
    echo "    log:     $LOG"
    # `nohup env … "$BIN"` exec-chains into the server, so $! is the
    # server's own pid (what the pid file must hold for `stop`).
    nohup env UNIDPP_REGISTRY_BIND="$KIT_BIND:$KIT_PORT" \
      UNIDPP_REGISTRY_STATE_FILE="$JOURNAL" \
      UNIDPP_REGISTRY_ADMIN_TOKEN="$KIT_ADMIN_TOKEN" \
      "$BIN" >> "$LOG" 2>&1 &
    echo $! > "$PIDFILE"
    wait_healthy
  fi
  seed_base_dataset
  echo "==> ready. Admin token: $TOKEN_FILE — e.g.:"
  echo "      curl -H \"Authorization: Bearer \$(cat $TOKEN_FILE)\" '$URL/items'"
}

start_foreground() {
  ensure_binary
  ensure_admin_token
  if healthz; then
    echo "==> a registry is already listening on $URL (reusing it)"
    seed_base_dataset
    exit 0
  fi
  # Seed over the API just after the server comes up, then hand the
  # terminal to the server process (exec: signals flow straight through).
  ( for i in $(seq 1 50); do
      healthz && break
      sleep 0.2
    done
    seed_base_dataset >/dev/null 2>&1 || true ) &
  echo "==> starting unidpp-registry on $URL (foreground; Ctrl-C stops)"
  echo "    journal: $JOURNAL"
  cd "$KIT_HOME"
  exec env UNIDPP_REGISTRY_BIND="$KIT_BIND:$KIT_PORT" \
    UNIDPP_REGISTRY_STATE_FILE="$JOURNAL" \
    UNIDPP_REGISTRY_ADMIN_TOKEN="$KIT_ADMIN_TOKEN" \
    "$BIN"
}

start_tunnel() {
  command -v cloudflared >/dev/null 2>&1 || die "--tunnel requested but cloudflared is not installed (https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)"
  local tlog="$KIT_HOME/tunnel.log" tpid="$KIT_HOME/tunnel.pid" pub=""
  echo "==> starting cloudflared quick tunnel -> $URL (log: $tlog)"
  nohup cloudflared tunnel --url "$URL" > "$tlog" 2>&1 &
  echo $! > "$tpid"
  local i pub=""
  for i in $(seq 1 40); do
    # The assigned hostname is a hyphenated subdomain; exclude
    # api.trycloudflare.com (cloudflared's own endpoint, which also
    # appears in the log, including in error messages).
    pub="$(grep -o 'https://[a-z0-9][a-z0-9-]*\.trycloudflare\.com' "$tlog" 2>/dev/null \
      | grep -v '^https://api\.trycloudflare\.com$' | head -1 || true)"
    [[ -n "$pub" ]] && kill -0 "$(cat "$tpid")" 2>/dev/null && break
    pub=""
    sleep 0.5
  done
  if [[ -n "$pub" ]]; then
    echo "    public URL: $pub"
    echo "$pub" > "$KIT_HOME/PUBLIC_URL"
  else
    echo "    (no URL captured yet — check $tlog)"
  fi
  cat <<'NOTE'
    NOTE: a quick tunnel is ephemeral (the URL changes on restart) and is
    for demos only. A jurisdictional registry publishes a stable hostname
    through a named tunnel or its own ingress, e.g.:

      cloudflared tunnel create jurisdiction-de
      cloudflared tunnel route dns jurisdiction-de registry.example.org
      cloudflared tunnel run --token <token> --url http://127.0.0.1:8391

    Nothing in the kit or the federation protocol requires a tunnel:
    peers verify signatures and as-of semantics, not hosting location.
NOTE
}

cmd_stop() {
  if [[ -f "$PIDFILE" ]]; then
    local pid; pid="$(cat "$PIDFILE")"
    if kill "$pid" 2>/dev/null; then
      echo "==> stopped registry (pid $pid); journal preserved at $JOURNAL"
    else
      echo "==> pid $pid not running (stale pid file?)"
    fi
    rm -f "$PIDFILE"
  else
    echo "==> no pid file at $PIDFILE (foreground instances stop with Ctrl-C)"
  fi
  if [[ -f "$KIT_HOME/tunnel.pid" ]] && kill "$(cat "$KIT_HOME/tunnel.pid")" 2>/dev/null; then
    rm -f "$KIT_HOME/tunnel.pid"
    echo "==> stopped cloudflared tunnel"
  fi
  exit 0
}

cmd_status() {
  if healthz; then
    echo "healthy:     $URL/healthz"
    echo "journal:     $JOURNAL ($(wc -l < "$JOURNAL" 2>/dev/null | tr -d ' ' || echo 0) records)"
    echo "admin token: $TOKEN_FILE"
    curl -sf "$URL/services" \
      -H "Authorization: Bearer $(cat "$TOKEN_FILE" 2>/dev/null)" 2>/dev/null \
      | jq -r '"services registered: \(.count)"' 2>/dev/null || true
  else
    echo "not running on $URL"
    exit 1
  fi
}

case "${1:-start}" in
  start)
    shift || true
    daemon=0; tunnel=0
    for arg in "$@"; do
      case "$arg" in
        --daemon) daemon=1 ;;
        --tunnel) tunnel=1 ;;
        *) die "unknown option: $arg (expected --daemon, --tunnel)" ;;
      esac
    done
    if [[ "$daemon" == "1" || "$tunnel" == "1" ]]; then
      start_daemon
      [[ "$tunnel" == "1" ]] && start_tunnel
      if [[ "$tunnel" == "1" ]]; then
        echo "==> streaming registry log (Ctrl-C detaches; server keeps running)"
        tail -f "$LOG" || true
      fi
    else
      start_foreground
    fi
    ;;
  stop)   shift || true; cmd_stop ;;
  status) shift || true; cmd_status ;;
  *) die "usage: run-registry.sh [start [--daemon] [--tunnel]] | stop | status" ;;
esac
