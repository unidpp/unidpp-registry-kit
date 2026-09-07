#!/usr/bin/env python3
"""seed-jurisdiction.py — register a jurisdiction's registry as a federation peer.

Part of unidpp-registry-kit (memo work item R3): parameterized seeding of a
national/jurisdictional DPP registry deployment. Given a two-letter
jurisdiction code, this script registers through the unidpp-registry HTTP
API (see ../unidpp-registry/README.md):

  1. the C3 self-descriptor  — a *signed* service descriptor, class
     `registry`, jurisdiction=<code>: how other peers discover and bind
     to this registry (PLAN-OPERATORS §1.1 row C3, §3 tier T2);
  2. the jurisdiction profile item — the jurisdictional DPP profile as a
     versioned 19135 item in the national subregister (memo F4), with a
     dated applicability binding to a sample product type;
  3. sample data-element items — the semantic seeds (memo F4 / C1) the
     profile's data points reference.

Signing: the C3 descriptor is signed with an Ed25519 key deterministically
derived from an operator label (SHA-256("UNIDPP-DISCOVERY/OPERATOR-SEED"
|| label)) — the unidpp-registry dev keyring scheme. The default label
`unidpp-registry` is the dev stand-in; in production the jurisdiction's
operator credential comes from the trust service (ONBOARDING.md step 1)
and the keyring seam (`AppState::keyring`) is where it plugs in.

Canonical JSON: the signature covers the descriptor body serialized the
way serde_json emits it (BTreeMap key order — i.e. sorted keys — compact
separators, UTF-8 passthrough). `canonical()` reproduces that byte form.

Idempotency: re-running against the same journal is safe. Already
registered items/services are skipped on 409; the applicability binding
is only created if the profile does not already apply to the subject.

Usage:
  bin/seed-jurisdiction.py --jurisdiction DE
  bin/seed-jurisdiction.py --jurisdiction JP --port 8392

Requires: Python 3.9+, `cryptography` (Ed25519).
"""

import argparse
import hashlib
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

try:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
    from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
except ImportError:  # pragma: no cover
    sys.exit("seed-jurisdiction.py: the `cryptography` package is required (pip install cryptography)")

OPERATOR_SEED_DOMAIN = b"UNIDPP-DISCOVERY/OPERATOR-SEED"
DEFAULT_OPERATOR_LABEL = "unidpp-registry"
DEFAULT_PROTOCOL_BINDING = "pb-en18222-rest"  # seeded by POST /admin/seed

# Illustrative residency classes; not normative. A jurisdiction may use
# any string its profile defines.
RESIDENCY_BY_JURISDICTION = {
    "DE": "eu", "FR": "eu", "NL": "eu", "IT": "eu", "ES": "eu",
    "SE": "eu", "PL": "eu", "GB": "eu", "CH": "eu", "NO": "eu",
    "JP": "apac", "KR": "apac", "CN": "cn", "US": "na", "CA": "na",
}


def canonical(obj) -> bytes:
    """serde_json byte form: sorted keys, compact separators, raw UTF-8."""
    return json.dumps(obj, sort_keys=True, separators=(",", ":"),
                      ensure_ascii=False).encode("utf-8")


class Operator:
    """A deterministic dev operator key (the unidpp-registry keyring scheme)."""

    def __init__(self, label: str):
        self.label = label
        seed = hashlib.sha256(OPERATOR_SEED_DOMAIN + label.encode()).digest()
        self._sk = Ed25519PrivateKey.from_private_bytes(seed)
        self.public_key = self._sk.public_key().public_bytes(
            Encoding.Raw, PublicFormat.Raw)  # 32 bytes

    @property
    def id(self) -> str:
        d = hashlib.sha256(b"op" + self.public_key).hexdigest()
        return "op-" + d[:16]

    @property
    def key_id(self) -> str:
        d = hashlib.sha256(b"key" + self.public_key).hexdigest()
        return "k-" + d[:16]

    def record(self) -> dict:
        return {
            "id": self.id,
            "key_id": self.key_id,
            "public_key": self.public_key.hex(),
            "algorithm": "ed25519",
        }

    def sign(self, body: dict) -> dict:
        """Sign a descriptor body; returns the wire object with signature."""
        payload = canonical(body)
        return dict(body, signature={
            "key_id": self.key_id,
            "algorithm": "ed25519",
            "value": self._sk.sign(payload).hex(),
        })


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
                except ValueError:  # e.g. healthz returns plain "ok"
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


