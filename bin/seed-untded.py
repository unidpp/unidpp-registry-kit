#!/usr/bin/env python3
"""seed-untded.py — register the UNTDED 2005 trade data-element directory.

Part of unidpp-registry-kit (memo work item T-05): the UNTDED seed of
the registry NWIP. Reads the machine-readable **UNTDED 2005** dataset
(ECE/TRADE/362, published in parallel as ISO 7372:2005) from its
single source of truth — the untded-2005 repository (Ruby +
lutaml-model with YAML data; github.com/untded/untded-2005, local
checkout ~/src/untded/untded-2005, default path overridable with
--untded-dir / UNTDED_DIR) — and registers every data element as an
ISO 19135 item in the `untded` register:

  data/elements/*.yaml    1504 elements (tags 1000-9649, the 9 TDED
                          categories) -> POST /data-elements
                          (= POST /items, class data-element) with
    item_id               urn:untded:de:<tag>
    register_id           untded
    class                 data-element
    title                 UNTDED name (active) / old name (retired)
    definition            UNTDED description (active) / the directory's
                          own retirement line (retired elements carry no
                          description in the source)
    manifest              {version, tag, name, representation, charset,
                          min_length, max_length, un_edifact_ref,
                          untded_ref, untded_status, change_tag,
                          old_name, business_term, notes, bridges,
                          category, source_page, aligned_with_edifact}

Field mapping against the source (fields with no clean one-line mapping
are carried verbatim inside `manifest`, never dropped):

  tag                -> item_id urn:untded:de:<tag> + manifest.tag
  name               -> title + manifest.name
  description        -> definition
  representation     -> manifest.representation (raw notation `an..35`)
                        decomposed into charset / min_length / max_length
  un_edifact_ref     -> manifest.un_edifact_ref: the D05B UN/EDIFACT
                        element name for this tag, from the SSOT's
                        derived/edifact-links.json join (586 active
                        elements are referenced by D05B segments; 549
                        representation-aligned, 37 drifted — the D05B
                        vintage caveat is in the manifest flag, not
                        silently normalized)
  untded_ref         -> manifest.untded_ref: the edition citation
                        (UNTDED 2005, ECE/TRADE/362 = ISO 7372:2005,
                        section 4.2 + page) — the public element page
                        https://untded.org/elements/<tag> is the live
                        resolution target
  status             -> manifest.untded_status (source lifecycle:
                        active 1318 / retired 186). The registry's own
                        19135 status is `valid` for every seeded item —
                        new items cannot be registered in any other
                        status (see unidpp-registry/api.rs
                        create_item); the source lifecycle stays a
                        manifest property, and retired elements carry
                        the directory's replacement note ("DE to use
                        instead - 3392") in manifest.notes.
  change_tag / old_name / business_term / notes / bridges
                     -> carried verbatim in the manifest (bridges is
                        the UNLK / SAD / CIMP / MAR legacy-standard
                        bridge list)

Idempotency: re-running against the same journal is safe — an element
already registered (HTTP 409) is reported and skipped. Afterwards the
script verifies through `GET /data-elements?register=untded` that the
directory resolves and asserts the count.

Usage:
  bin/seed-untded.py                    # against 127.0.0.1:8391
  bin/seed-untded.py --port 8390        # against the pilot registry
  bin/seed-untded.py --status active    # active elements only
  bin/seed-untded.py --untded-dir ~/src/untded/untded-2005

Requires: Python 3.9+ with PyYAML (stdlib otherwise).
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

import yaml

REGISTER_DEFAULT = "untded"
SUBMITTING_ORG = "UNTDED 2005 (ECE/TRADE/362 = ISO 7372:2005) — untded-2005 SSOT import"
EFFECTIVE_FROM = "2005-01-01T00:00:00Z"  # the edition's publication year
KIT_VERSION = "1.0.0"
UNTDED_CITATION = "UNTDED 2005, ECE/TRADE/362 (ISO 7372:2005), §4.2 Trade Data Elements Directory"
UNTDED_DEFAULT_DIR = os.path.expanduser(
    os.environ.get("UNTDED_DIR", "~/src/untded/untded-2005"))
EDIFACT_LINKS_REL = "derived/edifact-links.json"
EDIFACT_SOURCE = ("UN/EDIFACT D.05B segments mirror — untded-2005 "
                  "derived/edifact-links.json (586 active elements referenced "
                  "by D05B segments; cross-check vintage, the edition was "
                  "built on D.02A)")


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


def load_elements(untded_dir: str):
    """The SSOT YAML: data/elements/*.yaml, tags ascending."""
    root = os.path.join(untded_dir, "data", "elements")
    if not os.path.isdir(root):
        sys.exit("no data/elements directory under %s — pass --untded-dir "
                 "(the untded-2005 SSOT checkout)" % untded_dir)
    elements, categories = [], {}
    for fname in sorted(os.listdir(root)):
        if not fname.endswith(".yaml"):
            continue
        with open(os.path.join(root, fname)) as f:
            doc = yaml.safe_load(f)
        category = (doc.get("category") or "").strip()
        for el in doc.get("elements") or []:
            el["_category"] = category
            elements.append(el)
            categories[category] = categories.get(category, 0) + 1
    elements.sort(key=lambda e: e["tag"])
    return elements, categories


def load_edifact_links(untded_dir: str):
    """derived/edifact-links.json — D05B element name per tag."""
    path = os.path.join(untded_dir, EDIFACT_LINKS_REL)
    if not os.path.isfile(path):
        print("  note: %s not found — seeding without un_edifact_ref "
              "(run `bin/join-edifact` in the SSOT to derive it)" % path)
        return {}
    with open(path) as f:
        doc = json.load(f)
    return {l["tag"]: l for l in doc.get("links", [])}


def build_item(el: dict, register: str, edifact: dict):
    """Map one UNTDED element to the 19135 item shape."""
    tag = el["tag"]
    retired = el.get("status") == "retired"
    name = el.get("name") or el.get("old_name") or ""
    manifest = {"version": KIT_VERSION, "tag": tag}
    if el.get("name"):
        manifest["name"] = el["name"]
    rep = el.get("representation") or {}
    if rep:
        manifest["representation"] = rep.get("raw")
        if rep.get("charset"):
            manifest["charset"] = rep["charset"]
        if rep.get("min_length") is not None:
            manifest["min_length"] = rep["min_length"]
        if rep.get("max_length") is not None:
            manifest["max_length"] = rep["max_length"]
    link = edifact.get(tag)
    if link and link.get("edifact", {}).get("name"):
        manifest["un_edifact_ref"] = link["edifact"]["name"]
        if "aligned" in link:
            manifest["aligned_with_edifact"] = bool(link["aligned"])
    manifest["untded_ref"] = "%s, element %d (p. %s); https://www.untded.org/elements/%d" % (
        UNTDED_CITATION, tag, (el.get("provenance") or {}).get("page", "?"), tag)
    manifest["untded_status"] = el.get("status") or "active"
    if el.get("change_tag"):
        manifest["change_tag"] = el["change_tag"]
    if el.get("old_name"):
        manifest["old_name"] = el["old_name"]
    if el.get("business_term"):
        manifest["business_term"] = el["business_term"]
    if el.get("notes"):
        manifest["notes"] = el["notes"]
    if el.get("bridges"):
        manifest["bridges"] = el["bridges"]
    if el.get("_category"):
        manifest["category"] = el["_category"]
    if (el.get("provenance") or {}).get("page") is not None:
        manifest["source_page"] = el["provenance"]["page"]

    if retired:
        definition = "%s — retired in UNTDED 2005. %s" % (
            name, el.get("notes") or "No replacement stated in the directory.")
    else:
        definition = el.get("description") or name
    # The registry API takes the item title from `definition` (falling
    # back to `title` only when definition is absent), so the wire body
    # carries no separate title and the definition doubles as the 19135
    # item name — the UNTDED element name rides in manifest.name.
    return {
        "register_id": register,
        "item_id": "urn:untded:de:%d" % tag,
        "class": "data-element",
        "title": name or ("UNTDED data element %d" % tag),
        "definition": definition,
        "version": KIT_VERSION,
        "effective_from": EFFECTIVE_FROM,
        "submitting_organization": SUBMITTING_ORG,
        "manifest": manifest,
    }


def ensure_element(api: Registry, register: str, el: dict, edifact: dict):
    """Register one data element (409-tolerant, journal-safe)."""
    body = build_item(el, register, edifact)
    status, resp = api.post("/data-elements", body)
    if status == 201:
        return "registered", "registered %s (audit_seq %s)" % (
            body["item_id"], resp.get("audit_seq"))
    if status == 409:
        return "skipped", "already registered %s" % body["item_id"]
    return "failed", "%s failed (HTTP %d): %s" % (
        body["item_id"], status, json.dumps(resp)[:400])


def verify(api: Registry, register: str, expected: set):
    """GET /data-elements and assert the UNTDED subset resolves."""
    status, resp = api.get("/data-elements?register=%s" % register)
    if status != 200:
        raise SystemExit("verification failed: GET /data-elements -> HTTP %d"
                         % status)
    listed = {i.get("identifier") for i in resp.get("items", [])}
    missing = sorted(expected - listed)
    return {
        "subregister_count": resp.get("count"),
        "listed": len(listed),
        "expected": len(expected),
        "missing": missing,
        "ok": not missing and len(listed) == len(expected),
    }


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
                    help="register id for the elements (default: untded)")
    ap.add_argument("--untded-dir", default=UNTDED_DEFAULT_DIR,
                    help="untded-2005 SSOT checkout (default: ~/src/untded/untded-2005)")
    ap.add_argument("--status", choices=["active", "retired", "all"],
                    default="all",
                    help="which source lifecycle states to seed "
                         "(default: all — the full directory)")
    ap.add_argument("--limit", type=int, default=None,
                    help="seed at most N elements (smallest tags first) — "
                         "for smoke runs; the default is the full directory")
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

    elements, categories = load_elements(args.untded_dir)
    if args.status != "all":
        want = {"active": "active", "retired": "retired"}[args.status]
        elements = [e for e in elements if e.get("status") == want]
    if args.limit is not None:
        elements = elements[:args.limit]
    edifact = load_edifact_links(args.untded_dir)

    # internal consistency: unique tags, well-formed item ids
    tags = [e["tag"] for e in elements]
    if len(tags) != len(set(tags)):
        sys.exit("duplicate tags in the UNTDED dataset")

    active = sum(1 for e in elements if e.get("status") == "active")
    linked = sum(1 for e in elements if e["tag"] in edifact)
    print("unidpp-registry-kit: seeding %d UNTDED 2005 data elements into "
          "%s/data-elements (register `%s`)" % (len(elements), base, args.register))
    print("  lifecycle: %d active / %d retired (source states; every item "
          "registers 19135-status valid)" % (active, len(elements) - active))
    print("  un_edifact_ref: %d of %d (D05B join: %s)" % (
        linked, len(elements), EDIFACT_SOURCE.split(" — ")[0]))
    print("  categories: " + ", ".join(
        "%s %d" % (k, v) for k, v in sorted(categories.items())))
    print()

    registered = skipped = failed = 0
    for i, el in enumerate(elements, 1):
        outcome, line = ensure_element(api, args.register, el, edifact)
        if outcome == "registered":
            registered += 1
        elif outcome == "skipped":
            skipped += 1
        else:
            failed += 1
            print("  FAILED:", line)
        if i % 250 == 0 or i == len(elements):
            print("  %d/%d — %d registered, %d already present, %d failed"
                  % (i, len(elements), registered, skipped, failed))

    # verify through the public subregister read
    result = verify(api, args.register,
                    {"urn:untded:de:%d" % e["tag"] for e in elements})
    print()
    print("verify: GET /data-elements?register=%s -> count=%s (expected %d) — %s"
          % (args.register, result["subregister_count"], result["expected"],
             "OK" if result["ok"] else "FAILED"))
    if result["missing"]:
        for m in result["missing"][:10]:
            print("  missing:", m)
        sys.exit(1)
    if failed:
        sys.exit("%d elements failed to register" % failed)

    print()
    print("next:")
    print("  curl -s '%s/data-elements?register=untded' | jq '.count'" % base)
    print("  curl -s '%s/data-elements/urn:untded:de:1000' | jq '.manifest'" % base)


if __name__ == "__main__":
    main()
