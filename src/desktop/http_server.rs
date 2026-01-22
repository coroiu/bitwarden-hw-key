use crate::credentials::{Credential, SyncRequest, SyncResponse};
use std::error::Error;
use std::sync::{Arc, Mutex};
use tiny_http::{Header, Method, Response, Server, StatusCode};

pub struct SyncServer {
    server: Server,
    credentials: Arc<Mutex<Vec<Credential>>>,
}

impl SyncServer {
    pub fn new(addr: &str) -> Result<Self, Box<dyn Error>> {
        let server = Server::http(addr).map_err(|e| format!("Failed to start server: {}", e))?;
        Ok(Self {
            server,
            credentials: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn get_credentials_ref(&self) -> Arc<Mutex<Vec<Credential>>> {
        self.credentials.clone()
    }

    pub fn handle_request(&self) -> Result<(), Box<dyn Error>> {
        let request = self.server.recv()?;

        match (request.method(), request.url()) {
            (&Method::Post, "/api/sync") => self.handle_sync(request),
            (&Method::Get, "/api/status") => self.handle_status(request),
            (&Method::Post, "/api/clear") => self.handle_clear(request),
            _ => request
                .respond(Response::from_string("Not Found").with_status_code(StatusCode(404)))
                .map_err(|e| e.into()),
        }
    }

    fn handle_sync(&self, mut request: tiny_http::Request) -> Result<(), Box<dyn Error>> {
        // Parse CBOR body
        let sync_req: SyncRequest = ciborium::from_reader(request.as_reader())?;

        let total_bytes = request.body_length().unwrap_or(0);

        // Store credentials
        let mut creds = self.credentials.lock().unwrap();
        *creds = sync_req.credentials;
        let synced = creds.len();
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
        let mut creds = self.credentials.lock().unwrap();
        creds.clear();
        drop(creds);

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

    pub fn get_credentials(&self) -> Vec<Credential> {
        self.credentials.lock().unwrap().clone()
    }
}
