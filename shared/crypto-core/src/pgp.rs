//! OpenPGP implementation using Sequoia PGP.

use sequoia_openpgp as openpgp;
use openpgp::cert::{CertBuilder, CipherSuite};
use openpgp::parse::stream::*;
use openpgp::parse::Parse;
use openpgp::policy::{Policy, StandardPolicy};
use openpgp::serialize::stream::*;
use openpgp::serialize::Serialize;
use openpgp::{Cert, KeyHandle};
use std::io::{self, Write};
use crate::{CryptoError, Result};

/// Thread-safe static policy instance.
static POLICY: StandardPolicy<'static> = StandardPolicy::new();

fn policy() -> &'static dyn Policy {
    &POLICY
}

/// PGP key pair wrapper around Sequoia Cert.
#[derive(Clone)]
pub struct PgpKeyPair {
    cert: Cert,
}

impl PgpKeyPair {
    /// Generate a new Cv25519 keypair with signing and encryption subkeys.
    pub fn generate(user_id: &str) -> Result<Self> {
        let (cert, _revocation) = CertBuilder::new()
            .add_userid(user_id)
            .add_signing_subkey()
            .add_transport_encryption_subkey()
            .set_cipher_suite(CipherSuite::Cv25519)
            .generate()
            .map_err(|e| CryptoError::Internal(format!("key generation failed: {}", e)))?;
        Ok(Self { cert })
    }

    /// Export public key in ASCII-armored format.
    pub fn export_public_key(&self) -> Result<String> {
        let mut buf = Vec::new();
        let mut writer = openpgp::armor::Writer::new(&mut buf, openpgp::armor::Kind::PublicKey)
            .map_err(|e| CryptoError::Internal(format!("armor writer failed: {}", e)))?;
        self.cert.serialize(&mut writer)
            .map_err(|e| CryptoError::Internal(format!("cert serialization failed: {}", e)))?;
        writer.finalize()
            .map_err(|e| CryptoError::Internal(format!("armor finalize failed: {}", e)))?;
        String::from_utf8(buf)
            .map_err(|e| CryptoError::Internal(format!("utf8 conversion failed: {}", e)))
    }

    /// Export secret key in ASCII-armored format.
    pub fn export_secret_key(&self) -> Result<String> {
        let mut buf = Vec::new();
        let mut writer = openpgp::armor::Writer::new(&mut buf, openpgp::armor::Kind::SecretKey)
            .map_err(|e| CryptoError::Internal(format!("armor writer failed: {}", e)))?;
        self.cert.as_tsk().serialize(&mut writer)
            .map_err(|e| CryptoError::Internal(format!("tsk serialization failed: {}", e)))?;
        writer.finalize()
            .map_err(|e| CryptoError::Internal(format!("armor finalize failed: {}", e)))?;
        String::from_utf8(buf)
            .map_err(|e| CryptoError::Internal(format!("utf8 conversion failed: {}", e)))
    }

    /// Import a public key from ASCII-armored format.
    pub fn from_public_key(armored: &str) -> Result<Self> {
        let cert = Cert::from_reader(io::Cursor::new(armored.as_bytes()))
            .map_err(|e| CryptoError::Internal(format!("failed to parse cert: {}", e)))?;
        Ok(Self { cert })
    }

    /// Import a secret key from ASCII-armored format.
    pub fn from_secret_key(armored: &str) -> Result<Self> {
        let cert = Cert::from_reader(io::Cursor::new(armored.as_bytes()))
            .map_err(|e| CryptoError::Internal(format!("failed to parse cert: {}", e)))?;
        Ok(Self { cert })
    }

    /// Get the certificate fingerprint as a hex string.
    pub fn fingerprint(&self) -> String {
        self.cert.fingerprint().to_hex()
    }

    /// Access the underlying certificate.
    pub fn cert(&self) -> &Cert {
        &self.cert
    }

    /// Sign a message and return a detached signature.
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        let keypair = self.cert.keys()
            .with_policy(policy(), None)
            .supported()
            .alive()
            .revoked(false)
            .for_signing()
            .next()
            .ok_or_else(|| CryptoError::Internal("no suitable signing key found".to_string()))?
            .key()
            .clone()
            .parts_into_secret()
            .map_err(|_| CryptoError::Internal("signing key has no secret material".to_string()))?
            .into_keypair()
            .map_err(|e| CryptoError::Internal(format!("failed to create keypair: {}", e)))?;

