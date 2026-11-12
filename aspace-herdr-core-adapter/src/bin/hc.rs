//! `hc` — the A'Space client CLI for herdr-core (D-N053 pas F1).
//!
//! Client only: every verb talks to an **already running, operator-side**
//! core over its socket (`client::require_existing` — never
//! `connect_or_start`, which would spawn one). There is no `server start`,
//! no `update`, no `serve`, no `hold` in this binary at all: clap rejects
//! them before any code runs. Output is JSON on stdout, always.
//!
//! Verbs mirror upstream `hc` (same names, same parameter types from
//! `herdr_core::api`, same RPC methods), minus the excluded ones:
//! `run ls get kill rm label keep send wait keys interrupt attach read put
//! exec`, `file read|write|edit`, `machine add|ls|login|update|rename|rm`,
//! `events`, `templates`, `server status|stop|restart`, `schema`, `doctor`,
//! `call`. `terminal <verb>` groups the terminal verbs under one prefix.

use aspace_herdr_core_adapter as adapter;
use adapter::api::{
    AttachParams, ExecParams, FileEditParams, FileReadParams, FileWriteParams, InterruptParams,
    KeepParams, KeysParams, LabelParams, ListParams, MachineAddParams, MachineParams,
    MachineRenameParams, PutParams, ReadParams, RunParams, SendParams, SubscribeParams,
    TerminalParams, WaitParams,
};
use adapter::paths::Paths;
use adapter::rpc::RpcError;
use clap::{Parser, Subcommand};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(name = "hc", version, about = "herdr-core client (A'Space adapter, D-N053 pas F1) — talks to an operator-side core, never starts one")]
struct Cli {
    /// Print the raw JSON result (output is JSON either way).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a program in a new terminal (waits unless --bg).
    Run(RunParams),
    /// List live terminals.
    Ls(ListParams),
    /// Show one terminal.
    Get(TerminalParams),
    /// End a terminal's program.
    Kill(TerminalParams),
    /// Remove a finished terminal.
    Rm(TerminalParams),
    /// Keep a live terminal after loss (--off disables).
    Keep(KeepParams),
    /// Set labels (key=value; key= removes).
    Label(LabelParams),
    /// Paste text into a terminal and press Enter.
    Send(SendParams),
    /// Wait for a terminal (or its children).
    Wait(WaitParams),
    /// Press named keys.
    Keys(KeysParams),
    /// Press the stop key and report what happened.
    Interrupt(InterruptParams),
    /// Attach to a terminal (viewer).
    Attach(AttachParams),
    /// Print a terminal's screen and history.
    Read(ReadParams),
    /// Send a local file beside a terminal.
    Put(PutParams),
    /// Run one program without a terminal; print what it wrote.
    Exec(ExecParams),
    /// Terminal verbs under one prefix (`hc terminal run …`).
    Terminal {
        #[command(subcommand)]
        action: TerminalCmd,
    },
    /// Read, write or edit files here or on a machine.
    File {
        #[command(subcommand)]
        action: FileCmd,
    },
    /// Other machines, reached over ssh.
    Machine {
        #[command(subcommand)]
        action: MachineCmd,
    },
    /// Follow fact changes as they happen.
    Events(SubscribeParams),
    /// Templates you can run by name.
    Templates,
    /// The core, as a client: status, stop, restart (never start).
    Server {
        #[command(subcommand)]
        action: ServerCmd,
    },
    /// Every API method with its JSON Schema, for app builders (local, no core needed).
    Schema,
    /// Show what this adapter resolves (socket, state, config, pins) — no core needed.
    Doctor,
    /// Call any RPC method directly: `hc call <method> --params '{…}'`.
    Call {
        /// RPC method, e.g. `terminal.list`.
        method: String,
        /// JSON params object (default `{}`).
        #[arg(long, value_name = "JSON", default_value = "{}")]
        params: String,
    },
}

#[derive(Subcommand)]
enum TerminalCmd {
    Run(RunParams),
    #[command(alias = "ls")]
    List(ListParams),
    Get(TerminalParams),
    Kill(TerminalParams),
    Rm(TerminalParams),
    Keep(KeepParams),
    Label(LabelParams),
    Send(SendParams),
    Wait(WaitParams),
    Keys(KeysParams),
    Interrupt(InterruptParams),
    Attach(AttachParams),
    Read(ReadParams),
    Put(PutParams),
    Exec(ExecParams),
}

