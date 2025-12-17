//! Secure QR code key exchange with anti-spoofing measures
//!
//! ## Security Design:
//! 1. QR contains: public key + fingerprint + timestamp + self-signature
//! 2. Visual identicon displayed next to QR for out-of-band verification
//! 3. Timestamp prevents replay attacks (reject QRs older than 5 minutes)
//! 4. No URLs/IPs in QR (only cryptographic material)
//! 5. Self-signature proves QR creator had private key
//!
//! ## Threat Mitigations:
//! - MITM/Phishing → Visual identicon + fingerprint verification UI
//! - Replay attacks → Timestamp validation (5-minute window)
//! - Data injection → Strict schema validation + signature verification
//! - Visual spoofing → Identicon computed from fingerprint hash

use anyhow::{Result, Context, bail};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use qrcode::{QrCode, EcLevel};
use image::{DynamicImage, Luma, ImageBuffer};
use rqrr::PreparedImage;

use cryptochat_crypto_core::pgp::PgpKeyPair;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

const QR_VERSION: u32 = 1;

const MAX_QR_AGE_SECONDS: i64 = 300; // 5 minutes
#[derive(Debug, Serialize, Deserialize)]
pub struct QrPayload {
    /// Protocol version
    v: u32,

    /// PGP key fingerprint (40 hex characters)
    fp: String,

    /// ASCII-armored public key
    pk: String,

    /// Unix timestamp (seconds since epoch)
    ts: i64,

    /// Self-signature over {v, fp, pk, ts} using private key
    /// This proves the QR creator had access to the private key
    sig: String,
}

impl QrPayload {
    /// Create a new QR payload with self-signature
    pub fn new(fingerprint: String, public_key: String, signature: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        Self {
            v: QR_VERSION,
            fp: fingerprint,
            pk: public_key,
            ts: timestamp,
            sig: signature,
        }
    }

    /// Create a new QR payload with automatic signature generation
    pub fn create_and_sign(keypair: &PgpKeyPair) -> Result<Self> {
        let fingerprint = keypair.fingerprint();
        let public_key = keypair.export_public_key()
            .context("Failed to export public key")?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs() as i64;

        // Create canonical message to sign: v||fp||pk||ts
        let message = format!("{}{}{}{}", QR_VERSION, fingerprint, public_key, timestamp);

        // Sign the message
        let signature_bytes = keypair.sign(message.as_bytes())
            .context("Failed to sign QR payload")?;
        let signature = BASE64.encode(&signature_bytes);

        Ok(Self {
            v: QR_VERSION,
            fp: fingerprint,
            pk: public_key,
            ts: timestamp,
            sig: signature,
        })
    }

    /// Get the public key from the QR payload
    pub fn public_key(&self) -> &str {
        &self.pk
    }
    /// Validate QR payload security properties
    pub fn validate(&self) -> Result<()> {
        // Check version
        if self.v != QR_VERSION {
            bail!("Unsupported QR code version: {}", self.v);
        }

        // Check fingerprint format (40 hex chars)
        if self.fp.len() != 40 || !self.fp.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("Invalid fingerprint format: must be 40 hex characters");
        }

        // Check timestamp (reject if older than 5 minutes)
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let age = now - self.ts;

        if age < 0 {
            bail!("QR code from the future! Clock skew detected.");
        }

        if age > MAX_QR_AGE_SECONDS {
            bail!("QR code expired (age: {}s). For security, QR codes are only valid for 5 minutes. Please generate a fresh one.", age);
        }

        // Check public key format (basic validation)
        if !self.pk.starts_with("-----BEGIN PGP PUBLIC KEY BLOCK-----") {
            bail!("Invalid public key format");
        }

        // Verify self-signature to prevent QR spoofing
        self.verify_signature()?;

        Ok(())
    }

    /// Verify the self-signature on the QR payload
    fn verify_signature(&self) -> Result<()> {
        // Parse the public key from the payload
        let keypair = PgpKeyPair::from_public_key(&self.pk)
            .context("Failed to parse public key from QR payload")?;

        // Verify the fingerprint matches
        if keypair.fingerprint() != self.fp {
            bail!("SECURITY WARNING: Fingerprint mismatch! The fingerprint in the QR code does not match the public key. This QR may have been tampered with.");
        }

        // Recreate the canonical message that was signed
        let message = format!("{}{}{}{}", self.v, self.fp, self.pk, self.ts);

        // Decode the signature from base64
        let signature_bytes = BASE64.decode(&self.sig)
            .context("Failed to decode signature")?;

        // Verify the signature
        PgpKeyPair::verify(keypair.cert(), message.as_bytes(), &signature_bytes)
            .context("SECURITY WARNING: Signature verification failed! This QR code may have been forged or tampered with.")?;

        Ok(())
    }

    /// Serialize to JSON for QR encoding
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).context("Failed to serialize QR payload")
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> Result<Self> {
        let payload: QrPayload = serde_json::from_str(json)
            .context("Failed to parse QR code data")?;
        payload.validate()?;
        Ok(payload)
    }
}

/// Generate a QR code image from a payload
pub fn generate_qr_image(payload: &QrPayload) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>> {
    let json = payload.to_json()?;

    // QR code size limits: L=Low allows ~2953 bytes, M=Medium ~2331, Q=Quartile ~1663, H=High ~1273
    // Armored PGP keys are typically 2-4KB, so we need L (lowest error correction)
    let code = QrCode::with_error_correction_level(json.as_bytes(), EcLevel::L)
        .context("Failed to create QR code - payload may be too large (max ~2900 bytes for QR)")?;
    
    let image = code.render::<Luma<u8>>()
        .min_dimensions(400, 400)
        .max_dimensions(800, 800)
        .build();
    
    Ok(image)
}

/// Save QR code image to file
pub fn save_qr_to_file(img: &ImageBuffer<Luma<u8>, Vec<u8>>, filename: &str) -> Result<()> {
    img.save(filename)
        .context("Failed to save QR code image")?;
    Ok(())
}

/// Scan a QR code from an image file (PC-friendly - no camera needed)
pub fn scan_qr_from_file(path: &str) -> Result<QrPayload> {
    let img = image::open(path)
        .context("Failed to open image file")?;
    
    let img_luma = img.to_luma8();
    let mut img_prepared = PreparedImage::prepare(img_luma);
    let grids = img_prepared.detect_grids();
    
    if grids.is_empty() {
        bail!("No QR code found in image");
    }
    
    let (_meta, content) = grids[0].decode()
        .context("Failed to decode QR code")?;
    
    QrPayload::from_json(&content)
}

/// Generate a 5x5 identicon for visual verification
pub fn generate_identicon(fingerprint: &str) -> [[bool; 5]; 5] {
    let mut grid = [[false; 5]; 5];
    let bytes = fingerprint.as_bytes();
    
    for row in 0..5 {
        for col in 0..3 {
            let idx = (row * 3 + col) % bytes.len();
            let bit = (bytes[idx] >> (col % 8)) & 1;
            grid[row][col] = bit == 1;
            grid[row][4 - col] = bit == 1;
        }
    }
    
    grid
}

/// Format fingerprint for display (groups of 4 hex chars)
pub fn format_fingerprint(fp: &str) -> String {
    fp.chars()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}
