//! The jurisdiction demonstration (the port of `demo-jurisdiction.sh`):
//! the worked example — a fictional "DE" registry, end to end, against
//! the real API. The demonstration runs the full lifecycle a
//! jurisdictional registry peer goes through:
//!
//!   [1] bring up the registry (persistent journal, guarded admin)
//!   [2] seed jurisdiction DE (signed C3 self-descriptor, jurisdiction
//!       profile, applicability binding, sample data elements)
//!   [3] federation discovery — who serves the DE registry? (C3)
//!   [4] as-of applicability — which profiles bind product X at time T?
//!   [5] supersession — versioning a data element (19135 discipline)
//!   [6] enumeration resistance — what this registry does NOT expose
//!
//! Re-runnable: every step is idempotent (409s are tolerated, bindings
//! are checked before creation). Step [2] still delegates to
//! `bin/seed-jurisdiction.py`, exactly as the shell script did: the
//! seeding signs the C3 self-descriptor with an Ed25519 key, and that
//! operation lives in the Python seeding tool, not in this launcher.

use std::process::Command;
use std::time::Duration;

use serde_json::Value;

use crate::http;
use crate::{die, start_daemon, Kit};

const BANNER: usize = 70;
const PRODUCT_TYPE: &str = "gtin:4260123400019";
const ELEMENT: &str = "urn:unidpp:de:battery-carbon-footprint";

