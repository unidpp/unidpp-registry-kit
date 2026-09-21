//! `unidpp-kit seed-jurisdiction` — register a jurisdiction's registry
//! as a federation peer (memo work item R3). Given a two-letter
//! jurisdiction code this seeds, through the unidpp-registry HTTP API:
//! the signed C3 self-descriptor (how other peers discover and bind to
//! this registry), the jurisdictional battery DPP profile item in its
//! SIGNED form (PR-1), the sample data elements the profile's data
//! points reference, and the dated applicability binding. Re-running
//! against the same journal is safe: already-registered items are
//! skipped on 409, and the binding is only created when the profile
//! does not already apply to the subject.

use serde_json::{json, Value};

use super::{parse_seed_args, quote, Operator};
use crate::die;

pub const USAGE: &str = "usage: unidpp-kit seed-jurisdiction --jurisdiction DE \
[--port N] [--bind ADDR] [--base-url URL] [--endpoint-uri URI] \
[--admin-token T] [--token-file PATH] [--operator-label L] \
[--register R] [--product-type ID]";

/// The residency table is illustrative, not normative; a jurisdiction
/// may use any string its profile defines.
fn residency_for(jur: &str) -> &str {
    match jur {
        "DE" | "FR" | "NL" | "IT" | "ES" | "SE" | "PL" | "GB" | "CH" | "NO" => "eu",
        "JP" | "KR" => "apac",
        "CN" => "cn",
        "US" | "CA" => "na",
        _ => "anywhere",
    }
}

fn ensure_service(
    api: &super::Registry,
    operator: &Operator,
    jur: &str,
    endpoint_uri: &str,
    residency: &str,
) -> String {
    let identifier = format!("registry-{}-v1", jur.to_lowercase());
    let body = json!({
        "identifier": identifier,
        "operator": operator.record(),
        "class": "registry",
        "endpoints": [{"uri": endpoint_uri,
                       "protocol_binding_ref": super::DEFAULT_PROTOCOL_BINDING}],
        "protocol_binding_ref": super::DEFAULT_PROTOCOL_BINDING,
        "jurisdiction": jur,
        "residency_class": residency,
        "status": "active",
        "service_kind": "jurisdictional-registry",
        "succession_pointer": Value::Null,
    });
    let signed = operator.sign(&body);
    let (status, resp) = api.post(
        "/services",
        &json!({
            "identifier": identifier,
            "version": "1.0.0",
            "effective_from": "2026-09-01T00:00:00Z",
            "body": signed,
            "signature": signed["signature"],
        }),
    );
    match status {
        201 => format!(
            "registered C3 self-descriptor `{identifier}` (audit_seq {})",
            resp.get("audit_seq").cloned().unwrap_or(Value::Null)
        ),
        409 => format!("C3 self-descriptor `{identifier}` already registered"),
        other => die(&format!(
            "service registration failed (HTTP {other}): {}",
            serde_json::to_string(&resp)
                .unwrap_or_default()
                .chars()
                .take(400)
                .collect::<String>()
        )),
    }
}

struct SampleElement {
    item_id: &'static str,
    definition: &'static str,
    provenance: &'static str,
    granularity: &'static str,
    trust_floor: &'static str,
    min_capability: &'static str,
    manifest: Value,
}

fn sample_elements() -> Vec<SampleElement> {
    // Element ids follow the pilot convention (`de/m/...` in
    // unidpp-pilot-data): the `de:` URN namespace is the *data
    // element* namespace, jurisdiction-neutral — the jurisdiction is
    // carried by the register (`jurisdiction-<code>`), which is what
    // makes the same concept harmonizable across national
    // subregisters (memo F4).
    vec![
        SampleElement {
            item_id: "urn:unidpp:de:battery-carbon-footprint",
            definition: "Carbon footprint of the battery, declared per the \
                         applicable delegated regulation (kg CO2e per battery, \
                         life-cycle)",
            provenance: "attested",
            granularity: "instance",
            trust_floor: "attested",
            min_capability: "S1",
            manifest: json!({
                "version": "1.0.0", "unit_ref": "unit-kg",
                "datatype": "decimal(9,3)",
                "representation": "ISO 80000-1 quantity value",
                "source_register": "sample national RA entry",
            }),
        },
        SampleElement {
            item_id: "urn:unidpp:de:battery-recycled-content",
            definition: "Share of recycled content recovered from waste in \
                         the active material of the battery, percent by mass",
            provenance: "attested",
            granularity: "type",
            trust_floor: "attested",
            min_capability: "S1",
            manifest: json!({
                "version": "1.0.0", "unit_ref": Value::Null,
                "datatype": "decimal(5,2)",
                "representation": "percentage 0-100",
                "source_register": "sample national RA entry",
            }),
        },
        SampleElement {
            item_id: "urn:unidpp:de:battery-due-diligence-statement",
            definition: "Reference to the due-diligence statement and \
                         verification report covering raw-material supply \
                         chains of the battery",
            provenance: "log_anchored",
            granularity: "type",
            trust_floor: "log_anchored",
            min_capability: "S1",
            manifest: json!({
                "version": "1.0.0", "unit_ref": Value::Null,
                "datatype": "uri",
                "representation": "signed document reference",
                "source_register": "sample national RA entry",
            }),
        },
    ]
}