        let mut sink = Vec::new();
        let message_writer = Message::new(&mut sink);
        let mut signer = Signer::new(message_writer, keypair)
            .detached()
            .build()
            .map_err(|e| CryptoError::Internal(format!("signer build failed: {}", e)))?;

        signer.write_all(message)
            .map_err(|e| CryptoError::Internal(format!("write failed: {}", e)))?;
        signer.finalize()
            .map_err(|e| CryptoError::Internal(format!("signer finalize failed: {}", e)))?;

        Ok(sink)
    }

    /// Verify a detached signature over a message.
    pub fn verify(cert: &Cert, message: &[u8], signature: &[u8]) -> Result<()> {
        struct Helper<'a> {
            cert: &'a Cert,
        }

        impl<'a> VerificationHelper for Helper<'a> {
            fn get_certs(&mut self, _ids: &[KeyHandle]) -> openpgp::Result<Vec<Cert>> {
                Ok(vec![self.cert.clone()])
            }

            fn check(&mut self, structure: MessageStructure) -> openpgp::Result<()> {
                for layer in structure.into_iter() {
                    match layer {
                        MessageLayer::SignatureGroup { results } => {
                            for result in results {
                                match result {
                                    Ok(_) => return Ok(()),
                                    Err(_) => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(openpgp::Error::InvalidOperation("no valid signatures found".to_string()).into())
            }
        }

        let helper = Helper { cert };
        let mut verifier = DetachedVerifierBuilder::from_bytes(signature)
            .map_err(|e| CryptoError::Internal(format!("verifier build failed: {}", e)))?
            .with_policy(policy(), None, helper)
            .map_err(|e| CryptoError::Internal(format!("verifier policy failed: {}", e)))?;

        verifier.verify_bytes(message)
            .map_err(|_e| CryptoError::VerificationFailed)?;

        Ok(())
    }

    /// Encrypt a message for a recipient's public key.
    pub fn encrypt(recipient_cert: &Cert, plaintext: &[u8]) -> Result<Vec<u8>> {
        let recipients = recipient_cert.keys()
            .with_policy(policy(), None)
            .supported()
            .alive()
            .revoked(false)
            .for_transport_encryption()
            .map(|ka| ka.key())
            .collect::<Vec<_>>();

        if recipients.is_empty() {
            return Err(CryptoError::Internal("no suitable encryption key found".to_string()));
        }

        let mut sink = Vec::new();
        let message = Message::new(&mut sink);
        let message = Armorer::new(message).build()
            .map_err(|e| CryptoError::Internal(format!("armorer build failed: {}", e)))?;
        let message = Encryptor2::for_recipients(message, recipients)
            .build()
            .map_err(|e| CryptoError::Internal(format!("encryptor build failed: {}", e)))?;
        let mut message = LiteralWriter::new(message)
            .build()
            .map_err(|e| CryptoError::Internal(format!("literal writer build failed: {}", e)))?;

        message.write_all(plaintext)
            .map_err(|e| CryptoError::Internal(format!("write failed: {}", e)))?;
        message.finalize()
            .map_err(|e| CryptoError::Internal(format!("finalize failed: {}", e)))?;

        Ok(sink)
    }

    /// Decrypt a message encrypted for this keypair.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        struct Helper<'a> {
            cert: &'a Cert,
            policy: &'static dyn Policy,
        }

        impl<'a> DecryptionHelper for Helper<'a> {
            fn decrypt<D>(
                &mut self,
                pkesks: &[openpgp::packet::PKESK],
                _skesks: &[openpgp::packet::SKESK],
                sym_algo: Option<openpgp::types::SymmetricAlgorithm>,
                mut decrypt: D,
            ) -> openpgp::Result<Option<openpgp::Fingerprint>>
            where
                D: FnMut(openpgp::types::SymmetricAlgorithm, &openpgp::crypto::SessionKey) -> bool,
            {
                let keys: Vec<_> = self.cert.keys()
                    .with_policy(self.policy, None)
                    .supported()
                    .alive()
                    .revoked(false)
                    .for_transport_encryption()
                    .secret()
                    .collect();

                for pkesk in pkesks {
                    for key in &keys {
                        let mut keypair = key.key().clone().into_keypair()
                            .map_err(|e| openpgp::Error::InvalidOperation(format!("keypair failed: {}", e)))?;

                        if let Some((algo, session_key)) = pkesk.decrypt(&mut keypair, sym_algo) {
                            if decrypt(algo, &session_key) {
                                return Ok(Some(key.fingerprint()));
                            }
                        }
                    }
                }

                Err(openpgp::Error::InvalidOperation("decryption failed".to_string()).into())
            }
        }

        impl<'a> VerificationHelper for Helper<'a> {
            fn get_certs(&mut self, _ids: &[KeyHandle]) -> openpgp::Result<Vec<Cert>> {
                Ok(vec![self.cert.clone()])
            }

            fn check(&mut self, _structure: MessageStructure) -> openpgp::Result<()> {
                Ok(())
            }
        }

        let helper = Helper { cert: &self.cert, policy: policy() };
        let mut plaintext = Vec::new();
        let mut decryptor = DecryptorBuilder::from_reader(io::Cursor::new(ciphertext))
            .map_err(|e| CryptoError::Internal(format!("decryptor build failed: {}", e)))?
            .with_policy(policy(), None, helper)
            .map_err(|e| CryptoError::Internal(format!("decryptor policy failed: {}", e)))?;

        io::copy(&mut decryptor, &mut plaintext)
            .map_err(|e| CryptoError::Internal(format!("copy failed: {}", e)))?;

        Ok(plaintext)
    }

    /// Encrypt and sign a message.
    pub fn encrypt_and_sign(&self, recipient_cert: &Cert, plaintext: &[u8]) -> Result<Vec<u8>> {
        let recipients = recipient_cert.keys()
            .with_policy(policy(), None)
            .supported()
            .alive()
            .revoked(false)
            .for_transport_encryption()
            .map(|ka| ka.key())
            .collect::<Vec<_>>();

        if recipients.is_empty() {
            return Err(CryptoError::Internal("no suitable encryption key found".to_string()));
        }

        let signing_keypair = self.cert.keys()
            .with_policy(policy(), None)
            .supported()
            .alive()
            .revoked(false)
            .for_signing()
            .next()
            .ok_or_else(|| CryptoError::Internal("no suitable signing key found".to_string()))?
            .key()
            .clone()
            .parts_into_secret()
            .map_err(|_| CryptoError::Internal("signing key has no secret material".to_string()))?
            .into_keypair()
            .map_err(|e| CryptoError::Internal(format!("failed to create keypair: {}", e)))?;

        let mut sink = Vec::new();
        let message = Message::new(&mut sink);
        let message = Armorer::new(message).build()
            .map_err(|e| CryptoError::Internal(format!("armorer build failed: {}", e)))?;
        let message = Encryptor2::for_recipients(message, recipients)
            .build()
            .map_err(|e| CryptoError::Internal(format!("encryptor build failed: {}", e)))?;
        let message = Signer::new(message, signing_keypair)
            .build()
            .map_err(|e| CryptoError::Internal(format!("signer build failed: {}", e)))?;
        let mut message = LiteralWriter::new(message)
            .build()
            .map_err(|e| CryptoError::Internal(format!("literal writer build failed: {}", e)))?;

        message.write_all(plaintext)
            .map_err(|e| CryptoError::Internal(format!("write failed: {}", e)))?;
        message.finalize()
            .map_err(|e| CryptoError::Internal(format!("finalize failed: {}", e)))?;

        Ok(sink)
    }

    /// Decrypt and verify a message.
    pub fn decrypt_and_verify(&self, sender_cert: &Cert, ciphertext: &[u8]) -> Result<Vec<u8>> {
        struct Helper<'a> {
            decryption_cert: &'a Cert,
            verification_cert: &'a Cert,
            policy: &'static dyn Policy,
        }

        impl<'a> DecryptionHelper for Helper<'a> {
            fn decrypt<D>(
                &mut self,
                pkesks: &[openpgp::packet::PKESK],
                _skesks: &[openpgp::packet::SKESK],
                sym_algo: Option<openpgp::types::SymmetricAlgorithm>,
                mut decrypt: D,
            ) -> openpgp::Result<Option<openpgp::Fingerprint>>
            where
                D: FnMut(openpgp::types::SymmetricAlgorithm, &openpgp::crypto::SessionKey) -> bool,
            {
                let keys: Vec<_> = self.decryption_cert.keys()
                    .with_policy(self.policy, None)
                    .supported()
                    .alive()
                    .revoked(false)
                    .for_transport_encryption()
                    .secret()
                    .collect();

                for pkesk in pkesks {
                    for key in &keys {
                        let mut keypair = key.key().clone().into_keypair()
                            .map_err(|e| openpgp::Error::InvalidOperation(format!("keypair failed: {}", e)))?;

                        if let Some((algo, session_key)) = pkesk.decrypt(&mut keypair, sym_algo) {
                            if decrypt(algo, &session_key) {
                                return Ok(Some(key.fingerprint()));
                            }
                        }
                    }
                }

                Err(openpgp::Error::InvalidOperation("decryption failed".to_string()).into())
            }
        }

        impl<'a> VerificationHelper for Helper<'a> {
            fn get_certs(&mut self, _ids: &[KeyHandle]) -> openpgp::Result<Vec<Cert>> {
                Ok(vec![self.verification_cert.clone()])
            }

            fn check(&mut self, structure: MessageStructure) -> openpgp::Result<()> {
                for layer in structure.into_iter() {
                    match layer {
                        MessageLayer::SignatureGroup { results } => {
                            for result in results {
                                match result {
                                    Ok(_) => return Ok(()),
                                    Err(_) => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(openpgp::Error::InvalidOperation("no valid signatures found".to_string()).into())
            }
        }

        let helper = Helper {
            decryption_cert: &self.cert,
            verification_cert: sender_cert,
            policy: policy(),
        };

        let mut plaintext = Vec::new();
        let mut decryptor = DecryptorBuilder::from_reader(io::Cursor::new(ciphertext))
            .map_err(|e| CryptoError::Internal(format!("decryptor build failed: {}", e)))?
            .with_policy(policy(), None, helper)
            .map_err(|e| CryptoError::Internal(format!("decryptor policy failed: {}", e)))?;

        io::copy(&mut decryptor, &mut plaintext)
            .map_err(|e| CryptoError::Internal(format!("copy failed: {}", e)))?;

        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let keypair = PgpKeyPair::generate("alice@example.com").unwrap();
        assert!(!keypair.fingerprint().is_empty());
    }

    #[test]
    fn test_export_import_public_key() {
        let keypair = PgpKeyPair::generate("alice@example.com").unwrap();
        let exported = keypair.export_public_key().unwrap();
        let imported = PgpKeyPair::from_public_key(&exported).unwrap();
        assert_eq!(keypair.fingerprint(), imported.fingerprint());
    }

    #[test]
    fn test_export_import_secret_key() {
        let keypair = PgpKeyPair::generate("alice@example.com").unwrap();
        let exported = keypair.export_secret_key().unwrap();
        let imported = PgpKeyPair::from_secret_key(&exported).unwrap();
        assert_eq!(keypair.fingerprint(), imported.fingerprint());
    }

    #[test]
    fn test_sign_verify() {
        let keypair = PgpKeyPair::generate("alice@example.com").unwrap();
        let message = b"Hello, World!";
        let signature = keypair.sign(message).unwrap();
        PgpKeyPair::verify(keypair.cert(), message, &signature).unwrap();
    }

    #[test]
    fn test_encrypt_decrypt() {
        let alice = PgpKeyPair::generate("alice@example.com").unwrap();
        let plaintext = b"Secret message";
        let ciphertext = PgpKeyPair::encrypt(alice.cert(), plaintext).unwrap();
        let decrypted = alice.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_encrypt_and_sign_decrypt_and_verify() {
        let alice = PgpKeyPair::generate("alice@example.com").unwrap();
        let bob = PgpKeyPair::generate("bob@example.com").unwrap();
        let plaintext = b"Secret signed message";

        let ciphertext = alice.encrypt_and_sign(bob.cert(), plaintext).unwrap();
        let decrypted = bob.decrypt_and_verify(alice.cert(), &ciphertext).unwrap();

        assert_eq!(plaintext, &decrypted[..]);
    }
}
