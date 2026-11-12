//! Protocol proof for the herdr-core adapter (D-N053 pas F1, H-F1-3).
//!
//! Uses the adapter library itself (`herdr_core::wire` + `herdr_core::rpc` +
//! `herdr_core::config`) against a local stub that speaks the framed `hc`
//! protocol — never against a real `hc` server, never against
//! `releases.herdr.dev`.
//!
//! What is proven:
//! 1. The wire constants (`HEADER` 9, `MAX_PAYLOAD` 16 MiB, `WINDOW` 256 KiB)
//!    and a `wire::encode` / `wire::decode_header` round-trip.
//! 2. The adapter config is read by the library and `notes == Some(false)`
//!    (forced switch, D-F1-5).
//! 3. `client::require_existing` fails cleanly with no server and spawns
//!    nothing (the adapter never uses `connect_or_start`).
//! 4. A full handshake: plain-mode `hello` with `framed: true`, then a
//!    framed JSON-RPC `server.status` call on channel 0, answered by the stub.
//!
//! Usage: `cargo run --offline -- <adapter-config.toml> <scratch-dir>`

use herdr_core::{config, rpc, wire};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

fn fail(msg: String) -> ! {
    eprintln!("protocol-proof FAIL: {msg}");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        fail("usage: protocol-proof <adapter-config.toml> <scratch-dir>".into());
    }
    let config_path = PathBuf::from(&args[1]);
    let scratch = PathBuf::from(&args[2]);
    // The listener must be created here (not in the stub thread) so the
    // client can connect deterministically; the stub thread accepts on it.
    let sock_path = scratch.join("stub.sock");
    let listener = match UnixListener::bind(&sock_path) {
        Ok(l) => l,
        Err(e) => fail(format!("bind stub socket: {e}")),
    };
    listener.set_nonblocking(false).ok();
    let handle = std::thread::spawn(move || accept_stub(listener));
    // Re-run the checks that do not need the socket first.
    if let Err(e) = run_checks_only(&config_path, &scratch) {
        fail(e);
    }
    // Client side of the round-trip.
    let mut stream = match UnixStream::connect(&sock_path) {
        Ok(s) => s,
        Err(e) => fail(format!("connect stub: {e}")),
    };
    stream.set_read_timeout(Some(TIMEOUT)).ok();
    stream.set_write_timeout(Some(TIMEOUT)).ok();
    let mut reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(e) => fail(e.to_string()),
    };
    let hello = json!({"jsonrpc": "2.0", "id": 1, "method": "hello",
                       "params": {"generation": 1, "framed": true}});
    if let Err(e) = stream.write_all(format!("{hello}\n").as_bytes()) {
        fail(format!("write hello: {e}"));
    }
    let mut line = String::new();
    if let Err(e) = reader.read_line(&mut line) {
        fail(format!("read hello reply: {e}"));
    }
    let reply: Value = match serde_json::from_str(&line) {
        Ok(v) => v,
        Err(e) => fail(format!("parse hello reply: {e}")),
    };
    if reply["result"]["framed"] != json!(true) {
        fail(format!("hello reply did not confirm framed mode: {reply}"));
    }
    let req = json!({"jsonrpc": "2.0", "id": 2, "method": "server.status", "params": {}});
    if let Err(e) = stream.write_all(&wire::encode(0, wire::DATA, req.to_string().as_bytes())) {
        fail(format!("write framed request: {e}"));
    }
    let mut head = [0u8; wire::HEADER];
    if let Err(e) = reader.read_exact(&mut head) {
        fail(format!("read frame header: {e}"));
    }
    let (channel, kind, len) = match wire::decode_header(&head) {
        Ok(t) => t,
        Err(e) => fail(format!("decode reply header: {e}")),
    };
    if (channel, kind) != (0, wire::DATA) {
        fail(format!("reply frame on ({channel}, {kind}), expected (0, DATA)"));
    }
    let mut payload = vec![0u8; len];
    if let Err(e) = reader.read_exact(&mut payload) {
        fail(format!("read frame payload: {e}"));
    }
    let framed: Value = match serde_json::from_slice(&payload) {
        Ok(v) => v,
        Err(e) => fail(format!("parse framed reply: {e}")),
    };
    if framed["result"]["version"] != json!("0.1.18-stub") {
        fail(format!("framed server.status reply unexpected: {framed}"));
    }
    drop(reader);
    drop(stream);
    match handle.join() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => fail(e),
        Err(_) => fail("stub thread panicked".into()),
    }
    println!("protocol-proof OK: hello plain (framed:true) -> framed server.status round-trip on channel 0");
    println!("protocol-proof OK: wire HEADER=9 MAX_PAYLOAD=16MiB WINDOW=256KiB, notes=false read by the library, require_existing fails cleanly with no server");
}