def ensure_service(api: Registry, operator: Operator, jur: str,
                   endpoint_uri: str, residency: str) -> str:
    """Register the signed C3 self-descriptor (class registry, jurisdiction=...)."""
    identifier = "registry-%s-v1" % jur.lower()
    body = {
        "identifier": identifier,
        "operator": operator.record(),
        "class": "registry",
        "endpoints": [{"uri": endpoint_uri,
                       "protocol_binding_ref": DEFAULT_PROTOCOL_BINDING}],
        "protocol_binding_ref": DEFAULT_PROTOCOL_BINDING,
        "jurisdiction": jur,
        "residency_class": residency,
        "status": "active",
        "service_kind": "jurisdictional-registry",  # memo: C3 subclass marker
        "succession_pointer": None,
    }
    signed = operator.sign(body)
    status, resp = api.post("/services", {
        "identifier": identifier,
        "version": "1.0.0",
        "effective_from": "2026-09-01T00:00:00Z",
        "body": signed,                       # carries the embedded signature
        "signature": signed["signature"],     # and the same block top-level
    })
    if status == 201:
        return "registered C3 self-descriptor `%s` (audit_seq %s)" % (
            identifier, resp.get("audit_seq"))
    if status == 409:
        return "C3 self-descriptor `%s` already registered" % identifier
    raise SystemExit("service registration failed (HTTP %d): %s"
                     % (status, json.dumps(resp)[:400]))


def ensure_profile(api: Registry, jur: str, register: str,
                   elements: list) -> tuple:
    """Register the jurisdictional profile item (class profile)."""
    jl = jur.lower()
    item_id = "urn:unidpp:profile:%s-battery-passport" % jl
    body = {
        "register_id": register,
        "item_id": item_id,
        "definition": (
            "%s jurisdictional battery DPP profile: placement obligation, "
            "carbon footprint, recycled content and due-diligence data "
            "points for batteries placed on the %s market" % (jur, jur)),
        "version": "1.0.0",
        "effective_from": "2027-02-18T00:00:00Z",
        "submitting_organization": "%s national registry authority (kit sample)" % jur,
        "manifest": {
            "version": "1.0.0",
            "axes": ["jurisdiction"],
            "data_points": [
                {"element": e["item_id"], "cardinality": "1",
                 "required_provenance": e["provenance"],
                 "subject_granularity": e["granularity"],
                 "trust_floor": e["trust_floor"]}
                for e in elements
            ],
            "custody": {"default_model": "identity_preserved",
                        "standard": "ISO 22095:2020"},
            "legal_basis": [{
                "instrument": "Regulation (EU) 2023/1542 (batteries) "
                              "Art. 77 — digital product passport",
                "force": "binding",
                "citation": "obligation applies from 2027-02-18",
            }],
            "triggers": [{
                "description": "battery of a covered category placed on the market",
                "evaluation_mode": "on_issuance",
                "predicate_class": "fact_predicate",
                "predicate_ref": "pred/battery-dpp-obligation",
            }],
        },
    }
    status, resp = api.post("/profiles", body)
    if status == 201:
        return item_id, "registered profile `%s` (audit_seq %s)" % (
            item_id, resp.get("audit_seq"))
    if status == 409:
        return item_id, "profile `%s` already registered" % item_id
    raise SystemExit("profile registration failed (HTTP %d): %s"
                     % (status, json.dumps(resp)[:400]))


def sample_elements(jur: str) -> list:
    # Element ids follow the pilot convention (`de/m/...` in
    # unidpp-pilot-data): the `de:` URN namespace is the *data element*
    # namespace, jurisdiction-neutral — the jurisdiction is carried by
    # the register (`jurisdiction-<code>`), which is what makes the same
    # concept harmonizable across national subregisters (memo F4).
    return [
        {
            "item_id": "urn:unidpp:de:battery-carbon-footprint",
            "definition": "Carbon footprint of the battery, declared per "
                          "the applicable delegated regulation (kg CO2e "
                          "per battery, life-cycle)",
            "provenance": "attested", "granularity": "instance",
            "trust_floor": "attested",
            "manifest": {"version": "1.0.0", "unit_ref": "unit-kg",
                         "datatype": "decimal(9,3)",
                         "representation": "ISO 80000-1 quantity value",
                         "source_register": "sample national RA entry"},
        },
        {
            "item_id": "urn:unidpp:de:battery-recycled-content",
            "definition": "Share of recycled content recovered from waste "
                          "in the active material of the battery, percent "
                          "by mass",
            "provenance": "attested", "granularity": "type",
            "trust_floor": "attested",
            "manifest": {"version": "1.0.0", "unit_ref": None,
                         "datatype": "decimal(5,2)",
                         "representation": "percentage 0-100",
                         "source_register": "sample national RA entry"},
        },
        {
            "item_id": "urn:unidpp:de:battery-due-diligence-statement",
            "definition": "Reference to the due-diligence statement and "
                          "verification report covering raw-material "
                          "supply chains of the battery",
            "provenance": "log_anchored", "granularity": "type",
            "trust_floor": "log_anchored",
            "manifest": {"version": "1.0.0", "unit_ref": None,
                         "datatype": "uri",
                         "representation": "signed document reference",
                         "source_register": "sample national RA entry"},
        },
    ]


