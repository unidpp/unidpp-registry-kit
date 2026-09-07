#!/usr/bin/env python3
"""seed-units.py — register the UnitsML unit inventory into the /units subregister.

Part of unidpp-registry-kit (memo work item C10): the unitsml-import.
Registers 48 units — the SI base set, the SI derived set (special names
and coherent compounds), the prefixed / non-SI-accepted set DPP data
points actually reference, and the DPP-context compound units (energy,
charge, CO2-equivalent) — into a running unidpp-registry through the
same 19135 item API as every other registration (`POST /units`,
`GET /units`; see ../unidpp-registry/README.md, "units subregister").

Inventory sources (every record names its source in `manifest.source`;
citations were cross-checked, never invented):

  1. UnitsDB (github.com/unitsml/unitsdb — NIST; local checkout
     ~/src/mn/isq-smart/unitsdb): units.yaml / quantities.yaml /
     dimensions.yaml supply the unitsml ids (`u:...`), names, symbols,
     quantity kinds, and the dimension powers.
  2. The ISO/IEC 80000 unified dataset
     (github.com/metanorma/iso-iec-80000, sources/dataset/quantities.yaml
     — local checkout ~/src/mn/isq-smart/iso-iec-80000): supplies the
     part, edition and item number for each unit's citation
     (ISO 80000-3/-4/-5/-7/-9/-10:2019, IEC 80000-6:2022,
     ISO 80000-1:2022).
  3. BIPM, The International System of Units (SI Brochure), 9th ed.
     (2019): the fallback citation for units with no ISO/IEC 80000
     table entry in the transcribed dataset (katal; Table 4 special
     names) and for the non-SI units accepted for use with the SI
     (Table 8: min, h, L, t, eV; §4.1 other non-SI units: bar) and the
     gram (kilogram/gram base-name note).

Dimension vectors follow the QUDT/UnitsDB letter crosswalk in ISO
80000-1 base-quantity order L M T E H A I D, where L length, M mass,
T time, E electric current, H thermodynamic temperature, A amount of
substance, I luminous intensity, D dimensionless — e.g. the volt is
"L2M1T-3E-1H0A0I0D0". Powers are taken from UnitsDB dimensions.yaml,
computed per unit through its primary quantity's dimension.

Idempotency: re-running against the same journal is safe — a unit
already registered (HTTP 409) is reported and skipped. Afterwards the
script verifies through `GET /units` that every seeded id resolves and
asserts the count.

Usage:
  bin/seed-units.py                       # against 127.0.0.1:8391
  bin/seed-units.py --port 8392 --register unitsml-de

Requires: Python 3.9+ (stdlib only).
"""

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request

REGISTER_DEFAULT = "unitsml"
SUBMITTING_ORG = "UnitsML / UnitsDB (NIST) — unidpp-registry-kit import"
EFFECTIVE_FROM = "2026-01-01T00:00:00Z"
KIT_VERSION = "1.0.0"

# -- source keys used in manifest.source --------------------------------
SRC_UNITSDB = "UnitsDB (unitsml/unitsdb): ids, names, symbols, quantity kinds, dimension powers"
SRC_80000 = "ISO/IEC 80000 unified dataset (metanorma/iso-iec-80000, sources/dataset/quantities.yaml): part, edition, item"
SRC_BIPM = "BIPM SI Brochure, 9th ed. (2019)"

