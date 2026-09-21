//! `unidpp-kit seed-mappings` — register the GB 4943.1-2022 ↔ IEC
//! 62368-1 equivalence as a cross-register-mapping item, referentially
//! intact: both endpoint items are registered first (idempotently),
//! and the mapping is verified afterwards through the directional
//! lookup from both directions and by the named sides. No
//! `evidence_ref` is deposited: the claim carries the attester only —
//! add one through the registry API when your jurisdiction holds the
//! evidence document.

use serde_json::{json, Value};

use super::parse_seed_args;
use crate::die;

pub const USAGE: &str = "usage: unidpp-kit seed-mappings [--register R] \
[--port N] [--bind ADDR] [--base-url URL] [--admin-token T] [--token-file PATH]";

const MAPPING_ID: &str = "urn:unidpp:map:gb4943-iec62368";
const SUBMITTING_ORG: &str =
    "unidpp-registry-kit — GB↔IEC equivalence seed (cross-register-mapping class)";

fn endpoints() -> Vec<Value> {
    vec![
        json!({
            "register_id": "gb-std",
            "item_id": "gb-4943-1",
            "class": "data-element",
            "definition": "GB 4943.1-2022 — audio/video, information and \
                           communication technology equipment, part 1: safety \
                           requirements (Chinese national standard)",
            "version": "2022",
            "submitting_organization": "SAC (Standardization Administration of China)",
            "manifest": {
                "version": "2022",
                "designation": "GB 4943.1-2022",
                "name": "Audio/video, information and communication technology \
                         equipment — Part 1: Safety requirements",
            },
        }),
        json!({
            "register_id": "iec",
            "item_id": "iec-62368-1",
            "class": "data-element",
            "definition": "IEC 62368-1 — audio/video, information and \
                           communication technology equipment, part 1: safety \
                           requirements (international standard)",
            "version": "2018",
            "submitting_organization": "IEC/TC 108",
            "manifest": {
                "version": "2018",
                "designation": "IEC 62368-1:2018",
                "name": "Audio/video, information and communication technology \
                         equipment — Part 1: Safety requirements",
            },
        }),
    ]
}

fn ensure_item(api: &super::Registry, path: &str, body: &Value) -> String {
    let item_id = body["item_id"].as_str().unwrap_or_default();
    let (status, resp) = api.post(path, body);
    match status {
        201 => format!(
            "registered {item_id} (audit_seq {})",
            resp.get("audit_seq").cloned().unwrap_or(Value::Null)
        ),
        409 => format!("already registered {item_id}"),
        other => die(&format!(
            "item `{item_id}` failed (HTTP {other}): {}",
            serde_json::to_string(&resp)
                .unwrap_or_default()
                .chars()
                .take(400)
                .collect::<String>()
        )),
    }
}

fn lookup(api: &super::Registry, query: &str) -> Value {
    let (status, resp) = api.get(&format!("/cross-register-mappings?{query}"));
    if status != 200 {
        die(&format!(
            "lookup failed: /cross-register-mappings?{query} -> HTTP {status}"
        ));
    }
    resp
}

fn verify(api: &super::Registry) -> bool {
    let mut ok = true;
    let mut check = |query: &str, want: usize| {
        let resp = lookup(api, query);
        let mappings = resp
            .get("mappings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let got: Vec<&str> = mappings
            .iter()
            .filter_map(|m| m.get("identifier").and_then(Value::as_str))
            .collect();
        let good = mappings.len() == want && (want == 0 || got.contains(&MAPPING_ID));
        ok = ok && good;
        println!(
            "  {query:<42} count={} {}",
            mappings.len(),
            if good { "OK" } else { "FAILED" }
        );
    };

    println!("verify: directional lookups of {MAPPING_ID}");
    check("item=gb-4943-1", 1); // either direction, from the GB side
    check("item=iec-62368-1", 1); // either direction, from the IEC side
    check("source=gb-4943-1&target=iec-62368-1", 1); // the named sides
    check("target=gb-4943-1", 0); // gb-4943-1 is only ever a source here

    // the item itself resolves with the attested equivalence manifest
    let (status, resp) = api.get(&format!("/cross-register-mappings/{MAPPING_ID}"));
    if status != 200 {
        println!("  GET item -> HTTP {status} FAILED");
        return false;
    }
    let manifest = resp.get("manifest").cloned().unwrap_or(Value::Null);
    let attested = manifest.get("mapping_type").and_then(Value::as_str) == Some("equivalent")
        && manifest.get("attester").and_then(Value::as_str) == Some("CQC-pattern notified body");
    println!(
        "  {:<42} {}",
        "item manifest (equivalent, attested)",
        if attested { "OK" } else { "FAILED" }
    );
    ok && attested
}

pub fn run(kit: &crate::Kit, args: &[String]) {
    let mut register = "unidpp".to_string();
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

    println!(
        "unidpp-registry-kit: seeding GB 4943.1-2022 ↔ IEC 62368-1 as a \
         cross-register-mapping item into {} (register `{register}`)",
        parsed.base()
    );
    println!();

    // 1. referential integrity: both endpoints exist (idempotent)
    for endpoint in endpoints() {
        println!(
            "  endpoint {}: {}",
            endpoint["item_id"].as_str().unwrap_or_default(),
            ensure_item(&api, "/data-elements", &endpoint)
        );
    }
    // 2. the mapping itself, through the dedicated surface
    println!(
        "  mapping   {MAPPING_ID}: {}",
        ensure_item(
            &api,
            "/cross-register-mappings",
            &json!({
                "register_id": register,
                "item_id": MAPPING_ID,
                "definition": "GB 4943.1-2022 corresponds to IEC 62368-1 for the \
                               purposes of charger conformity evidence",
                "version": "1.0.0",
                "effective_from": "2026-09-01T00:00:00Z",
                "submitting_organization": SUBMITTING_ORG,
                "manifest": {
                    "version": "1.0.0",
                    "source": {"register": "gb-std", "item": "gb-4943-1",
                               "version": "2022"},
                    "target": {"register": "iec", "item": "iec-62368-1"},
                    "mapping_type": "equivalent",
                    "attester": "CQC-pattern notified body",
                },
            })
        )
    );
    println!();

    // 3. verify through the directional lookup
    if !verify(&api) {
        std::process::exit(1);
    }

    println!();
    println!(
        "next:\n  curl -s '{}/cross-register-mappings?item=iec-62368-1' | jq\n  \
         curl -s '{}/cross-register-mappings/{MAPPING_ID}' | jq '.manifest'",
        parsed.base(),
        parsed.base()
    );
}
