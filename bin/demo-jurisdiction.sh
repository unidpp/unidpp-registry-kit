#!/usr/bin/env bash
# demo-jurisdiction.sh — the worked example: a fictional "DE" registry,
# end to end, against the real API.
#
# Part of unidpp-registry-kit (memo work item R3). This script runs the
# full lifecycle a jurisdictional registry peer goes through:
#
#   [1] bring up the registry (persistent journal, guarded admin)
#   [2] seed jurisdiction DE (signed C3 self-descriptor, jurisdiction
#       profile, applicability binding, sample data elements)
#   [3] federation discovery — who serves the DE registry? (C3)
#   [4] as-of applicability — which profiles bind product X at time T?
#   [5] supersession — versioning a data element (19135 discipline)
#   [6] enumeration resistance — what this registry does NOT expose
#
# Re-runnable: every step is idempotent (409s are tolerated, bindings
# are checked before creation).
#
# Usage:  bin/demo-jurisdiction.sh [--jurisdiction DE]
set -euo pipefail

KIT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JUR="${2:-DE}"
[[ "${1:-}" == "--jurisdiction" || -z "${1:-}" ]] || { echo "usage: demo-jurisdiction.sh [--jurisdiction DE]" >&2; exit 1; }

BASE="http://127.0.0.1:${KIT_PORT:-8391}"
TOKEN_FILE="$KIT_ROOT/data/admin-token"
TOKEN="$(cat "$TOKEN_FILE" 2>/dev/null || true)"
AUTH=(-H "Authorization: Bearer $TOKEN")
PRODUCT_TYPE="gtin:4260123400019"
ELEMENT="urn:unidpp:de:battery-carbon-footprint"

hr() { echo; echo "======================================================================"; echo "$1"; echo "======================================================================"; }
show() { echo "\$ $1"; }

hr "[1] bring up the registry"
"$KIT_ROOT/bin/run-registry.sh" start --daemon

hr "[2] seed jurisdiction $JUR"
python3 "$KIT_ROOT/bin/seed-jurisdiction.py" --jurisdiction "$JUR"

hr "[3] federation discovery: who serves the $JUR registry? (C3)"
show "curl -s '$BASE/services?jurisdiction=$JUR&class=registry' | jq"
curl -sf "$BASE/services?jurisdiction=$JUR&class=registry" \
  | jq '.services[0] | {identifier,
         class: .version.body.class, jurisdiction: .version.body.jurisdiction,
         endpoints: .version.body.endpoints, operator: .version.body.operator.id,
         signed: (.version.signature.algorithm + "/" + .version.signature.key_id)}'

hr "[4] as-of applicability: which profiles bind $PRODUCT_TYPE at time T?"
echo "-- at 2026-06-01 (before the obligation starts):"
show "curl -s '$BASE/applicability?product_type=$PRODUCT_TYPE&at=2026-06-01T00:00:00Z'"
curl -sf "$BASE/applicability?product_type=$PRODUCT_TYPE&at=2026-06-01T00:00:00Z" \
  | jq '{at: .as_of, applies: (.applicability | length)}'
echo "-- at 2027-06-01 (the binding is in force):"
show "curl -s '$BASE/applicability?product_type=$PRODUCT_TYPE&at=2027-06-01T00:00:00Z'"
curl -sf "$BASE/applicability?product_type=$PRODUCT_TYPE&at=2027-06-01T00:00:00Z" \
  | jq '{at: .as_of,
         applies: [.applicability[] | {profile: .binding.profile_item,
                                       from: .binding.effective_from,
                                       retroactive: .binding.retroactive}]}'

hr "[5] supersession: versioning a data element (19135 discipline)"
echo "-- current registered version (v1.0.0 on first run; v2.0.0 on re-runs):"
show "curl -s '$BASE/data-elements/$ELEMENT' | jq '.version'"
curl -sf "$BASE/data-elements/$ELEMENT" | jq '.version | {version, status, effective_from}'
echo "-- register v2.0.0, effective 2028-01-01, with reason:"
show "curl -XPOST '$BASE/data-elements/$ELEMENT/versions' -d '{\"version\": \"2.0.0\", ...}'"
SUPRESP="$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/data-elements/$ELEMENT/versions" \
  "${AUTH[@]}" -H 'content-type: application/json' -d '{
    "version": "2.0.0",
    "reason": "align representation with ESDC data-space conventions; pin unit identity",
    "effective_from": "2028-01-01T00:00:00Z"
  }')"
if [[ "$SUPRESP" == "201" ]]; then
  echo "   201 Created — old version superseded"
elif [[ "$SUPRESP" == "409" ]]; then
  echo "   409 — v2.0.0 already registered (idempotent re-run)"
else
  echo "   unexpected HTTP $SUPRESP"; exit 1
fi
echo "-- as-of 2027-06-01: v1.0.0 still in force, window end derived from successor:"
show "curl -s '$BASE/data-elements/$ELEMENT?at=2027-06-01T00:00:00Z' | jq .version"
curl -sf "$BASE/data-elements/$ELEMENT?at=2027-06-01T00:00:00Z" \
  | jq '.version | {version, status, superseded_by_version, window_end}'
echo "-- as-of 2028-06-01: v2.0.0 in force:"
curl -sf "$BASE/data-elements/$ELEMENT?at=2028-06-01T00:00:00Z" \
  | jq '.version | {version, status, effective_from}'
echo "-- the supersession chain:"
show "curl -s '$BASE/data-elements/$ELEMENT/supersession' | jq .chain"
curl -sf "$BASE/data-elements/$ELEMENT/supersession" \
  | jq '{chain: [.chain[] | {version, status, superseded_by_version, window_end}]}'

hr "[6] enumeration resistance: what this registry does NOT expose"
cat <<'POSTURE'
  The registry holds DESCRIPTORS (definitions, profiles, service
  descriptors) and identity-keyed BINDINGS. It never holds records of
  things, and it exposes no surface for walking the installed base:

  - /applicability is identity-keyed: you must already hold a product
    identity to ask what applies to it. There is no "list all bound
    identities" endpoint. (I12: nobody enumerates the installed base.)
  - /items and the subregisters list REGISTERED DEFINITIONS (public,
    19135 items: data elements, profiles, units) — never identifiers
    in use.
  - The audit log (/admin/log) is Bearer-guarded operator evidence,
    not a query index over the installed base.
  - No passport payloads are stored anywhere in the stack (deposit, if
    a jurisdiction legislates it, is notarized Tier-C snapshots at a
    national archivist — authority never transfers with the copy, I7).

  Demonstrations:
POSTURE
echo "-- a) query applicability WITHOUT a product identity:"
show "curl -s '$BASE/applicability'"
curl -s "$BASE/applicability" -o /dev/null -w '   HTTP %{http_code}\n'
curl -s "$BASE/applicability" | head -c 200; echo
echo "-- b) read the audit log WITHOUT the operator credential:"
show "curl -s '$BASE/admin/log'   (no Authorization header)"
curl -s "$BASE/admin/log" -o /dev/null -w '   HTTP %{http_code}\n'
echo "-- c) the complete public route table (GET /) — every surface offered:"
show "curl -s '$BASE/' | jq -r '.endpoints[]'"
curl -sf "$BASE/" | jq -r '.endpoints[]' | sed 's/^/   /'
echo "   Zero endpoints list identifiers-in-use; the only product-identity"
echo "   read (/applicability) requires the identity as input. /items lists"
echo "   registered DEFINITIONS (public 19135 items), never records of things."

hr "done — registry still running on $BASE (bin/run-registry.sh stop to stop)"
