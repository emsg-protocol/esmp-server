use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde::Deserialize;
use ed25519_dalek::{Verifier, PublicKey, Signature};
use base64::{engine::general_purpose, Engine as _};

#[derive(Debug, Deserialize)]
struct EsmpMessage {
    to: Vec<String>,
    cc: Option<Vec<String>>,
    group_id: Option<String>,
    r#type: String,
    body: serde_json::Value,
    signature: String,         // base64-encoded signature
    sender_pubkey: String,     // base64-encoded public key
}

fn verify_signature(msg: &EsmpMessage) -> bool {
    // Prepare the message for signing: canonical JSON of all fields except signature and sender_pubkey
    let mut to_sign = serde_json::json!({
        "to": msg.to,
        "cc": msg.cc,
        "group_id": msg.group_id,
        "type": msg.r#type,
        "body": msg.body
    });
    let to_sign_bytes = serde_json::to_vec(&to_sign).unwrap();
    let pubkey_bytes = match general_purpose::STANDARD.decode(&msg.sender_pubkey) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let signature_bytes = match general_purpose::STANDARD.decode(&msg.signature) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let pubkey = match PublicKey::from_bytes(&pubkey_bytes) {
        Ok(pk) => pk,
        Err(_) => return false,
    };
    let signature = match Signature::from_bytes(&signature_bytes) {
        Ok(sig) => sig,
        Err(_) => return false,
    };
    pubkey.verify(&to_sign_bytes, &signature).is_ok()
}

pub async fn start_esmp_listener() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:5888").await?;
    println!("ESMP listener started on port 5888");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("Accepted connection from {}", addr);
        tokio::spawn(async move {
            let mut reader = BufReader::new(socket);
            let mut buffer = String::new();
            loop {
                buffer.clear();
                let bytes_read = reader.read_line(&mut buffer).await.unwrap_or(0);
                if bytes_read == 0 {
                    break;
                }
                match serde_json::from_str::<EsmpMessage>(&buffer) {
                    Ok(msg) => {
                        if verify_signature(&msg) {
                            println!("Valid signed ESMP message: {:?}", msg);
                            // Group chat support
                            if let Some(group_id) = &msg.group_id {
                                let thread_file = format!("group_{}.jsonl", group_id);
                                let record = serde_json::to_string(&msg).unwrap();
                                let _ = tokio::fs::OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(&thread_file)
                                    .await
                                    .and_then(|mut file| tokio::io::AsyncWriteExt::write_all(&mut file, format!("{}\n", record).as_bytes()));
                                // System message handling
                                match msg.r#type.as_str() {
                                    "joined" | "left" | "removed" | "group_created" => {
                                        println!("System message for group {}: {}", group_id, msg.r#type);
                                    }
                                    _ => {
                                        println!("Persisted group message for group {}", group_id);
                                    }
                                }
                            } else {
                                // Handle direct (non-group) messages here if needed
                                println!("Direct message: {:?}", msg);
                            }
                        } else {
                            eprintln!("Rejected unsigned or tampered ESMP message from {}", addr);
                        }
                    }
                    Err(e) => {
                        eprintln!("Invalid ESMP message: {}", e);
                    }
                }
            }
        });
    }
}
