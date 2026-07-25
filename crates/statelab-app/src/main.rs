//! StateLab interim double-click runner.
//!
//! A dependency-light (`std` + `serde_json`) host that lets you *use* the engine
//! today, before the frozen Tauri shell exists (that is Phase 3). It:
//!   1. binds a local HTTP server on an ephemeral loopback port,
//!   2. opens your default browser at it,
//!   3. serves the embedded UI, and on `GET /api/run?n=...` runs the **real
//!      engine** and returns a finalized Trajectory as JSON.
//!
//! Architectural fidelity: all mathematics stays in `statelab-engine` (the single
//! source of truth, Principle #4). The UI is a pure consumer over an HTTP boundary
//! — the same request/response shape the Tauri `invoke("run_trajectory", …)`
//! command will have (§3.2). This binary is a demo host, **not** a replacement for
//! the planned Tauri shell, and it contains no trajectory mathematics of its own.
//!
//! Hidden-window note: built as a console subsystem app so double-clicking shows a
//! small console with the local URL. The UI itself is the browser tab.

#![forbid(unsafe_code)]

mod dataset;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};

use statelab_engine::{ClassicCollatz, EngineConfig, InitialStateInput, TrajectoryCache};

/// Process-wide memoization cache (§4.8), shared across connection threads. The
/// Research Controller owns caching; the pure engine stays stateless.
type SharedCache = Arc<Mutex<TrajectoryCache>>;

/// The UI, embedded at compile time so the shipped `.exe` is a single file.
///
/// `embedded_ui.html` is the production build of the React + TS + Tailwind
/// frontend (`src/`), inlined into one file by `vite-plugin-singlefile`. Regenerate
/// it with `npm run build && cp dist/index.html crates/statelab-app/src/embedded_ui.html`
/// (see the `sync-ui` npm script). It is a generated artifact — edit `src/`, not this.
const INDEX_HTML: &str = include_str!("embedded_ui.html");

fn main() {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(e) => {
            eprintln!("StateLab: could not bind a local port: {e}");
            wait_for_enter();
            return;
        }
    };

    let url = match listener.local_addr() {
        Ok(addr) => format!("http://{addr}/"),
        Err(_) => "http://127.0.0.1/".to_string(),
    };

    println!("StateLab is running.");
    println!("  Open this in your browser if it doesn't open automatically:");
    println!("    {url}");
    println!("  (Close this window to stop the app.)");

    open_browser(&url);

    let cache: SharedCache = Arc::new(Mutex::new(TrajectoryCache::from_config(
        &EngineConfig::default(),
    )));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // One thread per connection: keeps the UI responsive without an
                // async runtime. Volume is trivially low (single local user).
                let cache = Arc::clone(&cache);
                std::thread::spawn(move || handle(stream, cache));
            }
            Err(e) => eprintln!("connection error: {e}"),
        }
    }
}

/// Handles one HTTP request: routes `/`, `/api/run`, `/api/dataset`, and a favicon.
fn handle(stream: TcpStream, cache: SharedCache) {
    let mut reader = BufReader::new(&stream);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }

    // Drain headers, capturing Content-Length (needed for POST bodies).
    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => break,
            Ok(_) => {
                if header == "\r\n" || header == "\n" {
                    break;
                }
                if let Some(rest) = header.to_ascii_lowercase().strip_prefix("content-length:") {
                    content_length = rest.trim().parse().unwrap_or(0);
                }
            }
            Err(_) => return,
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target.as_str(), ""),
    };

    // Read the body for POST (used by CSV dataset import), bounded defensively.
    let mut body = String::new();
    if method.eq_ignore_ascii_case("POST") && content_length > 0 {
        let mut buf = vec![0u8; content_length.min(8 * 1024 * 1024)];
        if reader.read_exact(&mut buf).is_ok() {
            body = String::from_utf8_lossy(&buf).into_owned();
        }
    }

    match path {
        "/" => respond(
            &stream,
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML.as_bytes(),
        ),
        "/api/run" => {
            // Accept the Research Controller's params (`initialState`,
            // `maxIterations`); fall back to `n` for ad-hoc callers.
            let initial_state = query_param(query, "initialState")
                .or_else(|| query_param(query, "n"))
                .unwrap_or_default();
            let max_iterations = query_param(query, "maxIterations").and_then(|v| v.parse().ok());
            let response = run_engine(&initial_state, max_iterations, &cache);
            respond(
                &stream,
                "200 OK",
                "application/json; charset=utf-8",
                response.as_bytes(),
            );
        }
        "/api/dataset" => stream_dataset_response(&stream, query, &body),
        "/favicon.ico" => respond(&stream, "204 No Content", "text/plain", b""),
        _ => respond(&stream, "404 Not Found", "text/plain", b"not found"),
    }
}

/// Streams a dataset as NDJSON. The response has no Content-Length — the body is
/// delimited by the connection close (`Connection: close`), so the client reads it
/// incrementally as the engine produces each summary row. If the client
/// disconnects mid-stream, the write fails and generation stops.
fn stream_dataset_response(mut stream: &TcpStream, query: &str, body: &str) {
    let spec = match dataset::spec_from_request(query, body) {
        Some(spec) => spec,
        None => {
            respond(
                stream,
                "400 Bad Request",
                "text/plain",
                b"invalid dataset spec",
            );
            return;
        }
    };
    let max_iterations = query_param(query, "maxIterations").and_then(|v| v.parse().ok());

    let header = "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson; charset=utf-8\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
    if stream.write_all(header.as_bytes()).is_err() {
        return;
    }
    let _ = dataset::stream_dataset(spec, max_iterations, &mut stream);
    let _ = stream.flush();
}

/// Runs the real engine for `initial_state` (through the memoization cache) and
/// serializes the finalized Trajectory to JSON. Invalid input still yields a
/// well-formed `SystemError` Trajectory (never a crash).
fn run_engine(initial_state: &str, max_iterations: Option<u64>, cache: &SharedCache) -> String {
    let system = ClassicCollatz;
    let config = match max_iterations {
        Some(max) => EngineConfig::with_max_iterations(max),
        None => EngineConfig::default(),
    };
    let input = InitialStateInput::new(initial_state);
    let trajectory = match cache.lock() {
        Ok(mut guard) => guard.get_or_compute(&system, &input, &config),
        // If a previous handler panicked while holding the lock, fall back to a
        // direct (uncached) run rather than failing the request.
        Err(poisoned) => poisoned
            .into_inner()
            .get_or_compute(&system, &input, &config),
    };
    serde_json::to_string(&trajectory)
        .unwrap_or_else(|e| format!("{{\"error\":\"serialization failed: {e}\"}}"))
}

/// Minimal `application/x-www-form-urlencoded` lookup with percent/`+` decoding.
pub(crate) fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == key).then(|| percent_decode(v))
    })
}

/// Decodes `%XX` escapes and `+` (space) in a query value.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_val(bytes[i + 1]);
                let lo = hex_val(bytes[i + 2]);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push(h << 4 | l);
                        i += 3;
                    }
                    _ => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Writes a complete HTTP/1.1 response and closes the connection.
fn respond(mut stream: &TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// Best-effort: open the default browser at `url` (Windows `start` via `cmd`).
fn open_browser(url: &str) {
    let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
}

fn wait_for_enter() {
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
}
