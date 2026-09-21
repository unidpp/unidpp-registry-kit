//! The seeding suite, as Rust subcommands of this binary: the port of
//! the four Python seeders (retired with the family's script-free
//! pass), command for command and output line for output line. The
//! signing half — the deterministic dev operator key and the signed
//! C3/profile forms — lives here; the canonical bytes are serde_json's
//! own (sorted keys, compact separators), which is the form the
//! registry's keyring verifies.

pub mod jurisdiction;
pub mod mappings;
pub mod units;
pub mod untded;

use ed25519_dalek::Signer;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;

use crate::die;
use crate::http;

pub const OPERATOR_SEED_DOMAIN: &[u8] = b"UNIDPP-DISCOVERY/OPERATOR-SEED";
pub const DEFAULT_OPERATOR_LABEL: &str = "unidpp-registry";
pub const DEFAULT_PROTOCOL_BINDING: &str = "pb-en18222-rest";

/// The registry connection the seeders drive: the kit's configured
/// bind/port, the admin token from the flag, the environment, or the
/// kit's own token file.
pub struct Registry {
    pub bind: String,
    pub port: u16,
    pub token: String,
}

impl Registry {
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.bind, self.port)
    }

    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> (u16, Value) {
        let body_text = body.map(|b| b.to_string());
        let response = http::request(
            method,
            &self.bind,
            self.port,
            path,
            body_text.as_deref(),
            if self.token.is_empty() {
                None
            } else {
                Some(&self.token)
            },
            Duration::from_secs(15),
        )
        .unwrap_or_else(|| die(&format!("cannot reach the registry at {}", self.base_url())));
        let parsed = serde_json::from_str::<Value>(&response.body)
            .unwrap_or_else(|_| json!({ "body": response.body }));
        (response.status, parsed)
    }

    pub fn get(&self, path: &str) -> (u16, Value) {
        self.request("GET", path, None)
    }

    pub fn post(&self, path: &str, body: &Value) -> (u16, Value) {
        self.request("POST", path, Some(body))
    }

    pub fn require_health(&self) {
        let (status, _) = self.get("/healthz");
        if status != 200 {
            die(&format!(
                "no healthy registry at {} — run unidpp-kit start --daemon first",
                self.base_url()
            ));
        }
    }
}

/// The options every seeder shares (`--port`, `--bind`, `--base-url`,
/// `--admin-token`, `--token-file`), resolved against the kit's own
/// configuration. `bind`/`base_url` fall back to the kit's model, not
/// to a private default.
pub struct SeedArgs {
    pub bind: String,
    pub port: u16,
    pub base_url: Option<String>,
    pub admin_token: Option<String>,
    pub token_file: Option<std::path::PathBuf>,
}

pub fn parse_seed_args(kit: &crate::Kit, args: &[String], usage: &str) -> SeedArgs {
    let mut parsed = SeedArgs {
        bind: kit.bind.clone(),
        port: kit.port,
        base_url: None,
        admin_token: None,
        token_file: None,
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--port" => {
                let value = rest.next().unwrap_or_else(|| die(usage));
                parsed.port = value
                    .parse()
                    .unwrap_or_else(|_| die(&format!("--port must be a number (got {value})")));
            }
            "--bind" => parsed.bind = rest.next().unwrap_or_else(|| die(usage)).clone(),
            "--base-url" => {
                parsed.base_url = Some(rest.next().unwrap_or_else(|| die(usage)).clone())
            }
            "--admin-token" => {
                parsed.admin_token = Some(rest.next().unwrap_or_else(|| die(usage)).clone())
            }
            "--token-file" => {
                parsed.token_file = Some(std::path::PathBuf::from(
                    rest.next().unwrap_or_else(|| die(usage)),
                ))
            }
            other => die(&format!("unknown option: {other}\n{usage}")),
        }
    }
    parsed
}

impl SeedArgs {
    pub fn registry(&self, kit: &crate::Kit) -> Registry {
        let token = self
            .admin_token
            .clone()
            .or_else(|| {
                let file = self.token_file.clone().unwrap_or_else(|| kit.token_file());
                std::fs::read_to_string(file)
                    .ok()
                    .map(|t| t.trim().to_string())
            })
            .unwrap_or_default();
        Registry {
            bind: self.bind.clone(),
            port: self.port,
            token,
        }
    }

    pub fn base(&self) -> String {
        self.base_url
            .clone()
            .unwrap_or_else(|| format!("http://{}:{}", self.bind, self.port))
    }
}

pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Percent-encode everything outside the unreserved set (the
/// `quote(s, safe="")` the wire paths rely on).
pub fn quote(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// A deterministic dev operator key: the unidpp-registry keyring
/// scheme. In production the jurisdiction's operator credential comes
/// from the trust service (ONBOARDING.md step 1); the keyring seam is
/// where it plugs in.
pub struct Operator {
    signing: ed25519_dalek::SigningKey,
    public_key: [u8; 32],
}

impl Operator {
    pub fn new(label: &str) -> Operator {
        let digest: [u8; 32] =
            Sha256::digest([OPERATOR_SEED_DOMAIN, label.as_bytes()].concat().as_slice()).into();
        let signing = ed25519_dalek::SigningKey::from_bytes(&digest);
        let public_key = signing.verifying_key().to_bytes();
        Operator {
            signing,
            public_key,
        }
    }

    pub fn id(&self) -> String {
        format!(
            "op-{}",
            &hex(&Sha256::digest(
                [b"op", self.public_key.as_slice()].concat().as_slice(),
            ))[..16]
        )
    }

    pub fn key_id(&self) -> String {
        format!(
            "k-{}",
            &hex(&Sha256::digest(
                [b"key", self.public_key.as_slice()].concat().as_slice(),
            ))[..16]
        )
    }

    pub fn record(&self) -> Value {
        json!({
            "id": self.id(),
            "key_id": self.key_id(),
            "public_key": hex(&self.public_key),
            "algorithm": "ed25519",
        })
    }

    /// Sign a descriptor body; returns the wire object with the
    /// signature block embedded.
    pub fn sign(&self, body: &Value) -> Value {
        let mut signed = body.clone();
        let signature = json!({
            "key_id": self.key_id(),
            "algorithm": "ed25519",
            "value": hex(&self.signing.sign(&canonical(body)).to_bytes()),
        });
        signed
            .as_object_mut()
            .expect("descriptor body is an object")
            .insert("signature".into(), signature.clone());
        signed
    }

    /// Complete a profile manifest in its SIGNED form (PR-1): the
    /// issuer class grades what the issuer may claim — a national
    /// registry authority's jurisdictional profile is law — the
    /// issuer names the signer of record, and the signature covers
    /// the manifest's bytes (its signature slot excluded).
    pub fn sign_manifest(&self, manifest: &Value) -> Value {
        let mut signed = manifest.clone();
        {
            let object = signed.as_object_mut().expect("manifest is an object");
            object.insert("issuer_class".into(), json!("law"));
            object.insert("issuer".into(), json!(self.key_id()));
        }
        let signature = json!({
            "suite": "ed25519",
            "key_id": self.key_id(),
            "signature": hex(&self.signing.sign(&canonical(&signed)).to_bytes()),
        });
        signed
            .as_object_mut()
            .expect("manifest is an object")
            .insert("signature".into(), signature);
        signed
    }
}

/// serde_json's own byte form: sorted keys, compact separators — the
/// exact bytes the registry's keyring verifies.
pub fn canonical(value: &Value) -> Vec<u8> {
    serde_json::to_string(value)
        .expect("serialization of a JSON value cannot fail")
        .into_bytes()
}