fn ensure_elements(api: &super::Registry, jur: &str, register: &str, elements: &[SampleElement]) {
    for el in elements {
        let body = json!({
            "register_id": register,
            "item_id": el.item_id,
            "definition": el.definition,
            "version": "1.0.0",
            "effective_from": "2026-10-01T00:00:00Z",
            "submitting_organization": format!("{jur} national registry authority (kit sample)"),
            "manifest": el.manifest,
        });
        let (status, resp) = api.post("/data-elements", &body);
        match status {
            201 => println!(
                "- registered data element `{}` (audit_seq {})",
                el.item_id,
                resp.get("audit_seq").cloned().unwrap_or(Value::Null)
            ),
            409 => println!("- data element `{}` already registered", el.item_id),
            other => die(&format!(
                "data element `{}` failed (HTTP {other}): {}",
                el.item_id,
                serde_json::to_string(&resp)
                    .unwrap_or_default()
                    .chars()
                    .take(400)
                    .collect::<String>()
            )),
        }
    }
}

/// Returns (item id, the operator-facing line).
fn ensure_profile(
    api: &super::Registry,
    operator: &Operator,
    jur: &str,
    register: &str,
    elements: &[SampleElement],
) -> (String, String) {
    let item_id = format!("urn:unidpp:profile:{}-battery-passport", jur.to_lowercase());
    let data_points: Vec<Value> = elements
        .iter()
        .map(|e| {
            json!({
                "element": e.item_id,
                "cardinality": "1",
                "min_capability": e.min_capability,
                "required_provenance": e.provenance,
                "subject_granularity": e.granularity,
                "trust_floor": e.trust_floor,
            })
        })
        .collect();
    let manifest = operator.sign_manifest(&json!({
        "version": "1.0.0",
        "axes": ["jurisdiction"],
        "data_points": data_points,
        "custody": {"default_model": "identity_preserved",
                    "standard": "ISO 22095:2020"},
        "legal_basis": [{
            "instrument": "Regulation (EU) 2023/1542 (batteries) Art. 77 — \
                           digital product passport",
            "force": "binding",
            "citation": "obligation applies from 2027-02-18",
        }],
        "triggers": [{
            "description": "battery of a covered category placed on the market",
            "evaluation_mode": "on_issuance",
            "predicate_class": "fact_predicate",
            "predicate_ref": "pred/battery-dpp-obligation",
        }],
    }));
    let body = json!({
        "register_id": register,
        "item_id": item_id,
        "definition": format!(
            "{jur} jurisdictional battery DPP profile: placement obligation, \
             carbon footprint, recycled content and due-diligence data points \
             for batteries placed on the {jur} market"),
        "version": "1.0.0",
        "effective_from": "2027-02-18T00:00:00Z",
        "submitting_organization": format!("{jur} national registry authority (kit sample)"),
        "manifest": manifest,
    });
    let (status, resp) = api.post("/profiles", &body);
    match status {
        201 => (
            item_id.clone(),
            format!(
                "registered profile `{item_id}` (audit_seq {})",
                resp.get("audit_seq").cloned().unwrap_or(Value::Null)
            ),
        ),
        409 => (
            item_id.clone(),
            format!("profile `{item_id}` already registered"),
        ),
        other => die(&format!(
            "profile registration failed (HTTP {other}): {}",
            serde_json::to_string(&resp)
                .unwrap_or_default()
                .chars()
                .take(400)
                .collect::<String>()
        )),
    }
}

