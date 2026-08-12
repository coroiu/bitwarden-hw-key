use crate::desktop::DesktopStorage;
use crate::platform::HeadlessSurface;
use bhk_core::input::NavIntent;
use push_protocol::{Credential, SyncRequest, SyncResponse};
use std::collections::VecDeque;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tiny_http::{Header, Method, Response, Server, StatusCode};

pub struct SyncServer {
    server: Server,
    credentials: Arc<Mutex<Vec<Credential>>>,
    storage: Arc<Mutex<DesktopStorage>>,
    should_shutdown: Arc<AtomicBool>,
    /// Fed by `POST /api/input` (W5), drained by a headless
    /// `emulator::platform::HttpInput` on every render-loop poll. See
    /// `.planning/decisions/2026-08-11-three-mode-testability.md`.
    input_queue: Arc<Mutex<VecDeque<NavIntent>>>,
    /// The `HeadlessSurface` `GET /api/screenshot` reads from, if any.
    /// `None` in windowed mode (there is no `HeadlessSurface` to read);
    /// set via `set_screenshot_surface` before the server starts handling
    /// requests in headless mode. See `emulator::platform::headless_surface::SharedHeadlessSurface`.
    screenshot_surface: Option<Arc<Mutex<HeadlessSurface>>>,
}

impl SyncServer {
    pub fn new(addr: &str, storage: Arc<Mutex<DesktopStorage>>) -> Result<Self, Box<dyn Error>> {
        let server = Server::http(addr).map_err(|e| format!("Failed to start server: {}", e))?;

        // Load existing credentials from storage
        let loaded_creds = storage
            .lock()
            .unwrap()
            .load()
            .unwrap_or_else(|e| {
                eprintln!("Failed to load credentials: {}", e);
                Vec::new()
            });

        Ok(Self {
            server,
            credentials: Arc::new(Mutex::new(loaded_creds)),
            storage,
            should_shutdown: Arc::new(AtomicBool::new(false)),
            input_queue: Arc::new(Mutex::new(VecDeque::new())),
            screenshot_surface: None,
        })
    }

    pub fn get_credentials_ref(&self) -> Arc<Mutex<Vec<Credential>>> {
        self.credentials.clone()
    }

    pub fn get_shutdown_signal(&self) -> Arc<AtomicBool> {
        self.should_shutdown.clone()
    }

    /// Hands out the shared queue `POST /api/input` enqueues `NavIntent`s
    /// into. A headless `emulator::platform::HttpInput` holds the other
    /// end, draining it on every `InputSource::poll()` — that's what
    /// actually gets an injected intent to the running `App` (see
    /// `bhk_core::run::run`'s loop; no changes to that loop were needed).
    #[must_use]
    pub fn get_input_queue_ref(&self) -> Arc<Mutex<VecDeque<NavIntent>>> {
        self.input_queue.clone()
    }

    /// Registers the `HeadlessSurface` handle `GET /api/screenshot` reads
    /// from. Only meaningful in headless mode; windowed mode never calls
    /// this, so `/api/screenshot` responds 404 there. Must be called
    /// before the server starts handling requests (i.e. before moving the
    /// `SyncServer` into its request-loop thread) — there is no
    /// synchronization protecting concurrent calls to this method itself.
    pub fn set_screenshot_surface(&mut self, surface: Arc<Mutex<HeadlessSurface>>) {
        self.screenshot_surface = Some(surface);
    }

