//! Proof of the framed `hc` protocol (H-F1-3) against a local stub —
//! never against a V5 `hc` server, never against `releases.herdr.dev`.
//!
//! 1. Framing round-trip: a JSON-RPC request on channel 0, kind DATA,
//!    encoded with `wire::encode`, read back with `wire::decode_header`,
//!    answered with a framed JSON-RPC response, decoded the same way.
//! 2. Plain-mode `client::call` round-trip: the stub reads one JSON line
//!    and answers `{"jsonrpc": "2.0", "id": 1, "result": …}`.
//! 3. Adapter constants: gel, bind, fiber address, `notes` forced OFF.
//! 4. Default socket: `$XDG_RUNTIME_DIR/herdr-core/default.sock`.

use aspace_herdr_core_adapter as adapter;
use serde_json::json;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;

#[test]
fn framed_jsonrpc_roundtrip_on_channel_zero() {
    let (mut client_end, mut stub_end) = UnixStream::pair().unwrap();
    let request = json!({"jsonrpc": "2.0", "id": 1, "method": "hello", "params": {}});
    let request_bytes = serde_json::to_vec(&request).unwrap();
    let frame = adapter::wire::encode(0, adapter::wire::DATA, &request_bytes);
    // Header shape: [channel u32 LE][kind u8][len u32 LE][payload].
    assert_eq!(frame.len(), adapter::wire::HEADER + request_bytes.len());
    assert_eq!(&frame[0..4], &0u32.to_le_bytes());
    assert_eq!(frame[4], adapter::wire::DATA);
    assert_eq!(
        &frame[5..9],
        &(request_bytes.len() as u32).to_le_bytes()
    );

    let stub = std::thread::spawn(move || {
        let mut header = [0u8; adapter::wire::HEADER];
        stub_end.read_exact(&mut header).unwrap();
        let (channel, kind, len) = adapter::wire::decode_header(&header).unwrap();
        assert_eq!((channel, kind), (0, adapter::wire::DATA));
        let mut payload = vec![0u8; len];
        stub_end.read_exact(&mut payload).unwrap();
        let got: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(got["method"], "hello");
        assert_eq!(got["id"], 1);
        let response = json!({"jsonrpc": "2.0", "id": 1, "result": {"hello": "stub"}});
        let response_bytes = serde_json::to_vec(&response).unwrap();
        stub_end
            .write_all(&adapter::wire::encode(0, adapter::wire::DATA, &response_bytes))
            .unwrap();
    });

    client_end.write_all(&frame).unwrap();
    let mut header = [0u8; adapter::wire::HEADER];
    client_end.read_exact(&mut header).unwrap();
    let (channel, kind, len) = adapter::wire::decode_header(&header).unwrap();
    assert_eq!((channel, kind), (0, adapter::wire::DATA));
    let mut payload = vec![0u8; len];
    client_end.read_exact(&mut payload).unwrap();
    let reply: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(reply["result"]["hello"], "stub");
    assert_eq!(reply["id"], 1);
    stub.join().unwrap();
}

#[test]
fn plain_mode_client_call_roundtrip() {
    let (client_end, mut stub_end) = UnixStream::pair().unwrap();
    let stub = std::thread::spawn(move || {
        let mut reader = BufReader::new(&stub_end);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let req: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(req["method"], "server.status");
        assert_eq!(req["jsonrpc"], "2.0");
        let reply = "{\"jsonrpc\": \"2.0\", \"id\": 1, \"result\": {\"core\": {\"id\": \"stub\"}}}\n";
        stub_end.write_all(reply.as_bytes()).unwrap();
    });
    let result = adapter::client::call(&client_end, "server.status", json!({})).unwrap();
    assert_eq!(result["core"]["id"], "stub");
    stub.join().unwrap();
}

#[test]
fn adapter_constants_are_the_f1_pins() {
    assert_eq!(
        adapter::PINNED_UPSTREAM_COMMIT,
        "cd12ddd88bc11490228343291dc1096c9ed3bf77"
    );
    assert_eq!(
        adapter::BIND_COMMIT,
        "14a37722ec6ba968b0bfdaacce4ec76b21b0b4ca"
    );
    assert_eq!(adapter::FIBER_ADDRESS, "fiber:a0:herdr-core");
    // D-F1-5: notes forced OFF (upstream default ON).
    assert!(!adapter::forced_notes());
}

#[test]
fn default_socket_is_under_xdg_runtime_dir() {
    let runtime = std::path::Path::new("/run/user/1000");
    assert_eq!(
        adapter::default_socket_path(runtime, "default"),
        std::path::Path::new("/run/user/1000/herdr-core/default.sock")
    );
    // A named session keeps its own socket next to the default one.
    assert_eq!(
        adapter::default_socket_path(runtime, "alpha"),
        std::path::Path::new("/run/user/1000/herdr-core/alpha.sock")
    );
}
