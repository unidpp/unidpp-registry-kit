#![allow(clippy::zombie_processes)] // daemons by design: see the module doc
//! The registry-kit launcher, as a program (TODO 246: the durability
//! contract is a program). This binary is the successor of the kit's
//! two operator scripts: `bin/run-registry.sh` (build and run the
//! unidpp-registry sibling service as a federation peer) and
//! `bin/demo-jurisdiction.sh` (the jurisdiction demonstration, ported
//! in [`demo`]). The scripts stay in place until the retirement
//! change; this program reproduces their subcommands, their
//! environment knobs and their files under `data/` exactly.
//!
//! The registry, the tunnel and the foreground seeding helper are
//! daemons by design: the launcher spawns them detached and exits, so
//! the never-waited spawns below are deliberate (not zombies) — the
//! same discipline as `unidpp-pilot-data/ops`.
//!
//! The journal is append-only and is never removed by this program:
//! stopping and restarting replays it (durability and auditability are
//! the same mechanism — see the unidpp-registry README, "Storage
//! choice").
//!
//! Usage:
//!   unidpp-kit                          # run in the foreground (Ctrl-C stops)
//!   unidpp-kit start --daemon           # run in the background (data/registry.pid)
//!   unidpp-kit start --tunnel           # daemonized + cloudflared quick tunnel
//!   unidpp-kit stop                     # stop a --daemon instance
//!   unidpp-kit status                   # healthz + journal + service count
//!   unidpp-kit seed                     # wait for health, then seed once
//!   unidpp-kit demo-jurisdiction [--jurisdiction DE]
//!
//! Configuration (environment):
//!   KIT_PORT            listen port            (default 8391; the UniDPP
//!                       pilot registry lives on 8390 — pick your own)
//!   KIT_BIND            listen address         (default 127.0.0.1)
//!   KIT_HOME            runtime state dir      (default <kit>/data)
//!   KIT_REGISTRY_DIR    unidpp-registry source (default <kit>/../unidpp-registry)
//!   KIT_FORCE_BUILD     1 = rebuild even if the binary exists
//!   KIT_ADMIN_TOKEN     admin Bearer token     (default: generated once,
//!                       persisted mode 600 in $KIT_HOME/admin-token)

mod demo;
mod http;

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

pub struct Kit {
    /// The kit checkout (the directory that holds `bin/` and `data/`).
    root: PathBuf,
    /// The runtime state directory (`KIT_HOME`, default `<kit>/data`).
    home: PathBuf,
    /// The unidpp-registry source checkout (`KIT_REGISTRY_DIR`).
    registry_dir: PathBuf,
    /// The listen port (`KIT_PORT`, default 8391).
    port: u16,
    /// The listen address (`KIT_BIND`, default 127.0.0.1).
    bind: String,
}

impl Kit {
    fn from_env() -> Kit {
        let root = kit_root();
        let env_or = |name: &str, default: String| {
            std::env::var(name)
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or(default)
        };
        let port = env_or("KIT_PORT", "8391".into());
        let port = port
            .parse::<u16>()
            .unwrap_or_else(|_| die(&format!("KIT_PORT must be a port number (got `{port}`)")));
        Kit {
            home: PathBuf::from(env_or(
                "KIT_HOME",
                root.join("data").to_string_lossy().into_owned(),
            )),
            registry_dir: PathBuf::from(env_or(
                "KIT_REGISTRY_DIR",
                root.join("..")
                    .join("unidpp-registry")
                    .to_string_lossy()
                    .into_owned(),
            )),
            port,
            bind: env_or("KIT_BIND", "127.0.0.1".into()),
            root,
        }
    }

    fn bin(&self) -> PathBuf {
        self.registry_dir.join("target/release/unidpp-registry")
    }
    pub(crate) fn journal(&self) -> PathBuf {
        self.home.join("jurisdiction-journal.jsonl")
    }
    fn log(&self) -> PathBuf {
        self.home.join("registry.log")
    }
    fn pid_file(&self) -> PathBuf {
        self.home.join("registry.pid")
    }
    pub(crate) fn token_file(&self) -> PathBuf {
        self.home.join("admin-token")
    }
    pub(crate) fn url(&self) -> String {
        format!("http://{}:{}", self.bind, self.port)
    }
    pub(crate) fn home(&self) -> &Path {
        &self.home
    }
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn port(&self) -> u16 {
        self.port
    }
}

