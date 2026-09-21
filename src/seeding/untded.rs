//! `unidpp-kit seed-untded` — register the UNTDED 2005 trade
//! data-element directory (memo work item T-05) from its single
//! source of truth, the untded-2005 repository (ECE/TRADE/362 =
//! ISO 7372:2005): every element becomes an ISO 19135 item in the
//! `untded` register, with the representation decomposed, the D05B
//! UN/EDIFACT join carried as a manifest property (the vintage caveat
//! stated, never silently normalized), and the source lifecycle
//! (active/retired) kept beside the directory's own replacement
//! notes. Fields with no clean one-line mapping ride the manifest
//! verbatim, never dropped.

use serde_json::{json, Map, Value};

use super::parse_seed_args;
use crate::die;

pub const USAGE: &str = "usage: unidpp-kit seed-untded [--register R] [--untded-dir DIR] \
[--status active|retired|all] [--limit N] \
[--port N] [--bind ADDR] [--base-url URL] [--admin-token T] [--token-file PATH]";

const SUBMITTING_ORG: &str =
    "UNTDED 2005 (ECE/TRADE/362 = ISO 7372:2005) — untded-2005 SSOT import";
const EFFECTIVE_FROM: &str = "2005-01-01T00:00:00Z";
const KIT_VERSION: &str = "1.0.0";
const UNTDED_CITATION: &str =
    "UNTDED 2005, ECE/TRADE/362 (ISO 7372:2005), §4.2 Trade Data Elements Directory";
const EDIFACT_LINKS_REL: &str = "derived/edifact-links.json";
const EDIFACT_SOURCE_SHORT: &str =
    "UN/EDIFACT D.05B segments mirror — untded-2005 derived/edifact-links.json";

fn default_untded_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let configured = std::env::var("UNTDED_DIR").unwrap_or_default();
    if !configured.is_empty() {
        return configured;
    }
    format!("{home}/src/untded/untded-2005")
}

/// serde_yaml values into serde_json values (the SSOT's shapes:
/// mappings, sequences, strings, integers, booleans, nulls).
fn yaml_to_json(value: serde_yaml::Value) -> Value {
    match value {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(b) => json!(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                json!(i)
            } else if let Some(u) = n.as_u64() {
                json!(u)
            } else {
                json!(n.as_f64().unwrap_or_default())
            }
        }
        serde_yaml::Value::String(s) => json!(s),
        serde_yaml::Value::Sequence(seq) => {
            json!(seq.into_iter().map(yaml_to_json).collect::<Vec<_>>())
        }
        serde_yaml::Value::Mapping(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                let key = match k {
                    serde_yaml::Value::String(s) => s,
                    other => serde_yaml::to_string(&other)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                };
                out.insert(key, yaml_to_json(v));
            }
            Value::Object(out)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(tagged.value),
    }
}

fn load_elements(untded_dir: &str) -> (Vec<Value>, std::collections::BTreeMap<String, usize>) {
    let root = std::path::Path::new(untded_dir)
        .join("data")
        .join("elements");
    if !root.is_dir() {
        die(&format!(
            "no data/elements directory under {untded_dir} — pass --untded-dir \
             (the untded-2005 SSOT checkout)"
        ));
    }
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| die(&format!("cannot list {}: {e}", root.display())))
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| n.ends_with(".yaml"))
        .collect();
    names.sort();
    let mut elements: Vec<(i64, Value)> = Vec::new();
    let mut categories: std::collections::BTreeMap<String, usize> = Default::default();
    for name in names {
        let text = std::fs::read_to_string(root.join(&name))
            .unwrap_or_else(|e| die(&format!("cannot read {name}: {e}")));
        let doc: serde_yaml::Value = serde_yaml::from_str(&text)
            .unwrap_or_else(|e| die(&format!("cannot parse {name}: {e}")));
        let category = doc
            .get("category")
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        let empty = Vec::new();
        for el in doc
            .get("elements")
            .and_then(|e| e.as_sequence())
            .unwrap_or(&empty)
        {
            let mut el = yaml_to_json(el.clone());
            if let Some(obj) = el.as_object_mut() {
                obj.insert("_category".into(), json!(category));
            }
            let tag = el
                .get("tag")
                .and_then(Value::as_i64)
                .unwrap_or_else(|| die(&format!("element without an integer tag in {name}")));
            *categories.entry(category.clone()).or_default() += 1;
            elements.push((tag, el));
        }
    }
    elements.sort_by_key(|(tag, _)| *tag);
    (elements.into_iter().map(|(_, e)| e).collect(), categories)
}