/// Checks 1-3 (constants, config, no-auto-start) without the socket.
fn run_checks_only(config_path: &Path, scratch: &Path) -> Result<(), String> {
    if wire::HEADER != 9 {
        return Err(format!("HEADER = {}, expected 9", wire::HEADER));
    }
    if wire::MAX_PAYLOAD != 16 << 20 {
        return Err(format!("MAX_PAYLOAD = {}, expected 16 MiB", wire::MAX_PAYLOAD));
    }
    if wire::WINDOW != 256 * 1024 {
        return Err(format!("WINDOW = {}, expected 256 KiB", wire::WINDOW));
    }
    let payload = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"hello\",\"params\":{\"framed\":true}}";
    let frame = wire::encode(0, wire::DATA, payload);
    if frame.len() != wire::HEADER + payload.len() {
        return Err("encoded frame has the wrong length".into());
    }
    let mut head = [0u8; wire::HEADER];
    head.copy_from_slice(&frame[..wire::HEADER]);
    let (channel, kind, len) = wire::decode_header(&head).map_err(|e| format!("decode_header: {e}"))?;
    if (channel, kind, len) != (0, wire::DATA, payload.len()) {
        return Err(format!("decoded header ({channel}, {kind}, {len}) != (0, DATA, {})", payload.len()));
    }
    let cfg = config::load(config_path).map_err(|e| format!("config::load: {}", e.message))?;
    if cfg.notes != Some(false) {
        return Err(format!("adapter config notes = {:?}, expected Some(false)", cfg.notes));
    }
    let bogus = scratch.join("no-such-core.sock");
    std::env::set_var("HC_SOCKET", &bogus);
    let paths = herdr_core::paths::Paths::from_env()
        .map_err(|e| format!("Paths::from_env: {}", e.message))?;
    let refused = herdr_core::client::require_existing(&paths).is_err();
    std::env::remove_var("HC_SOCKET");
    if !refused {
        return Err("require_existing unexpectedly connected with no server".into());
    }
    Ok(())
}

/// Accept one connection: plain `hello` (framed:true), then one framed call.
fn accept_stub(listener: UnixListener) -> Result<(), String> {
    let (mut stream, _) = listener.accept().map_err(|e| format!("stub accept: {e}"))?;
    stream.set_read_timeout(Some(TIMEOUT)).map_err(|e| e.to_string())?;
    stream.set_write_timeout(Some(TIMEOUT)).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| format!("stub read hello: {e}"))?;
    let hello: Value = serde_json::from_str(&line).map_err(|e| format!("stub parse hello: {e}"))?;
    if hello["method"] != json!("hello") || hello["params"]["framed"] != json!(true) {
        return Err(format!("stub got unexpected hello: {hello}"));
    }
    let id = hello["id"].clone();
    let hello_reply = json!({"jsonrpc": "2.0", "id": id, "result": {
        "generation": 1, "version": "0.1.18-stub",
        "core": {"id": "stub", "version": "0.1.18-stub"},
        "framed": true, "methods": ["hello", "server.status"], "features": []}});
    stream
        .write_all(format!("{hello_reply}\n").as_bytes())
        .map_err(|e| format!("stub write hello reply: {e}"))?;
    let mut head = [0u8; wire::HEADER];
    reader.read_exact(&mut head).map_err(|e| format!("stub read frame header: {e}"))?;
    let (channel, kind, len) = wire::decode_header(&head).map_err(|e| format!("stub decode header: {e}"))?;
    if (channel, kind) != (0, wire::DATA) {
        return Err(format!("stub got frame on ({channel}, {kind}), expected (0, DATA)"));
    }
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).map_err(|e| format!("stub read frame payload: {e}"))?;
    let req: Value = serde_json::from_slice(&payload).map_err(|e| format!("stub parse framed request: {e}"))?;
    if req["method"] != json!("server.status") {
        return Err(format!("stub got unexpected framed method: {req}"));
    }
    let reply = rpc::response(req["id"].clone(), Ok(json!({"version": "0.1.18-stub", "pid": 0, "uptime_secs": 0})));
    stream
        .write_all(&wire::encode(0, wire::DATA, reply.to_string().as_bytes()))
        .map_err(|e| format!("stub write framed reply: {e}"))?;
    Ok(())
}