fn ensure_binding(api: &super::Registry, profile_id: &str, product_type: &str) -> String {
    let eff = "2027-02-18T00:00:00Z";
    // Idempotency check through the same identity-keyed read the
    // runtime uses: ask whether the profile already applies to this
    // subject.
    let (status, resp) = api.get(&format!(
        "/applicability?product_type={}&at={}",
        quote(product_type),
        quote(eff)
    ));
    if status == 200 {
        if let Some(entries) = resp.get("applicability").and_then(Value::as_array) {
            for entry in entries {
                if entry
                    .get("binding")
                    .and_then(|b| b.get("profile_item"))
                    .and_then(Value::as_str)
                    == Some(profile_id)
                {
                    return format!("applicability binding already in force for `{product_type}`");
                }
            }
        }
    }
    let (status, resp) = api.post(
        "/applicability",
        &json!({
            "profile_id": profile_id,
            "product_type": product_type,
            "effective_from": eff,
            "retroactive": false,
        }),
    );
    match status {
        201 => format!(
            "bound profile to `{product_type}` from {eff} (audit_seq {}, non-retroactive)",
            resp.get("audit_seq").cloned().unwrap_or(Value::Null)
        ),
        other => die(&format!(
            "applicability binding failed (HTTP {other}): {}",
            serde_json::to_string(&resp)
                .unwrap_or_default()
                .chars()
                .take(400)
                .collect::<String>()
        )),
    }
}

/// The seeding itself, shared by the subcommand and the
/// demonstration: `base` is the registry URL the prints and the
/// default endpoint URI name.
pub fn seed(
    api: &super::Registry,
    base: &str,
    jurisdiction: &str,
    operator_label: &str,
    register_override: Option<&str>,
    product_type: &str,
    endpoint_uri_override: Option<&str>,
) {
    let jur = jurisdiction.to_uppercase();
    if jur.len() != 2 || !jur.bytes().all(|b| b.is_ascii_uppercase()) {
        die("jurisdiction must be an ISO 3166-1 alpha-2 code (e.g. DE)");
    }
    api.require_health();

    let operator = Operator::new(operator_label);
    let register = register_override
        .map(str::to_string)
        .unwrap_or_else(|| format!("jurisdiction-{}", jur.to_lowercase()));
    let residency = residency_for(&jur);
    let endpoint_uri = endpoint_uri_override
        .map(str::to_string)
        .unwrap_or_else(|| format!("{base}/"));

    println!("unidpp-registry-kit: seeding jurisdiction {jur} against {base}");
    println!(
        "  operator:  {operator_label} (id {}, key {})",
        operator.id(),
        operator.key_id()
    );
    println!("  register:  {register} (the national subregister, F4)");
    println!();

    println!(
        "- {}",
        ensure_service(api, &operator, &jur, &endpoint_uri, residency)
    );

    let elements = sample_elements();
    ensure_elements(api, &jur, &register, &elements);

    let (profile_id, profile_line) = ensure_profile(api, &operator, &jur, &register, &elements);
    println!("- {profile_line}");

    println!("- {}", ensure_binding(api, &profile_id, product_type));

    println!();
    println!("next (see unidpp-kit demo-jurisdiction):");
    println!("  curl -s '{base}/services?jurisdiction={jur}&class=registry' | jq");
    println!(
        "  curl -s '{base}/applicability?product_type={}&at=2027-06-01T00:00:00Z' | jq",
        quote(product_type)
    );
}

pub fn run(kit: &crate::Kit, args: &[String]) {
    let mut jurisdiction = String::new();
    let mut operator_label = super::DEFAULT_OPERATOR_LABEL.to_string();
    let mut register = None;
    let mut product_type = "gtin:4260123400019".to_string();
    let mut endpoint_uri = None;
    let mut shared = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--jurisdiction" => jurisdiction = rest.next().unwrap_or_else(|| die(USAGE)).clone(),
            "--operator-label" => {
                operator_label = rest.next().unwrap_or_else(|| die(USAGE)).clone()
            }
            "--register" => register = Some(rest.next().unwrap_or_else(|| die(USAGE)).clone()),
            "--product-type" => product_type = rest.next().unwrap_or_else(|| die(USAGE)).clone(),
            "--endpoint-uri" => {
                endpoint_uri = Some(rest.next().unwrap_or_else(|| die(USAGE)).clone())
            }
            _ => shared.push(arg.clone()),
        }
    }
    if jurisdiction.is_empty() {
        die(USAGE);
    }
    let parsed = parse_seed_args(kit, &shared, USAGE);
    let api = parsed.registry(kit);
    seed(
        &api,
        &parsed.base(),
        &jurisdiction,
        &operator_label,
        register.as_deref(),
        &product_type,
        endpoint_uri.as_deref(),
    );
}
