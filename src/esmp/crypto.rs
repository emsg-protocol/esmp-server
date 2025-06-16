use ed25519_dalek::{Keypair, PublicKey, Signature, Signer, Verifier};
use base64::{engine::general_purpose, Engine as _};

pub fn verify_signature(
    pubkey_b64: &str,
    signature_b64: &str,
    message: &[u8],
) -> bool {
    let pubkey_bytes = match general_purpose::STANDARD.decode(pubkey_b64) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let signature_bytes = match general_purpose::STANDARD.decode(signature_b64) {
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
    pubkey.verify(message, &signature).is_ok()
}

pub fn sign_message(keypair: &Keypair, message: &[u8]) -> String {
    let signature = keypair.sign(message);
    general_purpose::STANDARD.encode(signature.to_bytes())
}