def ensure_elements(api: Registry, jur: str, register: str,
                    elements: list) -> list:
    registered = []
    for el in elements:
        body = {
            "register_id": register,
            "item_id": el["item_id"],
            "definition": el["definition"],
            "version": "1.0.0",
            "effective_from": "2026-10-01T00:00:00Z",
            "submitting_organization": "%s national registry authority (kit sample)" % jur,
            "manifest": el["manifest"],
        }
        status, resp = api.post("/data-elements", body)
        if status == 201:
            registered.append("registered data element `%s` (audit_seq %s)"
                              % (el["item_id"], resp.get("audit_seq")))
        elif status == 409:
            registered.append("data element `%s` already registered"
                              % el["item_id"])
        else:
            raise SystemExit("data element `%s` failed (HTTP %d): %s"
                             % (el["item_id"], status,
                                json.dumps(resp)[:400]))
    return registered


def ensure_binding(api: Registry, jur: str, profile_id: str,
                   product_type: str) -> str:
    """Dated applicability binding, identity-keyed (never enumerable)."""
    eff = "2027-02-18T00:00:00Z"
    # Idempotency check through the same identity-keyed read the runtime
    # uses: ask whether the profile already applies to this subject.
    status, resp = api.get("/applicability?product_type=%s&at=%s"
                           % (urllib.parse.quote(product_type, safe=""), eff))
    if status == 200:
        for entry in resp.get("applicability", []):
            if entry.get("binding", {}).get("profile_item") == profile_id:
                return "applicability binding already in force for `%s`" % product_type
    status, resp = api.post("/applicability", {
        "profile_id": profile_id,
        "product_type": product_type,
        "effective_from": eff,
        "retroactive": False,
    })
    if status == 201:
        return "bound profile to `%s` from %s (audit_seq %s, non-retroactive)" % (
            product_type, eff, resp.get("audit_seq"))
    raise SystemExit("applicability binding failed (HTTP %d): %s"
                     % (status, json.dumps(resp)[:400]))


def main():
    kit_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--jurisdiction", required=True,
                    help="ISO 3166-1 alpha-2 code, e.g. DE, JP")
    ap.add_argument("--port", type=int,
                    default=int(os.environ.get("KIT_PORT", "8391")))
    ap.add_argument("--bind", default=os.environ.get("KIT_BIND", "127.0.0.1"))
    ap.add_argument("--base-url", default=None,
                    help="default http://$BIND:$PORT")
    ap.add_argument("--endpoint-uri", default=None,
                    help="URI the C3 descriptor advertises (default: base URL)")
    ap.add_argument("--admin-token", default=os.environ.get("KIT_ADMIN_TOKEN"))
    ap.add_argument("--token-file", default=os.path.join(
        os.environ.get("KIT_HOME", os.path.join(kit_root, "data")),
        "admin-token"))
    ap.add_argument("--operator-label", default=DEFAULT_OPERATOR_LABEL,
                    help="dev keyring label signing the descriptor "
                         "(production: your trust-service-issued key)")
    ap.add_argument("--register", default=None,
                    help="register id for the national subregister "
                         "(default: jurisdiction-<code-lowercase>)")
    ap.add_argument("--product-type", default="gtin:4260123400019",
                    help="sample product identity for the applicability demo")
    args = ap.parse_args()

    jur = args.jurisdiction.upper()
    if not re.fullmatch(r"[A-Z]{2}", jur):
        sys.exit("jurisdiction must be an ISO 3166-1 alpha-2 code (e.g. DE)")

    base = args.base_url or ("http://%s:%d" % (args.bind, args.port))
    token = args.admin_token
    if not token and os.path.isfile(args.token_file):
        with open(args.token_file) as f:
            token = f.read().strip()
    api = Registry(base, token or "")

    # The registry must be up and its base dataset (protocol bindings,
    # units) present — bin/run-registry.sh does both.
    status, _ = api.get("/healthz")
    if status != 200:
        sys.exit("no healthy registry at %s — run bin/run-registry.sh first" % base)

    operator = Operator(args.operator_label)
    register = args.register or ("jurisdiction-%s" % jur.lower())
    residency = RESIDENCY_BY_JURISDICTION.get(jur, "anywhere")
    endpoint_uri = args.endpoint_uri or (base + "/")

    print("unidpp-registry-kit: seeding jurisdiction %s against %s" % (jur, base))
    print("  operator:  %s (id %s, key %s)" % (
        args.operator_label, operator.id, operator.key_id))
    print("  register:  %s (the national subregister, F4)" % register)
    print()

    print("-", ensure_service(api, operator, jur, endpoint_uri, residency))

    elements = sample_elements(jur)
    for line in ensure_elements(api, jur, register, elements):
        print("-", line)

    profile_id, line = ensure_profile(api, jur, register, elements)
    print("-", line)

    print("-", ensure_binding(api, jur, profile_id, args.product_type))

    print()
    print("next (see bin/demo-jurisdiction.sh):")
    print("  curl -s '%s/services?jurisdiction=%s&class=registry' | jq" % (base, jur))
    print("  curl -s '%s/applicability?product_type=%s&at=2027-06-01T00:00:00Z' | jq" % (
        base, urllib.parse.quote(args.product_type, safe=":")))


if __name__ == "__main__":
    main()