# Each entry:
#   id        UnitsML unit id (`u:<short>`); prefixed ids follow the UnitsML
#             grammar proven by UnitsDB (u:kilogram, u:kilowatt_hour); the
#             two CO2-equivalent ids are kit-constructed (not UnitsML-
#             registered units) and say so in their definition.
#   name      ISO English name (UnitsDB preferred name)
#   symbol    unit symbol as written (UnitsDB unicode symbol)
#   quantity  quantity kind (UnitsDB primary quantity / 80000 item name)
#   dim       dimension vector, QUDT letters L M T E H A I D (see header)
#   citation  the unit's normative anchor
#   expr      coherent definition in terms of other units (exact)
#   source    provenance for this record (never invented)
#   category  grouping used in the seed report
UNITS = [
    # ---- SI base units (7) -------------------------------------------
    ("u:m", "metre", "m", "length",
     "L1M0T0E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-1.1 (length)",
     "SI base unit of length", SRC_UNITSDB + "; " + SRC_80000, "si-base"),
    ("u:kilogram", "kilogram", "kg", "mass",
     "L0M1T0E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-1 (mass)",
     "SI base unit of mass", SRC_UNITSDB + "; " + SRC_80000, "si-base"),
    ("u:second", "second", "s", "duration (time)",
     "L0M0T1E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-9 (duration)",
     "SI base unit of time", SRC_UNITSDB + "; " + SRC_80000, "si-base"),
    ("u:ampere", "ampere", "A", "electric current",
     "L0M0T0E1H0A0I0D0",
     "IEC 80000-6:2022, item 6-1 (electric current)",
     "SI base unit of electric current", SRC_UNITSDB + "; " + SRC_80000, "si-base"),
    ("u:kelvin", "kelvin", "K", "thermodynamic temperature",
     "L0M0T0E0H1A0I0D0",
     "ISO 80000-5:2019, item 5-1 (thermodynamic temperature)",
     "SI base unit of thermodynamic temperature", SRC_UNITSDB + "; " + SRC_80000, "si-base"),
    ("u:mole", "mole", "mol", "amount of substance",
     "L0M0T0E0H0A1I0D0",
     "ISO 80000-9:2019, item 9-2 (amount of substance)",
     "SI base unit of amount of substance", SRC_UNITSDB + "; " + SRC_80000, "si-base"),
    ("u:candela", "candela", "cd", "luminous intensity",
     "L0M0T0E0H0A0I1D0",
     "ISO 80000-7:2019, item 7-14 (luminous intensity)",
     "SI base unit of luminous intensity", SRC_UNITSDB + "; " + SRC_80000, "si-base"),

    # ---- SI derived units with special names (22) ---------------------
    ("u:radian", "radian", "rad", "plane angle",
     "L0M0T0E0H0A0I0D1",
     "ISO 80000-3:2019, item 3-5 (angular measure)",
     "1 rad = 1 m/m", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:steradian", "steradian", "sr", "solid angle",
     "L0M0T0E0H0A0I0D1",
     "ISO 80000-3:2019, item 3-8 (solid angular measure)",
     "1 sr = 1 m2/m2", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:hertz", "hertz", "Hz", "frequency",
     "L0M0T-1E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-17.1 (frequency)",
     "1 Hz = 1 s-1", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:newton", "newton", "N", "force",
     "L1M1T-2E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-9.1 (force)",
     "1 N = 1 kg m s-2", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:pascal", "pascal", "Pa", "pressure, stress",
     "L-1M1T-2E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-14.1 (pressure)",
     "1 Pa = 1 N/m2 = 1 kg m-1 s-2", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:joule", "joule", "J", "energy, work, amount of heat",
     "L2M1T-2E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-28 (energy, work)",
     "1 J = 1 N m", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:watt", "watt", "W", "power, radiant flux",
     "L2M1T-3E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-27 (power); IEC 80000-6:2022, item 6-45",
     "1 W = 1 J/s", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:coulomb", "coulomb", "C", "electric charge",
     "L0M0T1E1H0A0I0D0",
     "IEC 80000-6:2022, item 6-2.1 (electric charge)",
     "1 C = 1 A s", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:volt", "volt", "V", "electric potential difference",
     "L2M1T-3E-1H0A0I0D0",
     "IEC 80000-6:2022, item 6-11.1 (electric potential)",
     "1 V = 1 W/A", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:farad", "farad", "F", "capacitance",
     "L-2M-1T4E2H0A0I0D0",
     "IEC 80000-6:2022, item 6-13 (capacitance)",
     "1 F = 1 C/V", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:ohm", "ohm", "Ω", "electric resistance",
     "L2M1T-3E-2H0A0I0D0",
     "IEC 80000-6:2022, item 6-46 (resistance)",
     "1 Ω = 1 V/A", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:siemens", "siemens", "S", "electric conductance",
     "L-2M-1T3E2H0A0I0D0",
     "IEC 80000-6:2022, item 6-47 (conductance)",
     "1 S = 1 A/V = 1 Ω-1", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:weber", "weber", "Wb", "magnetic flux",
     "L2M1T-2E-1H0A0I0D0",
     "IEC 80000-6:2022, item 6-22.1 (magnetic flux)",
     "1 Wb = 1 V s", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:tesla", "tesla", "T", "magnetic flux density",
     "L0M1T-2E-1H0A0I0D0",
     "IEC 80000-6:2022, item 6-21 (magnetic flux density)",
     "1 T = 1 Wb/m2", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:henry", "henry", "H", "inductance",
     "L2M1T-2E-2H0A0I0D0",
     "IEC 80000-6:2022, item 6-41.1 (inductance)",
     "1 H = 1 Wb/A", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:degree_Celsius", "degree Celsius", "°C", "Celsius temperature",
     "L0M0T0E0H1A0I0D0",
     "ISO 80000-5:2019, item 5-2 (Celsius temperature)",
     "t/°C = T/K − 273.15", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:lumen", "lumen", "lm", "luminous flux",
     "L0M0T0E0H0A0I1D0",
     "ISO 80000-7:2019, item 7-13 (luminous flux)",
     "1 lm = 1 cd sr", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:lux", "lux", "lx", "illuminance",
     "L-2M0T0E0H0A0I1D0",
     "ISO 80000-7:2019, item 7-16 (illuminance)",
     "1 lx = 1 lm/m2", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:becquerel", "becquerel", "Bq", "activity referred to a radionuclide",
     "L0M0T-1E0H0A0I0D0",
     "ISO 80000-10:2019, item 10-27 (activity)",
     "1 Bq = 1 s-1", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:gray", "gray", "Gy", "absorbed dose",
     "L2M0T-2E0H0A0I0D0",
     "ISO 80000-10:2019, item 10-81.1 (absorbed dose)",
     "1 Gy = 1 J/kg", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    ("u:sievert", "sievert", "Sv", "dose equivalent",
     "L2M0T-2E0H0A0I0D0",
     "ISO 80000-10:2019, item 10-83.1 (dose equivalent)",
     "1 Sv = 1 J/kg", SRC_UNITSDB + "; " + SRC_80000, "si-derived-special"),
    # katal has no entry in the transcribed 80000 dataset — SI Brochure
    # fallback per the kit rule (never invent a citation).
    ("u:katal", "katal", "kat", "catalytic activity",
     "L0M0T-1E0H0A1I0D0",
     SRC_BIPM + ", Table 4 (SI coherent derived units with special names); no item in the transcribed ISO 80000-9 dataset",
     "1 kat = 1 mol/s", SRC_UNITSDB + "; " + SRC_BIPM, "si-derived-special"),

    # ---- SI derived units without special names (3) -------------------
    ("u:square_meter", "square metre", "m²", "area",
     "L2M0T0E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-3 (area)",
     "1 m2", SRC_UNITSDB + "; " + SRC_80000, "si-derived-compound"),
    ("u:cubic_meter", "cubic metre", "m³", "volume",
     "L3M0T0E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-4 (volume)",
     "1 m3", SRC_UNITSDB + "; " + SRC_80000, "si-derived-compound"),
    ("u:meter_per_second", "metre per second", "m/s", "velocity, speed",
     "L1M0T-1E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-10 (velocity, speed)",
     "1 m/s", SRC_UNITSDB + "; " + SRC_80000, "si-derived-compound"),

    # ---- SI prefixed units DPP data points use (4) --------------------
    # Prefixed UnitsML ids are constructed per the UnitsML id grammar
    # (prefix + root, as in UnitsDB's u:kilogram / u:kilowatt_hour).
    ("u:gram", "gram", "g", "mass",
     "L0M1T0E0H0A0I0D0",
     SRC_BIPM + " (kilogram; the gram is the unprefixed name base of the kilogram); 1 g = 10-3 kg",
     "1 g = 10-3 kg", SRC_UNITSDB + "; " + SRC_BIPM, "si-prefixed"),
    ("u:kilometer", "kilometre", "km", "length",
     "L1M0T0E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-1.1 (metre); prefix kilo per ISO 80000-1:2022 (SI prefixes)",
     "1 km = 103 m", SRC_UNITSDB + "; " + SRC_80000 + "; prefixed id constructed per UnitsML grammar", "si-prefixed"),
    ("u:millimeter", "millimetre", "mm", "length",
     "L1M0T0E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-1.1 (metre); prefix milli per ISO 80000-1:2022 (SI prefixes)",
     "1 mm = 10-3 m", SRC_UNITSDB + "; " + SRC_80000 + "; prefixed id constructed per UnitsML grammar", "si-prefixed"),
    ("u:megajoule", "megajoule", "MJ", "energy",
     "L2M1T-2E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-28 (joule); prefix mega per ISO 80000-1:2022 (SI prefixes)",
     "1 MJ = 106 J", SRC_UNITSDB + "; " + SRC_80000 + "; prefixed id constructed per UnitsML grammar", "si-prefixed"),

    # ---- non-SI units accepted for use with the SI (6) ----------------
    ("u:minute", "minute", "min", "duration (time)",
     "L0M0T1E0H0A0I0D0",
     SRC_BIPM + ", Table 8 (non-SI units accepted for use with the SI)",
     "1 min = 60 s", SRC_UNITSDB + "; " + SRC_BIPM, "non-si-accepted"),
    ("u:hour", "hour", "h", "duration (time)",
     "L0M0T1E0H0A0I0D0",
     SRC_BIPM + ", Table 8 (non-SI units accepted for use with the SI)",
     "1 h = 3600 s", SRC_UNITSDB + "; " + SRC_BIPM, "non-si-accepted"),
    ("u:liter", "litre", "L", "volume",
     "L3M0T0E0H0A0I0D0",
     SRC_BIPM + ", Table 8 (non-SI units accepted for use with the SI)",
     "1 L = 1 dm3 = 10-3 m3", SRC_UNITSDB + "; " + SRC_BIPM, "non-si-accepted"),
    ("u:metric_ton", "tonne", "t", "mass",
     "L0M1T0E0H0A0I0D0",
     SRC_BIPM + ", Table 8 (non-SI units accepted for use with the SI)",
     "1 t = 103 kg", SRC_UNITSDB + "; " + SRC_BIPM, "non-si-accepted"),
    ("u:electronvolt", "electronvolt", "eV", "energy",
     "L2M1T-2E0H0A0I0D0",
     SRC_BIPM + ", Table 8 (non-SI units accepted for use with the SI)",
     "1 eV = 1.602176634 × 10-19 J (exact)", SRC_UNITSDB + "; " + SRC_BIPM, "non-si-accepted"),
    ("u:bar", "bar", "bar", "pressure",
     "L-1M1T-2E0H0A0I0D0",
     SRC_BIPM + ", §4.1 (other non-SI units accepted for use with the SI)",
     "1 bar = 100 kPa", SRC_UNITSDB + "; " + SRC_BIPM, "non-si-accepted"),

    # ---- DPP-context units (6) ----------------------------------------
    # The units ESPR/battery-passport data points actually carry.
    ("u:percent", "percent", "%", "fraction of the unit one",
     "L0M0T0E0H0A0I0D1",
     "ISO 80000-1:2022, 7.1.4 (unit one; percent symbol %)",
     "1 % = 0.01", SRC_80000 + " (ISO 80000-1:2022 sources); id per UnitsML grammar", "dpp-context"),
    ("u:kilowatt_hour", "kilowatt hour", "kW·h", "energy (active energy)",
     "L2M1T-2E0H0A0I0D0",
     "IEC 80000-6:2022, item 6-62 (active energy)",
     "1 kW·h = 3.6 MJ exactly", SRC_UNITSDB + "; " + SRC_80000, "dpp-context"),
    ("u:watt_hour", "watt hour", "W·h", "energy (active energy)",
     "L2M1T-2E0H0A0I0D0",
     "IEC 80000-6:2022, item 6-62 (active energy)",
     "1 W·h = 3.6 kJ exactly", SRC_UNITSDB + "; " + SRC_80000, "dpp-context"),
    ("u:ampere_hour", "ampere hour", "A·h", "electric charge",
     "L0M0T1E1H0A0I0D0",
     "IEC 80000-6:2022, item 6-2.1 remark (electric charge; battery use)",
     "1 A·h = 3.6 kC exactly", SRC_UNITSDB + "; " + SRC_80000, "dpp-context"),
    # CO2-equivalent mass: no ISO 80000 quantity, no UnitsML-registered
    # unit — ids are kit-constructed, the citation names what is real
    # (the SI mass unit) and the conversion convention (GWP factors).
    ("u:gram_CO2e", "gram of carbon dioxide equivalent", "g CO2e",
     "mass of CO2-equivalent greenhouse gas",
     "L0M1T0E0H0A0I0D0",
     SRC_BIPM + " (gram); CO2-equivalence via GWP100 factors per IPCC AR6 (2023) — no ISO 80000 quantity of this kind",
     "1 g CO2e = 10-3 kg CO2e; GWP factors per IPCC AR6",
     SRC_BIPM + "; kit-constructed id (not a UnitsML-registered unit)", "dpp-context"),
    ("u:kilogram_CO2e", "kilogram of carbon dioxide equivalent", "kg CO2e",
     "mass of CO2-equivalent greenhouse gas",
     "L0M1T0E0H0A0I0D0",
     SRC_BIPM + " (kilogram); CO2-equivalence via GWP100 factors per IPCC AR6 (2023) — no ISO 80000 quantity of this kind",
     "GWP factors per IPCC AR6; declared unit of battery carbon footprint (kg CO2e per battery)",
     SRC_BIPM + "; kit-constructed id (not a UnitsML-registered unit)", "dpp-context"),
]


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


def ensure_unit(api: Registry, register: str, unit: tuple) -> str:
    """Register one unit item (409-tolerant, journal-safe)."""
    (uid, name, symbol, quantity, dim, citation, expr, source, category) = unit
    item_id = "unitsml:" + uid
    body = {
        "register_id": register,
        "item_id": item_id,
        "definition": "%s (%s) — %s; %s" % (name, symbol, quantity, expr),
        "version": KIT_VERSION,
        "effective_from": EFFECTIVE_FROM,
        "submitting_organization": SUBMITTING_ORG,
        "manifest": {
            "version": KIT_VERSION,
            "name": name,
            "symbol": symbol,
            "unitsml_id": uid,
            "category": category,
            "quantity_kind": quantity,
            "dimension_vector": dim,
            "definition_expr": expr,
            "iso_80000_citation": citation,
            "source": source,
        },
    }
    status, resp = api.post("/units", body)
    if status == 201:
        return "registered %s — %s (audit_seq %s)" % (item_id, name, resp.get("audit_seq"))
    if status == 409:
        return "already registered %s" % item_id
    raise SystemExit("unit `%s` failed (HTTP %d): %s"
                     % (item_id, status, json.dumps(resp)[:400]))


def verify(api: Registry, register: str) -> dict:
    """GET /units and assert every seeded unit resolves (count check)."""
    status, resp = api.get("/units?register=%s" % register)
    if status != 200:
        raise SystemExit("verification failed: GET /units -> HTTP %d" % status)
    listed = resp.get("items", [])
    listed_ids = {i.get("identifier") for i in listed}
    expected = {"unitsml:" + u[0] for u in UNITS}
    missing = sorted(expected - listed_ids)
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
                    help="register id for the units (default: unitsml)")
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

    # internal consistency: unique ids, dimension vectors well-formed
    # (8 components in QUDT order L M T E H A I D, each letter + integer)
    ids = [u[0] for u in UNITS]
    if len(ids) != len(set(ids)):
        sys.exit("unit table has duplicate ids")
    dim_re = re.compile(
        r"^L(-?\d+)M(-?\d+)T(-?\d+)E(-?\d+)H(-?\d+)A(-?\d+)I(-?\d+)D(-?\d+)$")
    for uid, _, _, _, dim, _, _, _, _ in UNITS:
        if not dim_re.match(dim):
            sys.exit("malformed dimension vector for %s: %s" % (uid, dim))

    categories = {}
    for u in UNITS:
        categories[u[8]] = categories.get(u[8], 0) + 1

    print("unidpp-registry-kit: seeding %d UnitsML units into %s%s (register `%s`)"
          % (len(UNITS), base, "/units", args.register))
    print("  categories: " + ", ".join(
        "%s %d" % (k, v) for k, v in sorted(categories.items())))
    print()

    for unit in UNITS:
        print("-", ensure_unit(api, args.register, unit))

    # verify through the public subregister read
    result = verify(api, args.register)
    print()
    print("verify: GET /units?register=%s -> count=%s (expected %d) — %s"
          % (args.register, result["subregister_count"], result["expected"],
             "OK" if result["ok"] else "FAILED"))
    if result["missing"]:
        for m in result["missing"]:
            print("  missing:", m)
        sys.exit(1)

    print()
    print("next:")
    print("  curl -s '%s/units?register=%s' | jq '.count'" % (base, args.register))
    print("  curl -s '%s/units/unitsml:u:kilowatt_hour' | jq '.manifest'" % base)


if __name__ == "__main__":
    main()