/// An ordered JSON view used only for display: `serde_json`'s map
/// sorts keys, while `jq` prints the keys in the order the query
/// names them, so the projections below carry their own order.
enum J {
    /// A value taken straight from a server document.
    V(Value),
    /// An object whose fields print in the given order.
    Obj(Vec<(&'static str, J)>),
    /// An array whose items print in the given order.
    Arr(Vec<J>),
}

impl J {
    fn render(&self, depth: usize) -> String {
        let pad = |depth: usize| "  ".repeat(depth);
        match self {
            J::V(value) => serde_json::to_string_pretty(value)
                .unwrap_or_else(|_| "null".into())
                .replace('\n', &format!("\n{}", pad(depth))),
            J::Obj(fields) if fields.is_empty() => "{}".into(),
            J::Obj(fields) => {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|(key, value)| {
                        format!("{}\"{key}\": {}", pad(depth + 1), value.render(depth + 1))
                    })
                    .collect();
                format!("{{\n{}\n{}}}", inner.join(",\n"), pad(depth))
            }
            J::Arr(items) if items.is_empty() => "[]".into(),
            J::Arr(items) => {
                let inner: Vec<String> = items
                    .iter()
                    .map(|item| format!("{}{}", pad(depth + 1), item.render(depth + 1)))
                    .collect();
                format!("[\n{}\n{}]", inner.join(",\n"), pad(depth))
            }
        }
    }
}

fn show_j(view: &J) {
    println!("{}", view.render(0));
}

/// A field of a document, as `jq` yields it: a missing field is null.
fn field(doc: &Value, pointer: &str) -> Value {
    doc.pointer(pointer).cloned().unwrap_or(Value::Null)
}

fn as_text(doc: &Value, pointer: &str) -> String {
    doc.pointer(pointer)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn hr(title: &str) {
    println!();
    println!("{}", "=".repeat(BANNER));
    println!("{title}");
    println!("{}", "=".repeat(BANNER));
}

fn show(cmd: &str) {
    println!("$ {cmd}");
}

/// `curl -sf` under `set -e`: a connection failure or an HTTP error
/// ends the demonstration.
fn get_json(kit: &Kit, path: &str) -> Value {
    let resp = http::get("127.0.0.1", kit.port(), path, Duration::from_secs(10))
        .unwrap_or_else(|| die(&format!("no answer from the registry for GET {path}")));
    if resp.status >= 400 {
        die(&format!("GET {path} returned HTTP {}", resp.status));
    }
    serde_json::from_str::<Value>(&resp.body)
        .unwrap_or_else(|e| die(&format!("GET {path} returned invalid JSON: {e}")))
}

pub fn run(kit: &Kit, jurisdiction: &str) {
    let base = format!("http://127.0.0.1:{}", kit.port());
    // The admin token for the supersession POST: the file the launcher
    // generated (honouring KIT_HOME), or KIT_ADMIN_TOKEN when the
    // operator supplied one and no file exists yet.
    let token = std::fs::read_to_string(kit.token_file())
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| {
            std::env::var("KIT_ADMIN_TOKEN")
                .ok()
                .filter(|t| !t.is_empty())
        })
        .unwrap_or_default();
    let element_path = format!("/data-elements/{ELEMENT}");

    hr("[1] bring up the registry");
    start_daemon(kit);

    hr(&format!("[2] seed jurisdiction {jurisdiction}"));
    let script = kit.root().join("bin").join("seed-jurisdiction.py");
    let seeded = Command::new("python3")
        .arg(&script)
        .args(["--jurisdiction", jurisdiction])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !seeded {
        die(&format!(
            "seeding failed (python3 {} --jurisdiction {jurisdiction})",
            script.display()
        ));
    }

    hr(&format!(
        "[3] federation discovery: who serves the {jurisdiction} registry? (C3)"
    ));
    show(&format!(
        "curl -s '{base}/services?jurisdiction={jurisdiction}&class=registry' | jq"
    ));
    let doc = get_json(
        kit,
        &format!("/services?jurisdiction={jurisdiction}&class=registry"),
    );
    let service = field(&doc, "/services/0");
    show_j(&J::Obj(vec![
        ("identifier", J::V(field(&service, "/identifier"))),
        ("class", J::V(field(&service, "/version/body/class"))),
        (
            "jurisdiction",
            J::V(field(&service, "/version/body/jurisdiction")),
        ),
        (
            "endpoints",
            J::V(field(&service, "/version/body/endpoints")),
        ),
        (
            "operator",
            J::V(field(&service, "/version/body/operator/id")),
        ),
        (
            "signed",
            J::V(Value::String(format!(
                "{}/{}",
                as_text(&service, "/version/signature/algorithm"),
                as_text(&service, "/version/signature/key_id")
            ))),
        ),
    ]));

    hr(&format!(
        "[4] as-of applicability: which profiles bind {PRODUCT_TYPE} at time T?"
    ));
    println!("-- at 2026-06-01 (before the obligation starts):");
    show(&format!(
        "curl -s '{base}/applicability?product_type={PRODUCT_TYPE}&at=2026-06-01T00:00:00Z'"
    ));
    let doc = get_json(
        kit,
        &format!("/applicability?product_type={PRODUCT_TYPE}&at=2026-06-01T00:00:00Z"),
    );
    show_j(&J::Obj(vec![
        ("at", J::V(field(&doc, "/as_of"))),
        (
            "applies",
            J::V(Value::from(
                doc.get("applicability")
                    .and_then(|a| a.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0),
            )),
        ),
    ]));
    println!("-- at 2027-06-01 (the binding is in force):");
    show(&format!(
        "curl -s '{base}/applicability?product_type={PRODUCT_TYPE}&at=2027-06-01T00:00:00Z'"
    ));
    let doc = get_json(
        kit,
        &format!("/applicability?product_type={PRODUCT_TYPE}&at=2027-06-01T00:00:00Z"),
    );
    let applies: Vec<J> = doc
        .get("applicability")
        .and_then(|a| a.as_array())
        .map(|entries| {
            entries
                .iter()
                .map(|entry| {
                    J::Obj(vec![
                        ("profile", J::V(field(entry, "/binding/profile_item"))),
                        ("from", J::V(field(entry, "/binding/effective_from"))),
                        ("retroactive", J::V(field(entry, "/binding/retroactive"))),
                    ])
                })
                .collect()
        })
        .unwrap_or_default();
    show_j(&J::Obj(vec![
        ("at", J::V(field(&doc, "/as_of"))),
        ("applies", J::Arr(applies)),
    ]));

    hr("[5] supersession: versioning a data element (19135 discipline)");
    println!("-- current registered version (v1.0.0 on first run; v2.0.0 on re-runs):");
    show(&format!("curl -s '{base}{element_path}' | jq '.version'"));
    let doc = get_json(kit, &element_path);
    let version = field(&doc, "/version");
    show_j(&J::Obj(vec![
        ("version", J::V(field(&version, "/version"))),
        ("status", J::V(field(&version, "/status"))),
        ("effective_from", J::V(field(&version, "/effective_from"))),
    ]));
    println!("-- register v2.0.0, effective 2028-01-01, with reason:");
    show(&format!(
        "curl -XPOST '{base}{element_path}/versions' -d '{{\"version\": \"2.0.0\", ...}}'"
    ));
    let body = serde_json::json!({
        "version": "2.0.0",
        "reason": "align representation with ESDC data-space conventions; pin unit identity",
        "effective_from": "2028-01-01T00:00:00Z"
    });
    let code = http::post_auth(
        "127.0.0.1",
        kit.port(),
        &format!("{element_path}/versions"),
        &body.to_string(),
        &token,
        Duration::from_secs(15),
    )
    .map(|r| r.status)
    .unwrap_or(0);
    match code {
        201 => println!("   201 Created — old version superseded"),
        409 => println!("   409 — v2.0.0 already registered (idempotent re-run)"),
        other => {
            println!("   unexpected HTTP {other}");
            std::process::exit(1);
        }
    }
    println!("-- as-of 2027-06-01: v1.0.0 still in force, window end derived from successor:");
    show(&format!(
        "curl -s '{base}{element_path}?at=2027-06-01T00:00:00Z' | jq .version"
    ));
    let doc = get_json(kit, &format!("{element_path}?at=2027-06-01T00:00:00Z"));
    let version = field(&doc, "/version");
    show_j(&J::Obj(vec![
        ("version", J::V(field(&version, "/version"))),
        ("status", J::V(field(&version, "/status"))),
        (
            "superseded_by_version",
            J::V(field(&version, "/superseded_by_version")),
        ),
        ("window_end", J::V(field(&version, "/window_end"))),
    ]));
    println!("-- as-of 2028-06-01: v2.0.0 in force:");
    let doc = get_json(kit, &format!("{element_path}?at=2028-06-01T00:00:00Z"));
    let version = field(&doc, "/version");
    show_j(&J::Obj(vec![
        ("version", J::V(field(&version, "/version"))),
        ("status", J::V(field(&version, "/status"))),
        ("effective_from", J::V(field(&version, "/effective_from"))),
    ]));
    println!("-- the supersession chain:");
    show(&format!(
        "curl -s '{base}{element_path}/supersession' | jq .chain"
    ));
    let doc = get_json(kit, &format!("{element_path}/supersession"));
    let chain: Vec<J> = doc
        .get("chain")
        .and_then(|c| c.as_array())
        .map(|entries| {
            entries
                .iter()
                .map(|entry| {
                    J::Obj(vec![
                        ("version", J::V(field(entry, "/version"))),
                        ("status", J::V(field(entry, "/status"))),
                        (
                            "superseded_by_version",
                            J::V(field(entry, "/superseded_by_version")),
                        ),
                        ("window_end", J::V(field(entry, "/window_end"))),
                    ])
                })
                .collect()
        })
        .unwrap_or_default();
    show_j(&J::Obj(vec![("chain", J::Arr(chain))]));

    hr("[6] enumeration resistance: what this registry does NOT expose");
    print!("{}", POSTURE);
    println!("-- a) query applicability WITHOUT a product identity:");
    show(&format!("curl -s '{base}/applicability'"));
    let resp = http::get(
        "127.0.0.1",
        kit.port(),
        "/applicability",
        Duration::from_secs(5),
    );
    let code = resp.as_ref().map(|r| r.status).unwrap_or(0);
    println!("   HTTP {code}");
    if let Some(resp) = resp {
        let shown: String = resp.body.chars().take(200).collect();
        println!("{shown}");
    }
    println!("-- b) read the audit log WITHOUT the operator credential:");
    show(&format!(
        "curl -s '{base}/admin/log'   (no Authorization header)"
    ));
    let code = http::get(
        "127.0.0.1",
        kit.port(),
        "/admin/log",
        Duration::from_secs(5),
    )
    .map(|r| r.status)
    .unwrap_or(0);
    println!("   HTTP {code}");
    println!("-- c) the complete public route table (GET /) — every surface offered:");
    show(&format!("curl -s '{base}/' | jq -r '.endpoints[]'"));
    let doc = get_json(kit, "/");
    if let Some(endpoints) = doc.get("endpoints").and_then(|e| e.as_object()) {
        for value in endpoints.values() {
            if let Some(route) = value.as_str() {
                println!("   {route}");
            }
        }
    }
    println!("   Zero endpoints list identifiers-in-use; the only product-identity");
    println!("   read (/applicability) requires the identity as input. /items lists");
    println!("   registered DEFINITIONS (public 19135 items), never records of things.");

    let self_name = std::env::args()
        .next()
        .and_then(|a| {
            std::path::PathBuf::from(a)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "unidpp-kit".into());
    hr(&format!(
        "done — registry still running on {base} ({self_name} stop to stop)"
    ));
}

const POSTURE: &str = r#"  The registry holds DESCRIPTORS (definitions, profiles, service
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
"#;
