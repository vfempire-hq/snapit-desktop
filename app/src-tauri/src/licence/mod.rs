// licence/mod.rs — offline Ed25519 licence verification.
//
// R·01 M5: sold as one-off €69 personal / €129 family. Payment goes through
// Stripe on our tiny purchase-backend, which signs a licence blob with an
// Ed25519 key. The app verifies the blob offline using the public key baked
// into this binary. No online activation, no phone-home, no expiring keys.

use anyhow::{anyhow, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

// The public key is baked at build time. Placeholder until M5 provisions
// the real minisign-style keypair on the purchase-backend.
const LICENCE_PUB_KEY_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Serialize, Deserialize)]
pub struct Licence {
    pub email: String,
    pub tier: String, // "personal" | "family" | "pro"
    pub issued_at: i64,
    pub product: String, // "snapit"
    pub major_version: u32,
}

pub fn verify(licence_json: &str, signature_b64: &str) -> Result<Licence> {
    let pk_bytes = hex::decode(LICENCE_PUB_KEY_HEX)?;
    let pk_arr: [u8; 32] = pk_bytes.try_into().map_err(|_| anyhow!("pubkey wrong length"))?;
    let pk = VerifyingKey::from_bytes(&pk_arr)?;
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let sig_bytes = B64.decode(signature_b64)?;
    let sig = Signature::try_from(sig_bytes.as_slice())?;
    pk.verify(licence_json.as_bytes(), &sig)?;
    let l: Licence = serde_json::from_str(licence_json)?;
    if l.product != "snapit" {
        return Err(anyhow!("licence is for another product"));
    }
    Ok(l)
}