#[derive(Subcommand)]
enum FileCmd {
    Read(FileReadParams),
    Write(FileWriteParams),
    Edit(FileEditParams),
}

#[derive(Subcommand)]
enum MachineCmd {
    Add(MachineAddParams),
    Ls,
    Login(MachineParams),
    Update(MachineParams),
    Rename(MachineRenameParams),
    Rm(MachineParams),
}

#[derive(Subcommand)]
enum ServerCmd {
    Status,
    Stop,
    Restart,
}

fn main() {
    let cli = Cli::parse();
    std::process::exit(match run(cli.cmd) {
        Ok(out) => {
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
            0
        }
        Err(err) => {
            let body = json!({"error": {"kind": err.kind, "message": err.message, "hint": err.hint}});
            eprintln!("{body}");
            err.exit_code()
        }
    });
}

/// One plain-mode call to the already-running core. Never starts one.
fn call(paths: &Paths, method: &str, params: Value) -> Result<Value, RpcError> {
    let stream = adapter::client::require_existing(paths)?;
    adapter::client::call(&stream, method, params)
}

fn here() -> Option<String> {
    std::env::current_dir().ok().map(|p| p.display().to_string())
}

fn from_here(cwd: Option<String>) -> Option<String> {
    match (cwd, here()) {
        (Some(cwd), Some(here)) => Some(
            std::path::Path::new(&here).join(cwd).display().to_string(),
        ),
        (cwd, here) => cwd.or(here),
    }
}

fn caller() -> String {
    adapter::client::terminal_id()
        .unwrap_or_else(|| format!("ppid:{}", adapter::platform::parent_pid()))
}

fn identity_env() -> std::collections::BTreeMap<String, String> {
    std::env::vars()
        .filter(|(k, _)| adapter::api::IDENTITY_VARS.contains(&k.as_str()))
        .collect()
}

fn require_terminal(given: &str) -> Result<String, RpcError> {
    if given.is_empty() {
        return Err(RpcError::new(
            "usage",
            "pass a terminal address like t7 or a name (no picker in the adapter CLI)",
            "hc terminal list",
        ));
    }
    Ok(given.to_string())
}

fn piped(max: usize) -> Result<Vec<u8>, RpcError> {
    use std::io::Read;
    let stdin = std::io::stdin();
    if std::io::IsTerminal::is_terminal(&stdin) {
        return Err(RpcError::new("usage", "nothing piped in", "pipe it in"));
    }
    let mut bytes = Vec::new();
    stdin
        .lock()
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| RpcError::internal(e.to_string()))?;
    if bytes.len() > max {
        return Err(RpcError::invalid_params(format!(
            "at most {} MB piped in",
            max >> 20
        )));
    }
    Ok(bytes)
}

fn to(params: &impl serde::Serialize) -> Value {
    serde_json::to_value(params).unwrap_or(Value::Null)
}