fn load_edifact_links(untded_dir: &str) -> std::collections::BTreeMap<String, Value> {
    let path = std::path::Path::new(untded_dir).join(EDIFACT_LINKS_REL);
    if !path.is_file() {
        println!(
            "  note: {} not found — seeding without un_edifact_ref \
             (run `bin/join-edifact` in the SSOT to derive it)",
            path.display()
        );
        return Default::default();
    }
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", path.display())));
    let doc: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| die(&format!("cannot parse {}: {e}", path.display())));
    let mut out = std::collections::BTreeMap::new();
    for link in doc
        .get("links")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let tag = link
            .get("tag")
            .map(|t| match t {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();
        out.insert(tag, link);
    }
    out
}

fn insert_opt(manifest: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(v) = value {
        if !v.is_null() {
            manifest.insert(key.into(), v);
        }
    }
}

fn build_item(
    el: &Value,
    register: &str,
    edifact: &std::collections::BTreeMap<String, Value>,
) -> Value {
    let tag = el.get("tag").and_then(Value::as_i64).unwrap_or_default();
    let retired = el.get("status").and_then(Value::as_str) == Some("retired");
    let name = el
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| el.get("old_name").and_then(Value::as_str))
        .unwrap_or_default();
    let mut manifest = Map::new();
    manifest.insert("version".into(), json!(KIT_VERSION));
    manifest.insert("tag".into(), json!(tag));
    insert_opt(&mut manifest, "name", el.get("name").cloned());
    let rep = el.get("representation").cloned().unwrap_or(Value::Null);
    if !rep.is_null() {
        insert_opt(&mut manifest, "representation", rep.get("raw").cloned());
        insert_opt(&mut manifest, "charset", rep.get("charset").cloned());
        insert_opt(&mut manifest, "min_length", rep.get("min_length").cloned());
        insert_opt(&mut manifest, "max_length", rep.get("max_length").cloned());
    }
    let link = edifact.get(&tag.to_string());
    if let Some(link) = link {
        if let Some(edifact_name) = link
            .get("edifact")
            .and_then(|e| e.get("name"))
            .and_then(Value::as_str)
        {
            manifest.insert("un_edifact_ref".into(), json!(edifact_name));
            if let Some(aligned) = link.get("aligned") {
                manifest.insert(
                    "aligned_with_edifact".into(),
                    json!(!aligned.is_null() && aligned != &json!(false)),
                );
            }
        }
    }
    let page = el
        .get("provenance")
        .and_then(|p| p.get("page"))
        .cloned()
        .unwrap_or(json!("?"));
    manifest.insert(
        "untded_ref".into(),
        json!(format!(
            "{UNTDED_CITATION}, element {tag} (p. {page}); https://www.untded.org/elements/{tag}"
        )),
    );
    manifest.insert(
        "untded_status".into(),
        el.get("status").cloned().unwrap_or(json!("active")),
    );
    insert_opt(&mut manifest, "change_tag", el.get("change_tag").cloned());
    insert_opt(&mut manifest, "old_name", el.get("old_name").cloned());
    insert_opt(
        &mut manifest,
        "business_term",
        el.get("business_term").cloned(),
    );
    insert_opt(&mut manifest, "notes", el.get("notes").cloned());
    insert_opt(&mut manifest, "bridges", el.get("bridges").cloned());
    insert_opt(&mut manifest, "category", el.get("_category").cloned());
    if let Some(p) = el.get("provenance") {
        if p.get("page").is_some() {
            manifest.insert(
                "source_page".into(),
                p.get("page").cloned().unwrap_or(Value::Null),
            );
        }
    }

    let definition = if retired {
        let note = el
            .get("notes")
            .and_then(Value::as_str)
            .unwrap_or("No replacement stated in the directory.");
        format!("{name} — retired in UNTDED 2005. {note}")
    } else {
        el.get("description")
            .and_then(Value::as_str)
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| name.to_string())
    };
    json!({
        "register_id": register,
        "item_id": format!("urn:untded:de:{tag}"),
        "class": "data-element",
        "title": if name.is_empty() { format!("UNTDED data element {tag}") } else { name.to_string() },
        "definition": definition,
        "version": KIT_VERSION,
        "effective_from": EFFECTIVE_FROM,
        "submitting_organization": SUBMITTING_ORG,
        "manifest": Value::Object(manifest),
    })
}

fn verify(
    api: &super::Registry,
    register: &str,
    expected: &std::collections::BTreeSet<String>,
) -> (Value, usize, Vec<String>) {
    let (status, resp) = api.get(&format!("/data-elements?register={register}"));
    if status != 200 {
        die(&format!(
            "verification failed: GET /data-elements -> HTTP {status}"
        ));
    }
    let listed: std::collections::BTreeSet<String> = resp
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|i| i.get("identifier").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    let missing: Vec<String> = expected.difference(&listed).cloned().collect();
    (
        resp.get("count").cloned().unwrap_or(Value::Null),
        expected.len(),
        missing,
    )
}