/// The kit root, discovered the way the pilot discovers its directory:
/// the binary lives at `<kit>/target/(debug|release)/`, so the kit is
/// two levels up from the executable; a checkout that does not hold
/// `bin/run-registry.sh` is not the kit, and the working directory
/// stands in for an installed binary.
fn kit_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .and_then(|dir| dir.ancestors().nth(2).map(Path::to_path_buf))
        .filter(|p| p.join("bin").join("run-registry.sh").is_file())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

pub(crate) fn die(msg: &str) -> ! {
    eprintln!("unidpp-kit: {msg}");
    std::process::exit(1);
}

fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn terminate(pid: &str) -> bool {
    Command::new("kill")
        .arg(pid)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// `curl -sf -m 2 $URL/healthz`: success means the request answered
/// below 400.
fn healthz(kit: &Kit) -> bool {
    http::get(&kit.bind, kit.port, "/healthz", Duration::from_secs(2))
        .is_some_and(|r| (200..400).contains(&r.status))
}

fn wait_healthy(kit: &Kit) {
    // The shell polled 50 × 0.2 s (10 s); that budget no longer holds.
    // A first start of the sibling registry deposits the vendored
    // EXPRESS core model before it binds, which runs `expressir
    // validate` and measures around 30 s on this machine — the shell
    // script fails today the same way this port did before the budget
    // widened. The pilot's ops crate budgets 60 s (120 × 0.5 s) for
    // this same service, and this port follows the family number.
    for _ in 0..120 {
        if healthz(kit) {
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    die(&format!(
        "registry did not become healthy on {} (see {})",
        kit.url(),
        kit.log().display()
    ));
}

fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn journal_records(kit: &Kit) -> usize {
    std::fs::read(kit.journal())
        .map(|bytes| bytes.iter().filter(|b| **b == b'\n').count())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// The admin token (generated once, mode 600, reused across restarts)
// ---------------------------------------------------------------------------

fn ensure_admin_token(kit: &Kit) -> String {
    // Always guard admin mutations: a jurisdictional registry is a peer
    // that other peers audit, not an open dev service. The token is
    // generated once and reused so journal history stays attributable
    // to one operator credential across restarts.
    if let Ok(token) = std::env::var("KIT_ADMIN_TOKEN") {
        if !token.is_empty() {
            return token;
        }
    }
    let token_file = kit.token_file();
    let existing = std::fs::metadata(&token_file)
        .map(|m| m.len() > 0)
        .unwrap_or(false);
    if !existing {
        // The shell generated `openssl rand -hex 24`; this port draws
        // the same 24 bytes from /dev/urandom and hex-encodes them, so
        // the file holds the identical 48-character form.
        let mut bytes = [0u8; 24];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .unwrap_or_else(|e| die(&format!("cannot read /dev/urandom: {e}")));
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&token_file)
            .and_then(|mut f| f.write_all(format!("{token}\n").as_bytes()))
            .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", token_file.display())));
    }
    let token = std::fs::read_to_string(&token_file)
        .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", token_file.display())))
        .trim()
        .to_string();
    if token.is_empty() {
        die(&format!(
            "cannot create admin token at {}",
            token_file.display()
        ));
    }
    // The shell ran `chmod 600 … || true`; a best-effort tightening of
    // an existing file's mode follows the same tolerance.
    if let Ok(meta) = std::fs::metadata(&token_file) {
        let mut perms = meta.permissions();
        perms.set_mode(0o600);
        let _ = std::fs::set_permissions(&token_file, perms);
    }
    token
}

// ---------------------------------------------------------------------------
// The sibling binary (build on demand)
// ---------------------------------------------------------------------------

fn ensure_binary(kit: &Kit) {
    if is_executable(&kit.bin()) && std::env::var("KIT_FORCE_BUILD").as_deref() != Ok("1") {
        return;
    }
    let cargo = Command::new("cargo")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !cargo {
        die(&format!(
            "cargo not found and no prebuilt binary at {}",
            kit.bin().display()
        ));
    }
    println!(
        "==> building unidpp-registry (release) in {}",
        kit.registry_dir.display()
    );
    let built = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&kit.registry_dir)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !built || !is_executable(&kit.bin()) {
        die(&format!(
            "build finished but {} is missing",
            kit.bin().display()
        ));
    }
}

// ---------------------------------------------------------------------------
// Seeding (the base discovery dataset, through the API)
// ---------------------------------------------------------------------------

fn seed_base_dataset(kit: &Kit, token: &str) {
    // The base dataset (UniDPP's own C3/C4/C5 items + C1 units) is what
    // a jurisdictional descriptor references: protocol bindings and
    // units. Idempotent per process — but on a replayed journal the
    // records already exist and re-running would conflict, so check
    // first: units only enter the store through this seed.
    let units = http::get(&kit.bind, kit.port, "/units", Duration::from_secs(5))
        .and_then(|r| serde_json::from_str::<serde_json::Value>(&r.body).ok())
        .and_then(|doc| doc.get("items").and_then(|i| i.as_array()).map(|a| a.len()))
        .unwrap_or(0);
    if units != 0 {
        println!("==> base discovery dataset already present (journal replay; {units} units)");
        return;
    }
    println!("==> seeding base discovery dataset via the API");
    let resp = http::post_auth(
        &kit.bind,
        kit.port,
        "/admin/seed",
        "{}",
        token,
        Duration::from_secs(15),
    );
    let body = resp.map(|r| r.body).unwrap_or_default();
    let shown: String = body.chars().take(400).collect();
    println!("{shown}");
}

/// The `seed` subcommand: wait until the peer is healthy, then run the
/// base-dataset seeding once. The foreground start path re-invokes the
/// binary as this subcommand in a detached child, which reproduces the
/// shell's `( seed … >/dev/null 2>&1 || true ) &` subshell.
fn cmd_seed(kit: &Kit) {
    let token = ensure_admin_token(kit);
    wait_healthy(kit);
    seed_base_dataset(kit, &token);
}

// ---------------------------------------------------------------------------
// Start / stop / status
// ---------------------------------------------------------------------------

/// The shared core of every start path: adopt a healthy listener, or
/// spawn the daemon, and then seed. `bin/run-registry.sh` named this
/// `start_daemon`.
pub(crate) fn start_daemon(kit: &Kit) {
    ensure_binary(kit);
    let token = ensure_admin_token(kit);
    if healthz(kit) {
        println!(
            "==> a registry is already listening on {} (reusing it)",
            kit.url()
        );
    } else {
        println!("==> starting unidpp-registry (daemon) on {}", kit.url());
        println!("    journal: {}", kit.journal().display());
        println!("    log:     {}", kit.log().display());
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(kit.log())
            .unwrap_or_else(|e| die(&format!("log file: {e}")));
        // `nohup env … "$BIN" &` in the shell: the child runs detached
        // in its own process group (so the pid the launcher records is
        // the server's own pid, and the server survives the launcher).
        let mut command = Command::new(kit.bin());
        command
            .env("UNIDPP_REGISTRY_BIND", format!("{}:{}", kit.bind, kit.port))
            .env("UNIDPP_REGISTRY_STATE_FILE", kit.journal())
            .env("UNIDPP_REGISTRY_ADMIN_TOKEN", &token)
            .stdout(
                log.try_clone()
                    .unwrap_or_else(|e| die(&format!("log handle: {e}"))),
            )
            .stderr(log)
            .process_group(0);
        let child = command
            .spawn()
            .unwrap_or_else(|e| die(&format!("unidpp-registry failed to start: {e}")));
        std::fs::write(kit.pid_file(), child.id().to_string())
            .unwrap_or_else(|e| die(&format!("pid file: {e}")));
        wait_healthy(kit);
    }
    seed_base_dataset(kit, &token);
    println!(
        "==> ready. Admin token: {} — e.g.:",
        kit.token_file().display()
    );
    println!(
        "      curl -H \"Authorization: Bearer $(cat {})\" '{}/items'",
        kit.token_file().display(),
        kit.url()
    );
}

fn start_foreground(kit: &Kit) -> ! {
    ensure_binary(kit);
    let token = ensure_admin_token(kit);
    if healthz(kit) {
        println!(
            "==> a registry is already listening on {} (reusing it)",
            kit.url()
        );
        seed_base_dataset(kit, &token);
        std::process::exit(0);
    }
    // Seed over the API just after the server comes up, then hand the
    // terminal to the server process. The shell ran the seeding loop in
    // a `( … ) &` subshell; this port re-invokes the binary itself as a
    // detached `seed` child with its output discarded, because a thread
    // would not survive the exec below.
    Command::new(std::env::current_exe().unwrap_or_else(|_| PathBuf::from("unidpp-kit")))
        .arg("seed")
        .env("KIT_HOME", kit.home())
        .env("KIT_PORT", kit.port.to_string())
        .env("KIT_BIND", &kit.bind)
        .env("KIT_ADMIN_TOKEN", &token)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap_or_else(|e| die(&format!("seeding helper failed to start: {e}")));
    println!(
        "==> starting unidpp-registry on {} (foreground; Ctrl-C stops)",
        kit.url()
    );
    println!("    journal: {}", kit.journal().display());
    // exec: signals flow straight through to the server.
    let err = Command::new(kit.bin())
        .env("UNIDPP_REGISTRY_BIND", format!("{}:{}", kit.bind, kit.port))
        .env("UNIDPP_REGISTRY_STATE_FILE", kit.journal())
        .env("UNIDPP_REGISTRY_ADMIN_TOKEN", &token)
        .current_dir(kit.home())
        .exec();
    die(&format!("unidpp-registry failed to start: {err}"));
}

fn start_tunnel(kit: &Kit) {
    let installed = Command::new("cloudflared")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !installed {
        die("--tunnel requested but cloudflared is not installed \
             (https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)");
    }
    let tunnel_log = kit.home.join("tunnel.log");
    let tunnel_pid = kit.home.join("tunnel.pid");
    println!(
        "==> starting cloudflared quick tunnel -> {} (log: {})",
        kit.url(),
        tunnel_log.display()
    );
    // The shell redirected with `>` (truncate); the registry log appends.
    let log = File::create(&tunnel_log).unwrap_or_else(|e| die(&format!("tunnel log: {e}")));
    let child = Command::new("cloudflared")
        .args(["tunnel", "--url", &kit.url()])
        .stdout(
            log.try_clone()
                .unwrap_or_else(|e| die(&format!("tunnel log handle: {e}"))),
        )
        .stderr(log)
        .process_group(0)
        .spawn()
        .unwrap_or_else(|e| die(&format!("cloudflared failed to start: {e}")));
    std::fs::write(&tunnel_pid, child.id().to_string())
        .unwrap_or_else(|e| die(&format!("tunnel pid file: {e}")));
    let mut public = String::new();
    for _ in 0..40 {
        // The assigned hostname is a hyphenated subdomain; exclude
        // api.trycloudflare.com (cloudflared's own endpoint, which also
        // appears in the log, including in error messages).
        if let Ok(text) = std::fs::read_to_string(&tunnel_log) {
            if let Some(url) = quick_tunnel_url(&text) {
                if alive(child.id()) {
                    public = url;
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    if !public.is_empty() {
        println!("    public URL: {public}");
        let _ = std::fs::write(kit.home.join("PUBLIC_URL"), &public);
    } else {
        println!("    (no URL captured yet — check {})", tunnel_log.display());
    }
    print!(
        "    NOTE: a quick tunnel is ephemeral (the URL changes on restart) and is\n    \
         for demos only. A jurisdictional registry publishes a stable hostname\n    \
         through a named tunnel or its own ingress, e.g.:\n\n      \
         cloudflared tunnel create jurisdiction-de\n      \
         cloudflared tunnel route dns jurisdiction-de registry.example.org\n      \
         cloudflared tunnel run --token <token> --url http://127.0.0.1:8391\n\n    \
         Nothing in the kit or the federation protocol requires a tunnel:\n    \
         peers verify signatures and as-of semantics, not hosting location.\n"
    );
    io::stdout().flush().ok();
}

/// `grep -o 'https://[a-z0-9][a-z0-9-]*\.trycloudflare\.com'` with the
/// api host excluded, without a regex dependency: scan for the scheme,
/// take the `[a-z0-9-]` run that follows, and accept it only when the
/// first character is alphanumeric and the suffix matches.
fn quick_tunnel_url(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(found) = text[from..].find("https://") {
        let start = from + found + "https://".len();
        let mut end = start;
        while end < bytes.len()
            && (bytes[end].is_ascii_lowercase()
                || bytes[end].is_ascii_digit()
                || bytes[end] == b'-')
        {
            end += 1;
        }
        let host = &text[start..end];
        let first = bytes.get(start).copied().unwrap_or(0);
        if !host.is_empty()
            && (first.is_ascii_lowercase() || first.is_ascii_digit())
            && host != "api"
            && text[end..].starts_with(".trycloudflare.com")
        {
            return Some(format!("https://{host}.trycloudflare.com"));
        }
        from = start;
    }
    None
}

/// `tail -f`: the last ten lines, then every appended byte, until the
/// operator interrupts (Ctrl-C detaches; the daemon keeps running).
fn follow_log(path: &Path) {
    let Ok(mut file) = File::open(path) else {
        return;
    };
    let mut text = String::new();
    file.read_to_string(&mut text).ok();
    let lines: Vec<&str> = text.lines().collect();
    let tail = lines.len().saturating_sub(10);
    for line in &lines[tail..] {
        println!("{line}");
    }
    let mut position = text.len() as u64;
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        file.seek(SeekFrom::Start(position)).ok();
        if file.read_to_end(&mut buffer).unwrap_or(0) > 0 {
            position += buffer.len() as u64;
            io::stdout().write_all(&buffer).ok();
            io::stdout().flush().ok();
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn cmd_stop(kit: &Kit) -> ! {
    let pid_file = kit.pid_file();
    if pid_file.is_file() {
        let pid = std::fs::read_to_string(&pid_file).unwrap_or_default();
        let pid = pid.trim();
        if !pid.is_empty() && terminate(pid) {
            println!(
                "==> stopped registry (pid {pid}); journal preserved at {}",
                kit.journal().display()
            );
        } else {
            println!("==> pid {pid} not running (stale pid file?)");
        }
        let _ = std::fs::remove_file(&pid_file);
    } else {
        println!(
            "==> no pid file at {} (foreground instances stop with Ctrl-C)",
            pid_file.display()
        );
    }
    let tunnel_pid = kit.home.join("tunnel.pid");
    if tunnel_pid.is_file() {
        if let Ok(pid) = std::fs::read_to_string(&tunnel_pid) {
            if terminate(pid.trim()) {
                let _ = std::fs::remove_file(&tunnel_pid);
                println!("==> stopped cloudflared tunnel");
            }
        }
    }
    std::process::exit(0);
}

fn cmd_status(kit: &Kit) -> ! {
    if healthz(kit) {
        println!("healthy:     {}/healthz", kit.url());
        println!(
            "journal:     {} ({} records)",
            kit.journal().display(),
            journal_records(kit)
        );
        println!("admin token: {}", kit.token_file().display());
        let token = std::fs::read_to_string(kit.token_file())
            .map(|t| t.trim().to_string())
            .unwrap_or_default();
        if let Some(resp) = http::get_auth(
            &kit.bind,
            kit.port,
            "/services",
            &token,
            Duration::from_secs(5),
        ) {
            if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&resp.body) {
                if let Some(count) = doc.get("count").and_then(|c| c.as_u64()) {
                    println!("services registered: {count}");
                }
            }
        }
        std::process::exit(0);
    }
    println!("not running on {}", kit.url());
    std::process::exit(1);
}

fn cmd_start(kit: &Kit, daemon: bool, tunnel: bool) {
    if daemon || tunnel {
        start_daemon(kit);
        if tunnel {
            start_tunnel(kit);
            println!("==> streaming registry log (Ctrl-C detaches; server keeps running)");
            follow_log(&kit.log());
        }
    } else {
        start_foreground(kit);
    }
}

fn usage() -> ! {
    die(
        "usage: unidpp-kit [start [--daemon] [--tunnel]] | stop | status | seed \
         | demo-jurisdiction [--jurisdiction DE]",
    )
}

fn main() {
    let kit = Kit::from_env();
    std::fs::create_dir_all(&kit.home)
        .unwrap_or_else(|e| die(&format!("cannot create {}: {e}", kit.home.display())));
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => cmd_start(&kit, false, false),
        Some("start") => {
            let mut daemon = false;
            let mut tunnel = false;
            for arg in &args[1..] {
                match arg.as_str() {
                    "--daemon" => daemon = true,
                    "--tunnel" => tunnel = true,
                    other => die(&format!(
                        "unknown option: {other} (expected --daemon, --tunnel)"
                    )),
                }
            }
            cmd_start(&kit, daemon, tunnel);
        }
        Some("stop") => cmd_stop(&kit),
        Some("status") => cmd_status(&kit),
        Some("seed") => cmd_seed(&kit),
        Some("demo-jurisdiction") | Some("demo") => {
            let mut jurisdiction = "DE".to_string();
            let mut rest = args[1..].iter();
            while let Some(arg) = rest.next() {
                match arg.as_str() {
                    "--jurisdiction" => {
                        jurisdiction = rest.next().cloned().unwrap_or_else(|| usage());
                    }
                    other => die(&format!(
                        "unknown option: {other} (expected --jurisdiction)"
                    )),
                }
            }
            demo::run(&kit, &jurisdiction);
        }
        Some(_) => usage(),
    }
}
