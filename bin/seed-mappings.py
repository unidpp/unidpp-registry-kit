#!/usr/bin/env python3
"""seed-mappings.py — register the GB 4943.1-2022 ↔ IEC 62368-1
equivalence as a cross-register mapping item.

Part of unidpp-registry-kit (memo follow-up to T-07 / item 57): the
registry's cross-register-mapping item class had an integration-test
instance only, while the pilot seed still registered the equivalence
as a bare `transform` item (unidpp-pilot-data
seed/items/equiv-gb4943-iec62368.json). This script re-deposits the
same claim through the dedicated item class, referentially intact:

  gb-4943-1   register `gb-std`, class data-element, version 2022
  iec-62368-1 register `iec`,    class data-element, version 2018
  urn:unidpp:map:gb4943-iec62368
              register `unidpp`, class cross-register-mapping, with
              the manifest shape of unidpp-registry src/mapping.rs:

    { "version": "1.0.0",
      "source": {"register": "gb-std", "item": "gb-4943-1",
                 "version": "2022"},
      "target": {"register": "iec", "item": "iec-62368-1"},
      "mapping_type": "equivalent",
      "attester": "CQC-pattern notified body" }

Both endpoints are registered first (idempotently, when absent) —
the registry's `cross-register-mapping-integrity` intake check
rejects a mapping whose ends do not resolve to registered items
(register attribute matching, version pins existing). The source pin
is the GB designation year (2022); the target is unpinned (any
edition of IEC 62368-1), mirroring the registry's own fixture for
this mapping. The two standards ride the `data-element` class (the
registry's item classes are fixed; its integration fixture for this
mapping uses the same class for the endpoints).

No `evidence_ref` is deposited: the claim carries the attester only
— the kit does not fabricate a stable public citation for the GB/IEC
correspondence. Add one through the registry API when your
jurisdiction holds the evidence document.

Idempotency: re-running against the same journal is safe — items
already registered (HTTP 409) are reported and skipped (the same
409-tolerance as the UNTDED and units seeders). Afterwards the
script verifies through the directional lookup
(`GET /cross-register-mappings?item=…&source=…&target=…`) that the
mapping resolves from both directions and by the named sides.

Usage:
  bin/seed-mappings.py                     # against 127.0.0.1:8391
  bin/seed-mappings.py --port 8390         # against the pilot registry
  bin/seed-mappings.py --register unidpp   # mapping item's register

Requires: Python 3.9+ (stdlib only).
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

REGISTER_DEFAULT = "unidpp"  # cross-register mappings are governance-shaped
SUBMITTING_ORG = "unidpp-registry-kit — GB↔IEC equivalence seed (cross-register-mapping class)"
MAPPING_VERSION = "1.0.0"
MAPPING_EFFECTIVE_FROM = "2026-09-01T00:00:00Z"  # the pilot claim's window

MAPPING_ID = "urn:unidpp:map:gb4943-iec62368"

# The two endpoint items (both must exist for the mapping's
# referential-integrity check). Standard designations as data-element
# items, versions = the designation years.
ENDPOINTS = [
    {
        "register_id": "gb-std",
        "item_id": "gb-4943-1",
        "class": "data-element",
        "definition": "GB 4943.1-2022 — audio/video, information and "
                      "communication technology equipment, part 1: safety "
                      "requirements (Chinese national standard)",
        "version": "2022",
        "submitting_organization": "SAC (Standardization Administration of China)",
        "manifest": {
            "version": "2022",
            "designation": "GB 4943.1-2022",
            "name": "Audio/video, information and communication technology "
                    "equipment — Part 1: Safety requirements",
        },
    },
    {
        "register_id": "iec",
        "item_id": "iec-62368-1",
        "class": "data-element",
        "definition": "IEC 62368-1 — audio/video, information and "
                      "communication technology equipment, part 1: safety "
                      "requirements (international standard)",
        "version": "2018",
        "submitting_organization": "IEC/TC 108",
        "manifest": {
            "version": "2018",
            "designation": "IEC 62368-1:2018",
            "name": "Audio/video, information and communication technology "
                    "equipment — Part 1: Safety requirements",
        },
    },
]

MAPPING_DEFINITION = ("GB 4943.1-2022 corresponds to IEC 62368-1 for the "
                      "purposes of charger conformity evidence")


class Registry:
    def __init__(self, base_url: str, token: str):
        self.base = base_url.rstrip("/")
        self.token = token

    def _request(self, method: str, path: str, body=None):
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(self.base + path, data=data, method=method)
        req.add_header("content-type", "application/json")
        if self.token:
            req.add_header("authorization", "Bearer " + self.token)
        try:
            with urllib.request.urlopen(req, timeout=15) as resp:
                raw = resp.read()
                try:
                    return resp.status, json.loads(raw or b"{}")
                except ValueError:
                    return resp.status, {"body": raw.decode("utf-8", "replace")}
        except urllib.error.HTTPError as e:
            try:
                detail = json.loads(e.read() or b"{}")
            except Exception:
                detail = {}
            return e.code, detail

    def get(self, path: str):
        return self._request("GET", path)

    def post(self, path: str, body: dict):
        return self._request("POST", path, body)


def ensure_item(api: Registry, path: str, body: dict) -> str:
    """Register one item (409-tolerant, journal-safe)."""
    status, resp = api.post(path, body)
    if status == 201:
        return "registered %s (audit_seq %s)" % (body["item_id"], resp.get("audit_seq"))
    if status == 409:
        return "already registered %s" % body["item_id"]
    raise SystemExit("item `%s` failed (HTTP %d): %s"
                     % (body["item_id"], status, json.dumps(resp)[:400]))


def ensure_mapping(api: Registry, register: str) -> str:
    """Register the cross-register mapping through the dedicated surface."""
    body = {
        "register_id": register,
        "item_id": MAPPING_ID,
        "definition": MAPPING_DEFINITION,
        "version": MAPPING_VERSION,
        "effective_from": MAPPING_EFFECTIVE_FROM,
        "submitting_organization": SUBMITTING_ORG,
        "manifest": {
            "version": MAPPING_VERSION,
            "source": {"register": "gb-std", "item": "gb-4943-1", "version": "2022"},
            "target": {"register": "iec", "item": "iec-62368-1"},
            "mapping_type": "equivalent",
            "attester": "CQC-pattern notified body",
        },
    }
    return ensure_item(api, "/cross-register-mappings", body)


def lookup(api: Registry, query: str):
    """One directional lookup (item= either side, or source=/target=)."""
    status, resp = api.get("/cross-register-mappings?%s" % query)
    if status != 200:
        raise SystemExit("lookup failed: /cross-register-mappings?%s -> HTTP %d"
                         % (query, status))
    return resp


def verify(api: Registry) -> bool:
    """The mapping resolves from both directions and by the named sides."""
    ok = True

    def check(query: str, want: int) -> None:
        nonlocal ok
        resp = lookup(api, query)
        mappings = resp.get("mappings", [])
        got = [m.get("identifier") for m in mappings]
        good = len(mappings) == want and (want == 0 or MAPPING_ID in got)
        ok = ok and good
        print("  %-42s count=%d %s" % (query, len(mappings),
                                       "OK" if good else "FAILED"))

    print("verify: directional lookups of %s" % MAPPING_ID)
    check("item=gb-4943-1", 1)      # either direction, from the GB side
    check("item=iec-62368-1", 1)    # either direction, from the IEC side
    check("source=gb-4943-1&target=iec-62368-1", 1)  # the named sides
    check("target=gb-4943-1", 0)    # gb-4943-1 is only ever a source here

    # the item itself resolves with the attested equivalence manifest
    status, resp = api.get("/cross-register-mappings/%s" % MAPPING_ID)
    if status != 200:
        print("  GET item -> HTTP %d FAILED" % status)
        return False
    manifest = resp.get("manifest") or {}
    attested = (manifest.get("mapping_type") == "equivalent"
                and manifest.get("attester") == "CQC-pattern notified body")
    print("  %-42s %s" % ("item manifest (equivalent, attested)",
                          "OK" if attested else "FAILED"))
    return ok and attested


def main():
    kit_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=int,
                    default=int(os.environ.get("KIT_PORT", "8391")))
    ap.add_argument("--bind", default=os.environ.get("KIT_BIND", "127.0.0.1"))
    ap.add_argument("--base-url", default=None,
                    help="default http://$BIND:$PORT")
    ap.add_argument("--admin-token", default=os.environ.get("KIT_ADMIN_TOKEN"))
    ap.add_argument("--token-file", default=os.path.join(
        os.environ.get("KIT_HOME", os.path.join(kit_root, "data")),
        "admin-token"))
    ap.add_argument("--register", default=REGISTER_DEFAULT,
                    help="register id for the mapping item (default: unidpp)")
    args = ap.parse_args()

    base = args.base_url or ("http://%s:%d" % (args.bind, args.port))
    token = args.admin_token
    if not token and os.path.isfile(args.token_file):
        with open(args.token_file) as f:
            token = f.read().strip()
    api = Registry(base, token or "")

    status, _ = api.get("/healthz")
    if status != 200:
        sys.exit("no healthy registry at %s — run bin/run-registry.sh first" % base)

    print("unidpp-registry-kit: seeding GB 4943.1-2022 ↔ IEC 62368-1 as a "
          "cross-register-mapping item into %s (register `%s`)"
          % (base, args.register))
    print()

    # 1. referential integrity: both endpoints exist (idempotent)
    for endpoint in ENDPOINTS:
        print("  endpoint %s: %s" % (
            endpoint["item_id"],
            ensure_item(api, "/data-elements", endpoint)))
    # 2. the mapping itself, through the dedicated surface
    print("  mapping   %s: %s" % (MAPPING_ID, ensure_mapping(api, args.register)))
    print()

    # 3. verify through the directional lookup
    if not verify(api):
        sys.exit(1)

    print()
    print("next:")
    print("  curl -s '%s/cross-register-mappings?item=iec-62368-1' | jq" % base)
    print("  curl -s '%s/cross-register-mappings/%s' | jq '.manifest'" % (base, MAPPING_ID))


if __name__ == "__main__":
    main()
