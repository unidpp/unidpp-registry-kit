# unidpp-registry-kit

National DPP registry starter kit: run your jurisdiction's registry as a
federation peer — seeded, documented, enumeration-resistant. Memo work
item R3: the reference deployment of what the UniDPP framework already
runs, packaged as "your country's DPP registry in a box".

## Why a starter kit — the ePassport precedent

Every country that plans a DPP registry is right to want one — and the
international standard's job is not to replace it but to make it work
with everyone else's. The framework treats a national registry the way
aviation treats a national passport authority: each state issues and
inspects; the standard supplies the machine-readable format, the
resolution protocol, and a multi-witnessed list of trust anchors that
no single country controls. Your registry keeps your identifiers, your
crypto, your surveillance access; the federation protocol is what your
traders get in return — their products verified at every border without
your registry asking anyone's permission, and without anyone browsing
yours.

The alternative futures are both worse: one central world registry is
politically dead on arrival for every NSB that is not the first mover
and architecturally fatal (a single enumeration surface over the
world's installed base); N incompatible national silos make
cross-border verification bilateral diplomacy per product. The national
registry as **first-class peer in a federation** is the third path.

## "Registry" is six functions — the decomposition

The word conflates functions the framework already separates. A
national "DPP Registry" typically bundles:

| # | Function | Where it lives in this framework |
|---|---|---|
| F1 | Identifier allocation (minting/governing the national identifier space) | Registrar role under ISO/IEC 15459 issuing-agency discipline; national schemes (GS1/Handle/Ecode/MA) as C8 identifier-scheme items with registered bridges |
| F2 | Discovery/lookup (identifier → where the passport is served) | C3 service (class `registry`, jurisdiction=X) + resolver linksets routing by request context; national mirrors |
| F3 | Deposit of passport copies (where a regulator demands it) | A jurisdictional profile requirement, implemented as Tier-C notarized snapshots at a national archivist — authority never transfers with the copy (I7) |
| F4 | Semantic definitions | National RA as a FERIN subregister (ISO 19135:2025; EPSG precedent) |
| F5 | Trust (keys, accreditation) | Jurisdictional trust authority (threshold group) → trust list → M-of-K master list |
| F6 | Surveillance (invalidations, recalls, theft) | Predicate subregisters + predicate-based recall; statuses with reason-coded retroactivity |

This kit gives you F2's service-descriptor half, F4 in full, and the
F5/F6 seams, on the running `unidpp-registry` service. Global elements
stay tiny and governance-shaped (scheme namespaces, master trust list,
cross-register mappings); everything data-shaped stays yours.

## What is in the kit

| File | Purpose |
|---|---|
| `bin/run-registry.sh` | Launcher: builds and runs `unidpp-registry` (path dependency `../unidpp-registry`) on a configurable port with a persistent append-only journal, guarded admin mutations, base dataset seeded via the API; optional cloudflared tunnel |
| `bin/seed-jurisdiction.py` | Seeder parameterized by jurisdiction: the **signed C3 self-descriptor** (service class `registry`, jurisdiction=X), a jurisdiction profile item, a dated applicability binding, sample data-element items |
| `bin/seed-units.py` | Seeder for the **units subregister** (C1): 48 UnitsML units — SI base (7), SI derived with special names (22) and coherent compounds (m², m³, m/s), prefixed units DPP data points use (g, km, mm, MJ), non-SI units accepted with the SI (min, h, L, t, eV, bar), and DPP-context units (kWh, Wh, Ah, %, g/kg CO2e) — each with quantity kind, QUDT-letter dimension vector, exact conversion where exact (1 kW·h = 3.6 MJ), and a per-unit citation cross-checked against the ISO/IEC 80000 dataset (metanorma/iso-iec-80000) or the BIPM SI Brochure 9th ed. (UnitsDB supplies the ids/names/symbols; ids `unitsml:u:*` in register `unitsml`) |
| `bin/seed-untded.py` | Seeder for the **semantic subregister (F4)**: the full UNTDED 2005 trade data-element directory — 1504 data elements, tags 1000–9649 (1318 active, 186 retired with the directory's replacement notes), the 9 TDED categories — read live from the untded-2005 SSOT (github.com/untded/untded-2005, default `~/src/untded/untded-2005`; the YAML under `data/elements/`). Ids `urn:untded:de:<tag>` in register `untded`; each manifest carries tag, name, representation (`an..35` decomposed into charset/lengths), the D05B UN/EDIFACT element name where the join resolves (586 elements), the edition citation (ECE/TRADE/362 = ISO 7372:2005, section + page + untded.org element page), the source lifecycle state, change tag, old/business names and legacy bridges |
| `bin/seed-mappings.py` | Seeder for the **cross-register mappings** (ISO 19135 harmonization, registry item 57): deposits the GB 4943.1-2022 ↔ IEC 62368-1 equivalence (`urn:unidpp:map:gb4943-iec62368`, register `unidpp`) through the registry's dedicated cross-register-mapping item class — both endpoint items registered first, referentially intact (the registry's intake check rejects dangling ends); verifies via the directional lookup from both directions. Idempotent, 409-tolerant |
| `bin/demo-jurisdiction.sh` | The worked example — a fictional "DE" jurisdiction end-to-end: as-of applicability, supersession, enumeration-resistance posture |
| `ONBOARDING.md` | The ceremony guide for a country joining the federation: operator credential issuance, trust-list entry, discovery self-registration, continuity/succession filing, the conformance checklist |

Prerequisites: Rust toolchain (to build the registry; a prebuilt binary
is reused if present), Python 3.9+ with `cryptography` (`PyYAML` as
well, for the UNTDED seed), `curl`, `jq`. `cloudflared` only if you
pass `--tunnel`.

## What to run

```sh
git clone https://github.com/unidpp/unidpp-registry-kit
git clone https://github.com/unidpp/unidpp-registry   # sibling checkout (path dep)
cd unidpp-registry-kit

# 1. Run the registry (foreground; --daemon to background; --tunnel adds a
#    public URL). Journal persists under data/; admin token in data/admin-token.
bin/run-registry.sh --daemon

# 2. Seed the unit inventory (SI + common DPP units; idempotent, 409-tolerant)
bin/seed-units.py

# 3. Seed the UNTDED 2005 directory (1504 trade data elements, register
#    `untded`; requires the untded-2005 SSOT checkout, see the table above
#    — idempotent, 409-tolerant)
bin/seed-untded.py

# 4. Seed the cross-register mappings (GB 4943.1-2022 ↔ IEC 62368-1
#    equivalence through the dedicated item class — idempotent,
#    409-tolerant)
bin/seed-mappings.py

# 5. Seed your jurisdiction (ISO 3166-1 alpha-2)
bin/seed-jurisdiction.py --jurisdiction DE

# 6. Or run the whole worked example in one command
bin/demo-jurisdiction.sh
```

`bin/run-registry.sh stop` stops a daemon; the journal is never removed —
restart replays it (durability and auditability are the same mechanism).

Configuration (environment): `KIT_PORT` (default 8391; the UniDPP pilot
lives on 8390), `KIT_BIND`, `KIT_HOME`, `KIT_REGISTRY_DIR`,
`KIT_ADMIN_TOKEN` (default: generated once into `data/admin-token`),
`KIT_FORCE_BUILD=1`.

## What to query

The registry is `unidpp-registry` — full API semantics in its README.
The queries a federation peer or verifier runs:

```sh
BASE=http://127.0.0.1:8391

# Discovery: which registry serves jurisdiction DE, at which endpoint,
# under which wire grammar, signed by whom?
curl -s "$BASE/services?jurisdiction=DE&class=registry" | jq

# Point-in-time applicability: what applied to this product then?
# (identity-keyed: you must hold the identity to ask)
curl -s "$BASE/applicability?product_type=gtin:4260123400019&at=2027-06-01T00:00:00Z" | jq

# The semantic items behind a profile (national subregister)
curl -s "$BASE/data-elements" | jq
curl -s "$BASE/data-elements/urn:unidpp:de:battery-carbon-footprint?at=2027-06-01T00:00:00Z" | jq '.version'

# The UNTDED 2005 semantic subregister (1504 trade data elements,
# register `untded`) — e.g. UN/EDIFACT element 1000 documentName:
curl -s "$BASE/data-elements?register=untded" | jq '.count'
curl -s "$BASE/data-elements/urn:untded:de:1000" | jq '.manifest'

# The units subregister (C1): what a data point's unit_ref resolves to —
# symbol, quantity kind, dimension vector, citation, exact conversions
curl -s "$BASE/units?register=unitsml" | jq '.count'
curl -s "$BASE/units/unitsml:u:kilowatt_hour" | jq '.manifest'

# Cross-register mappings (ISO 19135 harmonization): the GB↔IEC
# equivalence from either direction, or by the named sides —
# ?item= matches both ends; ?source=/&target= the named direction
curl -s "$BASE/cross-register-mappings?item=gb-4943-1" | jq
curl -s "$BASE/cross-register-mappings?source=gb-4943-1&target=iec-62368-1" | jq '.mappings[].manifest'

# The supersession chain of a definition
curl -s "$BASE/data-elements/urn:unidpp:de:battery-carbon-footprint/supersession" | jq '.chain'

# Operator evidence (Bearer-guarded)
curl -s -H "Authorization: Bearer $(cat data/admin-token)" "$BASE/admin/log?limit=5" | jq
```

All reads carry as-of semantics (`?at=`, `x-as-of` header); without
`at`, the current registered version is returned.

## Enumeration resistance — what this registry does NOT expose

The registry holds *descriptors of services and shapes* — never records
of things (I12). Specifically:

- **No listing endpoint over identifiers-in-use.** `/applicability` is
  identity-keyed: the product identity is the input, never the output.
  A jurisdiction may index identifiers-in-use internally; the standard's
  requirement (conformance class) is that it do so verifiably without
  being browsable — predicate access for entitled regulators, no
  walk-the-registry enumeration.
- `/items` and the subregisters list **registered definitions** —
  public ISO 19135 items (data elements, profiles, units) — not
  passports, not products, not installed base.
- The audit log is Bearer-guarded operator evidence, not a query index.
- No passport payloads are stored anywhere in the stack; deposit (where
  a jurisdiction legislates it) is notarized Tier-C snapshots at a
  national archivist, and republication never transfers authority (I7).

`bin/demo-jurisdiction.sh` demonstrates each of these against the live
route table.

## Onboarding as a peer

See `ONBOARDING.md` for the full ceremony: operator credential
issuance (trust service / Confium threshold keys; the dev keyring seam
documented), trust-list entry and master-list admission (T3), discovery
self-registration (T2), the jurisdiction profile + semantic subregister
(F4), continuity/succession filing, and the jurisdictional-registry
operator conformance checklist (19135 governance, signed descriptors,
enumeration resistance, mirroring, as-of).

## Related repositories

- `unidpp/unidpp-registry` — the service this kit deploys: ISO 19135
  item registration, versioned supersession, point-in-time resolution,
  applicability bindings with retroactivity, signed C3/C4/C5 discovery
  layer.
- `unidpp/unidpp-pilot-data` — the pilot deployment this kit's seeds are
  modeled on (working journal, profiles, bindings).
- `unidpp/unidpp-signatif`, `unidpp/unidpp-trust` — the trust layer the
  production keyring seam plugs into.

## License

MIT — see LICENSE. The deployed service (`unidpp-registry`) is
Apache-2.0.