fn run(cmd: Cmd) -> Result<Value, RpcError> {
    let paths = Paths::from_env()?;
    match cmd {
        Cmd::Run(mut p) => {
            if p.on.is_none() {
                p.cwd = from_here(p.cwd);
            }
            p.env = identity_env();
            p.caller = Some(caller());
            p.wait = !p.bg;
            call(&paths, "terminal.run", to(&p))
        }
        Cmd::Ls(p) => call(&paths, "terminal.list", to(&p)),
        Cmd::Get(mut p) => {
            p.terminal = require_terminal(&p.terminal)?;
            p.caller = Some(caller());
            call(&paths, "terminal.get", to(&p))
        }
        Cmd::Kill(mut p) => {
            p.terminal = require_terminal(&p.terminal)?;
            p.caller = Some(caller());
            call(&paths, "terminal.kill", to(&p))
        }
        Cmd::Rm(mut p) => {
            p.terminal = require_terminal(&p.terminal)?;
            call(&paths, "terminal.rm", to(&p))
        }
        Cmd::Keep(p) => call(&paths, "terminal.keep", to(&p)),
        Cmd::Label(p) => call(&paths, "terminal.label", to(&p)),
        Cmd::Send(mut p) => {
            p.caller = Some(caller());
            call(&paths, "terminal.send", to(&p))
        }
        Cmd::Wait(mut p) => {
            if p.children.is_none() && p.terminal.is_none() {
                return Err(RpcError::new(
                    "usage",
                    "give a terminal, or --children any|all",
                    "hc wait <terminal>",
                ));
            }
            p.caller = Some(caller());
            call(&paths, "terminal.wait", to(&p))
        }
        Cmd::Keys(mut p) => {
            p.caller = Some(caller());
            call(&paths, "terminal.keys", to(&p))
        }
        Cmd::Interrupt(mut p) => {
            p.terminal = require_terminal(&p.terminal)?;
            call(&paths, "terminal.interrupt", to(&p))
        }
        Cmd::Attach(p) => {
            let terminal = require_terminal(&p.terminal)?;
            let message = adapter::attach::run(&paths, &terminal, p.observe, p.history)?;
            Ok(json!({"attached": terminal, "message": message}))
        }
        Cmd::Read(mut p) => {
            p.terminal = require_terminal(&p.terminal)?;
            p.caller = Some(caller());
            call(&paths, "terminal.read", to(&p))
        }
        Cmd::Put(mut p) => {
            let bytes = std::fs::read(&p.name)
                .map_err(|e| RpcError::new("usage", format!("cannot read {}: {e}", p.name), ""))?;
            p.data = adapter::put::encode(&bytes);
            call(&paths, "terminal.put", to(&p))
        }
        Cmd::Exec(mut p) => {
            if p.on.is_none() {
                p.cwd = from_here(p.cwd);
            }
            if p.stdin {
                p.input = Some(adapter::put::encode(&piped(adapter::exec::MAX_STDIN)?));
            }
            call(&paths, "machine.exec", to(&p))
        }
        Cmd::Terminal { action } => match action {
            TerminalCmd::Run(p) => run(Cmd::Run(p)),
            TerminalCmd::List(p) => run(Cmd::Ls(p)),
            TerminalCmd::Get(p) => run(Cmd::Get(p)),
            TerminalCmd::Kill(p) => run(Cmd::Kill(p)),
            TerminalCmd::Rm(p) => run(Cmd::Rm(p)),
            TerminalCmd::Keep(p) => run(Cmd::Keep(p)),
            TerminalCmd::Label(p) => run(Cmd::Label(p)),
            TerminalCmd::Send(p) => run(Cmd::Send(p)),
            TerminalCmd::Wait(p) => run(Cmd::Wait(p)),
            TerminalCmd::Keys(p) => run(Cmd::Keys(p)),
            TerminalCmd::Interrupt(p) => run(Cmd::Interrupt(p)),
            TerminalCmd::Attach(p) => run(Cmd::Attach(p)),
            TerminalCmd::Read(p) => run(Cmd::Read(p)),
            TerminalCmd::Put(p) => run(Cmd::Put(p)),
            TerminalCmd::Exec(p) => run(Cmd::Exec(p)),
        },
        Cmd::File { action } => match action {
            FileCmd::Read(mut p) => {
                if p.at.on.is_none() {
                    p.at.cwd = from_here(p.at.cwd.take());
                }
                call(&paths, "file.read", to(&p))
            }
            FileCmd::Write(mut p) => {
                if p.at.on.is_none() {
                    p.at.cwd = from_here(p.at.cwd.take());
                }
                p.data = adapter::put::encode(&piped(adapter::files::MAX_BYTES)?);
                call(&paths, "file.write", to(&p))
            }
            FileCmd::Edit(mut p) => {
                if p.at.on.is_none() {
                    p.at.cwd = from_here(p.at.cwd.take());
                }
                if p.old.len() != p.new.len() {
                    return Err(RpcError::new(
                        "usage",
                        "give a --new for each --old",
                        "--old X --new Y",
                    ));
                }
                p.edits = p
                    .old
                    .drain(..)
                    .zip(p.new.drain(..))
                    .map(|(old, new)| adapter::api::Edit { old, new })
                    .collect();
                call(&paths, "file.edit", to(&p))
            }
        },
        Cmd::Machine { action } => match action {
            MachineCmd::Add(p) => {
                // Log in first when a person can answer ssh's questions.
                let target = p.via.as_ref().or(p.target.as_ref()).cloned().unwrap_or_default();
                let reach = adapter::machines::Reach { target: &target, via: p.via.is_some() };
                if std::io::IsTerminal::is_terminal(&std::io::stdin())
                    && std::io::IsTerminal::is_terminal(&std::io::stderr())
                {
                    adapter::machines::login(&paths, reach)?;
                }
                call(&paths, "machine.add", to(&p))
            }
            MachineCmd::Ls => call(&paths, "machine.list", json!({})),
            MachineCmd::Login(p) => {
                let list = call(&paths, "machine.list", json!({}))?;
                let found = list["machines"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|m| m["name"] == p.machine.as_str())
                    .cloned();
                let Some(m) = found else {
                    return Err(RpcError::new(
                        "machine_unknown",
                        format!("unknown machine {:?}", p.machine),
                        "see `hc machine ls`",
                    ));
                };
                let reach = adapter::machines::Reach {
                    target: m["target"].as_str().unwrap_or_default(),
                    via: m["via"] == true,
                };
                adapter::machines::login(&paths, reach)?;
                call(&paths, "machine.connect", to(&p))
            }
            MachineCmd::Update(p) => {
                // Copies this adapter `hc` onto an already-added remote
                // machine and restarts `hc` there. Operator-side machines
                // only; nothing is installed or started on this host.
                let list = call(&paths, "machine.list", json!({}))?;
                let found = list["machines"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|m| m["name"] == p.machine.as_str())
                    .cloned();
                let Some(m) = found else {
                    return Err(RpcError::new(
                        "machine_unknown",
                        format!("unknown machine {:?}", p.machine),
                        "see `hc machine ls`",
                    ));
                };
                let reach = adapter::machines::Reach {
                    target: m["target"].as_str().unwrap_or_default(),
                    via: m["via"] == true,
                };
                let version =
                    adapter::machines::install(&paths, reach, m["session"].as_str())?;
                Ok(json!({"machine": p.machine, "version": version}))
            }
            MachineCmd::Rename(p) => call(&paths, "machine.rename", to(&p)),
            MachineCmd::Rm(p) => call(&paths, "machine.rm", to(&p)),
        },
        Cmd::Events(p) => {
            // One snapshot, then every notification until the core goes away
            // (upstream `follow_events`, but against the existing core only).
            let stream = adapter::client::require_existing(&paths)?;
            let snapshot = adapter::client::call(&stream, "events.subscribe", to(&p))?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap_or_default());
            use std::io::BufRead;
            for line in std::io::BufReader::new(&stream).lines() {
                let Ok(line) = line else { break };
                println!("{line}");
            }
            Ok(snapshot)
        }
        Cmd::Templates => call(&paths, "templates.list", json!({})),
        Cmd::Server { action } => match action {
            ServerCmd::Status => {
                let stream = adapter::client::require_existing(&paths)?;
                adapter::client::call(&stream, "server.status", json!({}))
            }
            ServerCmd::Stop => {
                let Some(stream) = adapter::client::connect_existing(&paths) else {
                    return Ok(json!({"stopped": false}));
                };
                adapter::client::call(&stream, "server.stop", json!({}))
            }
            ServerCmd::Restart => {
                let stream = adapter::client::require_existing(&paths)?;
                adapter::client::call(&stream, "server.restart", json!({}))
            }
        },
        Cmd::Schema => Ok(adapter::api::openrpc()),
        Cmd::Doctor => Ok(json!({
            "fiber": adapter::FIBER_ADDRESS,
            "adapter_version": adapter::ADAPTER_VERSION,
            "gel": adapter::PINNED_UPSTREAM_COMMIT,
            "bind": adapter::BIND_COMMIT,
            "socket": paths.socket,
            "state": paths.state,
            "config": paths.config,
            "notes_forced": adapter::forced_notes(),
            "daemon": "excluded: server::serve() and `hc server start` are not in this binary",
        })),
        Cmd::Call { method, params } => {
            let params: Value = serde_json::from_str(&params).map_err(|e| {
                RpcError::invalid_params(format!("--params is not JSON: {e}"))
            })?;
            call(&paths, &method, params)
        }
    }
}
