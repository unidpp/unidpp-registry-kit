//! `unidpp-kit seed-units` — register the UnitsML unit inventory into
//! the /units subregister (memo work item C10): the SI base set, the
//! SI derived set (special names and coherent compounds), the
//! prefixed / non-SI-accepted set DPP data points actually reference,
//! and the DPP-context compound units. Every record names its source
//! in `manifest.source`; citations were cross-checked, never invented.

use serde_json::{json, Value};

use super::parse_seed_args;
use crate::die;

pub const USAGE: &str = "usage: unidpp-kit seed-units [--register R] \
[--port N] [--bind ADDR] [--base-url URL] [--admin-token T] [--token-file PATH]";

const SUBMITTING_ORG: &str = "UnitsML / UnitsDB (NIST) — unidpp-registry-kit import";
const EFFECTIVE_FROM: &str = "2026-01-01T00:00:00Z";
const KIT_VERSION: &str = "1.0.0";

// -- source keys used in manifest.source --------------------------------
const SRC_UNITSDB: &str =
    "UnitsDB (unitsml/unitsdb): ids, names, symbols, quantity kinds, dimension powers";
const SRC_80000: &str = "ISO/IEC 80000 unified dataset (metanorma/iso-iec-80000, sources/dataset/quantities.yaml): part, edition, item";
const SRC_BIPM: &str = "BIPM SI Brochure, 9th ed. (2019)";

/// One unit row: (id, name, symbol, quantity, dimension vector,
/// citation, coherent definition, source, category). The source is
/// owned because it composes the shared citation keys.
type UnitRow = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    String,
    &'static str,
);