    /// The address the server actually bound to. Useful when binding to
    /// port 0 (an ephemeral port), e.g. in tests that don't want to
    /// hardcode/collide on 8080.
    ///
    /// # Panics
    ///
    /// Panics if the underlying listener isn't a TCP socket (`tiny_http`
    /// also supports Unix sockets, which this project never binds to —
    /// `new` is always called with an `ip:port` string).
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.server.server_addr().to_ip().expect("SyncServer always binds a TCP address, never a Unix socket")
    }

    pub fn handle_request(&self) -> Result<(), Box<dyn Error>> {
        let request = self.server.recv()?;

        match (request.method(), request.url()) {
            (&Method::Post, "/api/sync") => self.handle_sync(request),
            (&Method::Get, "/api/status") => self.handle_status(request),
            (&Method::Post, "/api/clear") => self.handle_clear(request),
            (&Method::Post, "/api/input") => self.handle_input(request),
            (&Method::Get, "/api/screenshot") => self.handle_screenshot(request),
            (&Method::Post, "/api/shutdown") => self.handle_shutdown(request),
            _ => request
                .respond(Response::from_string("Not Found").with_status_code(StatusCode(404)))
                .map_err(|e| e.into()),
        }
    }

    fn handle_sync(&self, mut request: tiny_http::Request) -> Result<(), Box<dyn Error>> {
        // Parse CBOR body
        let sync_req: SyncRequest = ciborium::from_reader(request.as_reader())?;

        let total_bytes = request.body_length().unwrap_or(0);

        // Store credentials in memory
        let mut creds = self.credentials.lock().unwrap();
        *creds = sync_req.credentials;
        let synced = creds.len();

        // Persist to storage
        self.storage
            .lock()
            .unwrap()
            .save(&creds)
            .map_err(|e| format!("Failed to save credentials: {}", e))?;

        drop(creds);

        // Respond with JSON
        let response = SyncResponse {
            status: "success".to_string(),
            synced,
            total_bytes,
        };
        let json = serde_json::to_string(&response)?;

        request
            .respond(
                Response::from_string(json)
                    .with_header("Content-Type: application/json".parse::<Header>().unwrap())
                    .with_header(
                        "Access-Control-Allow-Origin: http://localhost:4200"
                            .parse::<Header>()
                            .unwrap(),
                    ),
            )
            .map_err(|e| e.into())
    }

    fn handle_status(&self, request: tiny_http::Request) -> Result<(), Box<dyn Error>> {
        let creds = self.credentials.lock().unwrap();
        let count = creds.len();
        drop(creds);

        let status = serde_json::json!({
            "status": "running",
            "credential_count": count,
        });

        request
            .respond(
                Response::from_string(status.to_string())
                    .with_header("Content-Type: application/json".parse::<Header>().unwrap())
                    .with_header(
                        "Access-Control-Allow-Origin: http://localhost:4200"
                            .parse::<Header>()
                            .unwrap(),
                    ),
            )
            .map_err(|e| e.into())
    }

    fn handle_clear(&self, request: tiny_http::Request) -> Result<(), Box<dyn Error>> {
        // Clear credentials in memory
        let mut creds = self.credentials.lock().unwrap();
        creds.clear();
        drop(creds);

        // Clear storage file
        self.storage
            .lock()
            .unwrap()
            .clear()
            .map_err(|e| format!("Failed to clear storage: {}", e))?;

        let response = serde_json::json!({
            "status": "success",
            "message": "Credentials cleared",
        });

        request
            .respond(
                Response::from_string(response.to_string())
                    .with_header("Content-Type: application/json".parse::<Header>().unwrap())
                    .with_header(
                        "Access-Control-Allow-Origin: http://localhost:4200"
                            .parse::<Header>()
                            .unwrap(),
                    ),
            )
            .map_err(|e| e.into())
    }

    /// `POST /api/input`: enqueues a `NavIntent` for a headless
    /// `HttpInput` to drain on its next poll — the agent-drivable input
    /// half of the headless testability protocol (see
    /// `.planning/decisions/2026-08-11-three-mode-testability.md`). Body
    /// is the JSON form of `bhk_core::input::NavIntent`'s derived
    /// `Deserialize`: a bare string for the unit variants (e.g. `"Next"`,
    /// `"Prev"`, `"Activate"`, `"Back"`) or `{"NextN":5}` for the one
    /// tuple variant.
    fn handle_input(&self, mut request: tiny_http::Request) -> Result<(), Box<dyn Error>> {
        let intent: NavIntent = serde_json::from_reader(request.as_reader())?;
        self.input_queue.lock().unwrap().push_back(intent);

        let response = serde_json::json!({
            "status": "success",
            "queued": format!("{intent:?}"),
        });

        request
            .respond(
                Response::from_string(response.to_string())
                    .with_header("Content-Type: application/json".parse::<Header>().unwrap())
                    .with_header(
                        "Access-Control-Allow-Origin: http://localhost:4200"
                            .parse::<Header>()
                            .unwrap(),
                    ),
            )
            .map_err(Into::into)
    }

    /// `GET /api/screenshot`: PNG-encodes the framebuffer most recently
    /// flushed to the registered `HeadlessSurface` (see
    /// `set_screenshot_surface`) — the observable half of the headless
    /// testability protocol, paired with `handle_input` above. Responds
    /// 404 if no surface is registered (not running headless) and 503 if
    /// a surface is registered but nothing has been rendered yet.
    fn handle_screenshot(&self, request: tiny_http::Request) -> Result<(), Box<dyn Error>> {
        let Some(surface) = &self.screenshot_surface else {
            return request
                .respond(
                    Response::from_string("Screenshot unavailable: not running in headless mode")
                        .with_status_code(StatusCode(404)),
                )
                .map_err(Into::into);
        };

        match surface.lock().unwrap().encode_png() {
            Some(png_bytes) => request
                .respond(
                    Response::from_data(png_bytes)
                        .with_header("Content-Type: image/png".parse::<Header>().unwrap()),
                )
                .map_err(Into::into),
            None => request
                .respond(Response::from_string("No frame rendered yet").with_status_code(StatusCode(503)))
                .map_err(Into::into),
        }
    }

    fn handle_shutdown(&self, request: tiny_http::Request) -> Result<(), Box<dyn Error>> {
        // Signal shutdown
        self.should_shutdown.store(true, Ordering::Relaxed);

        let response = serde_json::json!({
            "status": "success",
            "message": "Shutting down emulator",
        });

        request
            .respond(
                Response::from_string(response.to_string())
                    .with_header("Content-Type: application/json".parse::<Header>().unwrap())
                    .with_header(
                        "Access-Control-Allow-Origin: http://localhost:4200"
                            .parse::<Header>()
                            .unwrap(),
                    ),
            )
            .map_err(|e| e.into())
    }

    pub fn get_credentials(&self) -> Vec<Credential> {
        self.credentials.lock().unwrap().clone()
    }
}