pub fn run(kit: &crate::Kit, args: &[String]) {
    let mut register = "untded".to_string();
    let mut untded_dir = default_untded_dir();
    let mut status_filter = "all".to_string();
    let mut limit: Option<usize> = None;
    let mut shared = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--register" => register = rest.next().unwrap_or_else(|| die(USAGE)).clone(),
            "--untded-dir" => untded_dir = rest.next().unwrap_or_else(|| die(USAGE)).clone(),
            "--status" => {
                status_filter = rest.next().unwrap_or_else(|| die(USAGE)).clone();
                if !matches!(status_filter.as_str(), "active" | "retired" | "all") {
                    die("--status is one of: active, retired, all");
                }
            }
            "--limit" => {
                let value = rest.next().unwrap_or_else(|| die(USAGE));
                limit =
                    Some(value.parse().unwrap_or_else(|_| {
                        die(&format!("--limit must be a number (got {value})"))
                    }));
            }
            _ => shared.push(arg.clone()),
        }
    }
    let parsed = parse_seed_args(kit, &shared, USAGE);
    let api = parsed.registry(kit);
    api.require_health();

    let (mut elements, categories) = load_elements(&untded_dir);
    if status_filter != "all" {
        elements.retain(|e| e.get("status").and_then(Value::as_str) == Some(&status_filter));
    }
    if let Some(limit) = limit {
        elements.truncate(limit);
    }
    let edifact = load_edifact_links(&untded_dir);

    // internal consistency: unique tags
    let tags: Vec<i64> = elements
        .iter()
        .filter_map(|e| e.get("tag").and_then(Value::as_i64))
        .collect();
    let mut sorted = tags.clone();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.len() != tags.len() {
        die("duplicate tags in the UNTDED dataset");
    }

    let active = elements
        .iter()
        .filter(|e| e.get("status").and_then(Value::as_str) == Some("active"))
        .count();
    let linked = elements
        .iter()
        .filter(|e| {
            let tag = e.get("tag").and_then(Value::as_i64).unwrap_or_default();
            edifact.contains_key(&tag.to_string())
        })
        .count();
    let categories_text: Vec<String> = categories.iter().map(|(k, v)| format!("{k} {v}")).collect();

    println!(
        "unidpp-registry-kit: seeding {} UNTDED 2005 data elements into \
         {}/data-elements (register `{register}`)",
        elements.len(),
        parsed.base()
    );
    println!(
        "  lifecycle: {active} active / {} retired (source states; every item \
         registers 19135-status valid)",
        elements.len() - active
    );
    println!(
        "  un_edifact_ref: {linked} of {} (D05B join: {EDIFACT_SOURCE_SHORT})",
        elements.len()
    );
    println!("  categories: {}", categories_text.join(", "));
    println!();

    let total = elements.len();
    let mut registered = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    for (i, el) in elements.iter().enumerate() {
        let body = build_item(el, &register, &edifact);
        let item_id = body["item_id"].as_str().unwrap_or_default().to_string();
        let (status, resp) = api.post("/data-elements", &body);
        match status {
            201 => registered += 1,
            409 => skipped += 1,
            other => {
                failed += 1;
                println!(
                    "  FAILED: {item_id} failed (HTTP {other}): {}",
                    serde_json::to_string(&resp)
                        .unwrap_or_default()
                        .chars()
                        .take(400)
                        .collect::<String>()
                );
            }
        }
        let n = i + 1;
        if n % 250 == 0 || n == total {
            println!(
                "  {n}/{total} — {registered} registered, {skipped} already present, {failed} failed"
            );
        }
    }

    let expected: std::collections::BTreeSet<String> =
        tags.iter().map(|t| format!("urn:untded:de:{t}")).collect();
    let (count, expected_len, missing) = verify(&api, &register, &expected);
    println!();
    println!(
        "verify: GET /data-elements?register={register} -> count={count} \
         (expected {expected_len}) — {}",
        if missing.is_empty() { "OK" } else { "FAILED" }
    );
    if !missing.is_empty() {
        for m in missing.iter().take(10) {
            println!("  missing: {m}");
        }
        std::process::exit(1);
    }
    if failed > 0 {
        die(&format!("{failed} elements failed to register"));
    }

    println!();
    println!(
        "next:\n  curl -s '{}/data-elements?register=untded' | jq '.count'\n  \
         curl -s '{}/data-elements/urn:untded:de:1000' | jq '.manifest'",
        parsed.base(),
        parsed.base()
    );
}