#[rustfmt::skip]
fn units() -> Vec<UnitRow> {
    vec![
    // ---- SI base units (7) -------------------------------------------
    ("u:m", "metre", "m", "length",
     "L1M0T0E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-1.1 (length)",
     "SI base unit of length", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-base"),
    ("u:kilogram", "kilogram", "kg", "mass",
     "L0M1T0E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-1 (mass)",
     "SI base unit of mass", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-base"),
    ("u:second", "second", "s", "duration (time)",
     "L0M0T1E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-9 (duration)",
     "SI base unit of time", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-base"),
    ("u:ampere", "ampere", "A", "electric current",
     "L0M0T0E1H0A0I0D0",
     "IEC 80000-6:2022, item 6-1 (electric current)",
     "SI base unit of electric current", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-base"),
    ("u:kelvin", "kelvin", "K", "thermodynamic temperature",
     "L0M0T0E0H1A0I0D0",
     "ISO 80000-5:2019, item 5-1 (thermodynamic temperature)",
     "SI base unit of thermodynamic temperature", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-base"),
    ("u:mole", "mole", "mol", "amount of substance",
     "L0M0T0E0H0A1I0D0",
     "ISO 80000-9:2019, item 9-2 (amount of substance)",
     "SI base unit of amount of substance", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-base"),
    ("u:candela", "candela", "cd", "luminous intensity",
     "L0M0T0E0H0A0I1D0",
     "ISO 80000-7:2019, item 7-14 (luminous intensity)",
     "SI base unit of luminous intensity", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-base"),

    // ---- SI derived units with special names (22) ---------------------
    ("u:radian", "radian", "rad", "plane angle",
     "L0M0T0E0H0A0I0D1",
     "ISO 80000-3:2019, item 3-5 (angular measure)",
     "1 rad = 1 m/m", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:steradian", "steradian", "sr", "solid angle",
     "L0M0T0E0H0A0I0D1",
     "ISO 80000-3:2019, item 3-8 (solid angular measure)",
     "1 sr = 1 m2/m2", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:hertz", "hertz", "Hz", "frequency",
     "L0M0T-1E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-17.1 (frequency)",
     "1 Hz = 1 s-1", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:newton", "newton", "N", "force",
     "L1M1T-2E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-9.1 (force)",
     "1 N = 1 kg m s-2", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:pascal", "pascal", "Pa", "pressure, stress",
     "L-1M1T-2E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-14.1 (pressure)",
     "1 Pa = 1 N/m2 = 1 kg m-1 s-2", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:joule", "joule", "J", "energy, work, amount of heat",
     "L2M1T-2E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-28 (energy, work)",
     "1 J = 1 N m", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:watt", "watt", "W", "power, radiant flux",
     "L2M1T-3E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-27 (power); IEC 80000-6:2022, item 6-45",
     "1 W = 1 J/s", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:coulomb", "coulomb", "C", "electric charge",
     "L0M0T1E1H0A0I0D0",
     "IEC 80000-6:2022, item 6-2.1 (electric charge)",
     "1 C = 1 A s", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:volt", "volt", "V", "electric potential difference",
     "L2M1T-3E-1H0A0I0D0",
     "IEC 80000-6:2022, item 6-11.1 (electric potential)",
     "1 V = 1 W/A", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:farad", "farad", "F", "capacitance",
     "L-2M-1T4E2H0A0I0D0",
     "IEC 80000-6:2022, item 6-13 (capacitance)",
     "1 F = 1 C/V", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:ohm", "ohm", "Ω", "electric resistance",
     "L2M1T-3E-2H0A0I0D0",
     "IEC 80000-6:2022, item 6-46 (resistance)",
     "1 Ω = 1 V/A", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:siemens", "siemens", "S", "electric conductance",
     "L-2M-1T3E2H0A0I0D0",
     "IEC 80000-6:2022, item 6-47 (conductance)",
     "1 S = 1 A/V = 1 Ω-1", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:weber", "weber", "Wb", "magnetic flux",
     "L2M1T-2E-1H0A0I0D0",
     "IEC 80000-6:2022, item 6-22.1 (magnetic flux)",
     "1 Wb = 1 V s", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:tesla", "tesla", "T", "magnetic flux density",
     "L0M1T-2E-1H0A0I0D0",
     "IEC 80000-6:2022, item 6-21 (magnetic flux density)",
     "1 T = 1 Wb/m2", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:henry", "henry", "H", "inductance",
     "L2M1T-2E-2H0A0I0D0",
     "IEC 80000-6:2022, item 6-41.1 (inductance)",
     "1 H = 1 Wb/A", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:degree_Celsius", "degree Celsius", "°C", "Celsius temperature",
     "L0M0T0E0H1A0I0D0",
     "ISO 80000-5:2019, item 5-2 (Celsius temperature)",
     "t/°C = T/K − 273.15", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:lumen", "lumen", "lm", "luminous flux",
     "L0M0T0E0H0A0I1D0",
     "ISO 80000-7:2019, item 7-13 (luminous flux)",
     "1 lm = 1 cd sr", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:lux", "lux", "lx", "illuminance",
     "L-2M0T0E0H0A0I1D0",
     "ISO 80000-7:2019, item 7-16 (illuminance)",
     "1 lx = 1 lm/m2", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:becquerel", "becquerel", "Bq", "activity referred to a radionuclide",
     "L0M0T-1E0H0A0I0D0",
     "ISO 80000-10:2019, item 10-27 (activity)",
     "1 Bq = 1 s-1", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:gray", "gray", "Gy", "absorbed dose",
     "L2M0T-2E0H0A0I0D0",
     "ISO 80000-10:2019, item 10-81.1 (absorbed dose)",
     "1 Gy = 1 J/kg", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    ("u:sievert", "sievert", "Sv", "dose equivalent",
     "L2M0T-2E0H0A0I0D0",
     "ISO 80000-10:2019, item 10-83.1 (dose equivalent)",
     "1 Sv = 1 J/kg", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-special"),
    // katal has no entry in the transcribed 80000 dataset — SI Brochure
    // fallback per the kit rule (never invent a citation).
    ("u:katal", "katal", "kat", "catalytic activity",
     "L0M0T-1E0H0A1I0D0",
     "BIPM SI Brochure, 9th ed. (2019), Table 4 (SI coherent derived units with special names); no item in the transcribed ISO 80000-9 dataset",
     "1 kat = 1 mol/s", format!("{SRC_UNITSDB}; {SRC_BIPM}"), "si-derived-special"),

    // ---- SI derived units without special names (3) -------------------
    ("u:square_meter", "square metre", "m²", "area",
     "L2M0T0E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-3 (area)",
     "1 m2", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-compound"),
    ("u:cubic_meter", "cubic metre", "m³", "volume",
     "L3M0T0E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-4 (volume)",
     "1 m3", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-compound"),
    ("u:meter_per_second", "metre per second", "m/s", "velocity, speed",
     "L1M0T-1E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-10 (velocity, speed)",
     "1 m/s", format!("{SRC_UNITSDB}; {SRC_80000}"), "si-derived-compound"),

    // ---- SI prefixed units DPP data points use (4) --------------------
    // Prefixed UnitsML ids are constructed per the UnitsML id grammar
    // (prefix + root, as in UnitsDB's u:kilogram / u:kilowatt_hour).
    ("u:gram", "gram", "g", "mass",
     "L0M1T0E0H0A0I0D0",
     "BIPM SI Brochure, 9th ed. (2019) (kilogram; the gram is the unprefixed name base of the kilogram); 1 g = 10-3 kg",
     "1 g = 10-3 kg", format!("{SRC_UNITSDB}; {SRC_BIPM}"), "si-prefixed"),
    ("u:kilometer", "kilometre", "km", "length",
     "L1M0T0E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-1.1 (metre); prefix kilo per ISO 80000-1:2022 (SI prefixes)",
     "1 km = 103 m", format!("{SRC_UNITSDB}; {SRC_80000}; prefixed id constructed per UnitsML grammar"), "si-prefixed"),
    ("u:millimeter", "millimetre", "mm", "length",
     "L1M0T0E0H0A0I0D0",
     "ISO 80000-3:2019, item 3-1.1 (metre); prefix milli per ISO 80000-1:2022 (SI prefixes)",
     "1 mm = 10-3 m", format!("{SRC_UNITSDB}; {SRC_80000}; prefixed id constructed per UnitsML grammar"), "si-prefixed"),
    ("u:megajoule", "megajoule", "MJ", "energy",
     "L2M1T-2E0H0A0I0D0",
     "ISO 80000-4:2019, item 4-28 (joule); prefix mega per ISO 80000-1:2022 (SI prefixes)",
     "1 MJ = 106 J", format!("{SRC_UNITSDB}; {SRC_80000}; prefixed id constructed per UnitsML grammar"), "si-prefixed"),

    // ---- non-SI units accepted for use with the SI (6) ----------------
    ("u:minute", "minute", "min", "duration (time)",
     "L0M0T1E0H0A0I0D0",
     "BIPM SI Brochure, 9th ed. (2019), Table 8 (non-SI units accepted for use with the SI)",
     "1 min = 60 s", format!("{SRC_UNITSDB}; {SRC_BIPM}"), "non-si-accepted"),
    ("u:hour", "hour", "h", "duration (time)",
     "L0M0T1E0H0A0I0D0",
     "BIPM SI Brochure, 9th ed. (2019), Table 8 (non-SI units accepted for use with the SI)",
     "1 h = 3600 s", format!("{SRC_UNITSDB}; {SRC_BIPM}"), "non-si-accepted"),
    ("u:liter", "litre", "L", "volume",
     "L3M0T0E0H0A0I0D0",
     "BIPM SI Brochure, 9th ed. (2019), Table 8 (non-SI units accepted for use with the SI)",
     "1 L = 1 dm3 = 10-3 m3", format!("{SRC_UNITSDB}; {SRC_BIPM}"), "non-si-accepted"),
    ("u:metric_ton", "tonne", "t", "mass",
     "L0M1T0E0H0A0I0D0",
     "BIPM SI Brochure, 9th ed. (2019), Table 8 (non-SI units accepted for use with the SI)",
     "1 t = 103 kg", format!("{SRC_UNITSDB}; {SRC_BIPM}"), "non-si-accepted"),
    ("u:electronvolt", "electronvolt", "eV", "energy",
     "L2M1T-2E0H0A0I0D0",
     "BIPM SI Brochure, 9th ed. (2019), Table 8 (non-SI units accepted for use with the SI)",
     "1 eV = 1.602176634 × 10-19 J (exact)", format!("{SRC_UNITSDB}; {SRC_BIPM}"), "non-si-accepted"),
    ("u:bar", "bar", "bar", "pressure",
     "L-1M1T-2E0H0A0I0D0",
     "BIPM SI Brochure, 9th ed. (2019), §4.1 (other non-SI units accepted for use with the SI)",
     "1 bar = 100 kPa", format!("{SRC_UNITSDB}; {SRC_BIPM}"), "non-si-accepted"),

    // ---- DPP-context units (6) ----------------------------------------
    // The units ESPR/battery-passport data points actually carry.
    ("u:percent", "percent", "%", "fraction of the unit one",
     "L0M0T0E0H0A0I0D1",
     "ISO 80000-1:2022, 7.1.4 (unit one; percent symbol %)",
     "1 % = 0.01", format!("{SRC_80000} (ISO 80000-1:2022 sources); id per UnitsML grammar"), "dpp-context"),
    ("u:kilowatt_hour", "kilowatt hour", "kW·h", "energy (active energy)",
     "L2M1T-2E0H0A0I0D0",
     "IEC 80000-6:2022, item 6-62 (active energy)",
     "1 kW·h = 3.6 MJ exactly", format!("{SRC_UNITSDB}; {SRC_80000}"), "dpp-context"),
    ("u:watt_hour", "watt hour", "W·h", "energy (active energy)",
     "L2M1T-2E0H0A0I0D0",
     "IEC 80000-6:2022, item 6-62 (active energy)",
     "1 W·h = 3.6 kJ exactly", format!("{SRC_UNITSDB}; {SRC_80000}"), "dpp-context"),
    ("u:ampere_hour", "ampere hour", "A·h", "electric charge",
     "L0M0T1E1H0A0I0D0",
     "IEC 80000-6:2022, item 6-2.1 remark (electric charge; battery use)",
     "1 A·h = 3.6 kC exactly", format!("{SRC_UNITSDB}; {SRC_80000}"), "dpp-context"),
    // CO2-equivalent mass: no ISO 80000 quantity, no UnitsML-registered
    // unit — ids are kit-constructed, the citation names what is real
    // (the SI mass unit) and the conversion convention (GWP factors).
    ("u:gram_CO2e", "gram of carbon dioxide equivalent", "g CO2e",
     "mass of CO2-equivalent greenhouse gas",
     "L0M1T0E0H0A0I0D0",
     "BIPM SI Brochure, 9th ed. (2019) (gram); CO2-equivalence via GWP100 factors per IPCC AR6 (2023) — no ISO 80000 quantity of this kind",
     "1 g CO2e = 10-3 kg CO2e; GWP factors per IPCC AR6",
     format!("{SRC_BIPM}; kit-constructed id (not a UnitsML-registered unit)"), "dpp-context"),
    ("u:kilogram_CO2e", "kilogram of carbon dioxide equivalent", "kg CO2e",
     "mass of CO2-equivalent greenhouse gas",
     "L0M1T0E0H0A0I0D0",
     "BIPM SI Brochure, 9th ed. (2019) (kilogram); CO2-equivalence via GWP100 factors per IPCC AR6 (2023) — no ISO 80000 quantity of this kind",
     "GWP factors per IPCC AR6; declared unit of battery carbon footprint (kg CO2e per battery)",
     format!("{SRC_BIPM}; kit-constructed id (not a UnitsML-registered unit)"), "dpp-context"),
    ]
}

const DIM_LETTERS: &[u8] = b"LMTEHAID";

/// The dimension vector's shape: eight letter+integer components in
/// QUDT order L M T E H A I D.
fn dimension_well_formed(dim: &str) -> bool {
    let bytes = dim.as_bytes();
    let mut at = 0usize;
    for letter in DIM_LETTERS {
        if at >= bytes.len() || bytes[at] != *letter {
            return false;
        }
        at += 1;
        let start = at;
        if at < bytes.len() && (bytes[at] == b'-' || bytes[at] == b'+') {
            at += 1;
        }
        let digits_start = at;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
        if at == digits_start {
            return false;
        }
        let _ = start;
    }
    at == bytes.len()
}

fn ensure_unit(api: &super::Registry, register: &str, unit: &UnitRow) -> String {
    let (uid, name, symbol, quantity, dim, citation, expr, source, category) = unit;
    let item_id = format!("unitsml:{uid}");
    let body = json!({
        "register_id": register,
        "item_id": item_id,
        "definition": format!("{name} ({symbol}) — {quantity}; {expr}"),
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
    });
    let (status, resp) = api.post("/units", &body);
    match status {
        201 => format!(
            "registered {item_id} — {name} (audit_seq {})",
            resp.get("audit_seq").cloned().unwrap_or(Value::Null)
        ),
        409 => format!("already registered {item_id}"),
        other => die(&format!(
            "unit `{item_id}` failed (HTTP {other}): {}",
            serde_json::to_string(&resp)
                .unwrap_or_default()
                .chars()
                .take(400)
                .collect::<String>()
        )),
    }
}

fn verify(api: &super::Registry, register: &str) -> (Value, usize, Vec<String>) {
    let (status, resp) = api.get(&format!("/units?register={register}"));
    if status != 200 {
        die(&format!("verification failed: GET /units -> HTTP {status}"));
    }
    let listed = resp
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let listed_ids: Vec<String> = listed
        .iter()
        .filter_map(|i| i.get("identifier").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    let expected: Vec<String> = units().iter().map(|u| format!("unitsml:{}", u.0)).collect();
    let missing: Vec<String> = expected
        .iter()
        .filter(|e| !listed_ids.contains(e))
        .cloned()
        .collect();
    (
        resp.get("count").cloned().unwrap_or(Value::Null),
        expected.len(),
        missing,
    )
}

pub fn run(kit: &crate::Kit, args: &[String]) {
    let mut register = "unitsml".to_string();
    let mut shared = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--register" => register = rest.next().unwrap_or_else(|| die(USAGE)).clone(),
            _ => shared.push(arg.clone()),
        }
    }
    let parsed = parse_seed_args(kit, &shared, USAGE);
    let api = parsed.registry(kit);
    api.require_health();

    // internal consistency: unique ids, well-formed dimension vectors
    let table = units();
    let mut ids: Vec<&str> = table.iter().map(|u| u.0).collect();
    ids.sort_unstable();
    if ids.windows(2).any(|w| w[0] == w[1]) {
        die("unit table has duplicate ids");
    }
    for unit in &table {
        if !dimension_well_formed(unit.4) {
            die(&format!(
                "malformed dimension vector for {}: {}",
                unit.0, unit.4
            ));
        }
    }

    // category report, alphabetical
    let mut categories: std::collections::BTreeMap<&str, usize> = Default::default();
    for unit in &table {
        *categories.entry(unit.8).or_default() += 1;
    }
    let categories_text: Vec<String> = categories.iter().map(|(k, v)| format!("{k} {v}")).collect();

    println!(
        "unidpp-registry-kit: seeding {} UnitsML units into {}/units (register `{register}`)",
        units().len(),
        parsed.base()
    );
    println!("  categories: {}", categories_text.join(", "));
    println!();

    for unit in &table {
        println!("- {}", ensure_unit(&api, &register, unit));
    }

    let (count, expected, missing) = verify(&api, &register);
    println!();
    println!(
        "verify: GET /units?register={register} -> count={count} (expected {expected}) — {}",
        if missing.is_empty() { "OK" } else { "FAILED" }
    );
    if !missing.is_empty() {
        for m in &missing {
            println!("  missing: {m}");
        }
        std::process::exit(1);
    }

    println!();
    println!(
        "next:\n  curl -s '{}/units?register={register}' | jq '.count'\n  \
         curl -s '{}/units/unitsml:u:kilowatt_hour' | jq '.manifest'",
        parsed.base(),
        parsed.base()
    );
}
