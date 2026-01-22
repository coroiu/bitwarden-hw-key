use crate::credentials::{Credential, SyncRequest, SyncResponse};
use crate::desktop::DesktopStorage;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tiny_http::{Header, Method, Response, Server, StatusCode};

pub struct SyncServer {
    server: Server,
    credentials: Arc<Mutex<Vec<Credential>>>,
    storage: Arc<Mutex<DesktopStorage>>,
    should_shutdown: Arc<AtomicBool>,
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
        })
    }

    pub fn get_credentials_ref(&self) -> Arc<Mutex<Vec<Credential>>> {
        self.credentials.clone()
    }

    pub fn get_shutdown_signal(&self) -> Arc<AtomicBool> {
        self.should_shutdown.clone()
    }

    pub fn handle_request(&self) -> Result<(), Box<dyn Error>> {
        let request = self.server.recv()?;

        match (request.method(), request.url()) {
            (&Method::Post, "/api/sync") => self.handle_sync(request),
            (&Method::Get, "/api/status") => self.handle_status(request),
            (&Method::Post, "/api/clear") => self.handle_clear(request),
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
