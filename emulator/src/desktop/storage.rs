use push_protocol::Credential;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct CredentialStore {
    pub credentials: Vec<Credential>,
    pub last_sync: Option<String>,
}

pub struct DesktopStorage {
    file_path: PathBuf,
}

impl DesktopStorage {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let file_path = PathBuf::from("./data/credentials.json");

        // Create data directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        Ok(Self { file_path })
    }

    pub fn load(&self) -> Result<Vec<Credential>, Box<dyn Error>> {
        if !self.file_path.exists() {
            println!("No credentials file found, starting with empty list");
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&self.file_path)?;
        let store: CredentialStore = serde_json::from_str(&contents)?;

        println!(
            "Loaded {} credentials from {}",
            store.credentials.len(),
            self.file_path.display()
        );

        if let Some(last_sync) = &store.last_sync {
            println!("Last sync: {}", last_sync);
        }

        Ok(store.credentials)
    }

    pub fn save(&self, credentials: &[Credential]) -> Result<(), Box<dyn Error>> {
        let store = CredentialStore {
            credentials: credentials.to_vec(),
            last_sync: Some(chrono::Utc::now().to_rfc3339()),
        };

        let json = serde_json::to_string_pretty(&store)?;
        fs::write(&self.file_path, json)?;

        println!(
            "Saved {} credentials to {}",
            credentials.len(),
            self.file_path.display()
        );

        Ok(())
    }

    pub fn clear(&self) -> Result<(), Box<dyn Error>> {
        if self.file_path.exists() {
            fs::remove_file(&self.file_path)?;
            println!("Cleared credentials file");
        }
        Ok(())
    }
}
