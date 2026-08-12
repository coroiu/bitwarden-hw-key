use push_protocol::SyncRequest;
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input.json> <output.cbor>", args[0]);
        std::process::exit(1);
    }

    let input_file = &args[1];
    let output_file = &args[2];

    // Read JSON file
    let json_data = fs::read_to_string(input_file)
        .expect("Failed to read JSON file");

    // Parse JSON
    let sync_request: SyncRequest = serde_json::from_str(&json_data)
        .expect("Failed to parse JSON");

    // Encode to CBOR
    let mut cbor_bytes = Vec::new();
    ciborium::into_writer(&sync_request, &mut cbor_bytes)
        .expect("Failed to encode CBOR");

    // Write CBOR file
    fs::write(output_file, &cbor_bytes)
        .expect("Failed to write CBOR file");

    println!("Converted {} credentials to CBOR ({} bytes)",
             sync_request.credentials.len(),
             cbor_bytes.len());
    println!("Written to: {}", output_file);
}
