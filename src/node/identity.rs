use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identity for a HIVEFUZZ node.
///
/// Each node generates a UUID and an Ed25519 keypair at startup.
/// The keypair is used to sign gossip messages, preventing rogue
/// nodes from injecting malicious data.
#[derive(Debug)]
pub struct NodeIdentity {
    /// Unique node identifier.
    pub id: Uuid,
    /// Ed25519 signing keypair.
    pub keypair: ed25519_dalek::SigningKey,
}

/// Serializable public identity (shared with peers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicIdentity {
    pub id: Uuid,
    pub public_key: Vec<u8>,
}

impl NodeIdentity {
    /// Generate a new random node identity.
    pub fn generate() -> Self {
        let mut secret = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut secret);
        let keypair = ed25519_dalek::SigningKey::from_bytes(&secret);

        Self {
            id: Uuid::new_v4(),
            keypair,
        }
    }

    /// Get the public identity (safe to share with peers).
    pub fn public_identity(&self) -> PublicIdentity {
        use ed25519_dalek::Signer;
        let verifying_key = self.keypair.verifying_key();

        PublicIdentity {
            id: self.id,
            public_key: verifying_key.as_bytes().to_vec(),
        }
    }

    /// Sign a message with this node's private key.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        use ed25519_dalek::Signer;
        let signature = self.keypair.sign(message);
        signature.to_bytes().to_vec()
    }
}

/// Verify a signature from a peer.
pub fn verify_signature(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> bool {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let Ok(key_bytes): Result<[u8; 32], _> = public_key.try_into() else {
        return false;
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&key_bytes) else {
        return false;
    };
    let Ok(sig_bytes): Result<[u8; 64], _> = signature.try_into() else {
        return false;
    };
    let sig = Signature::from_bytes(&sig_bytes);

    verifying_key.verify(message, &sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generation() {
        let id1 = NodeIdentity::generate();
        let id2 = NodeIdentity::generate();
        assert_ne!(id1.id, id2.id);
    }

    #[test]
    fn test_sign_and_verify() {
        let identity = NodeIdentity::generate();
        let message = b"hello hivefuzz swarm";

        let signature = identity.sign(message);
        let public = identity.public_identity();

        assert!(verify_signature(&public.public_key, message, &signature));
        assert!(!verify_signature(&public.public_key, b"tampered", &signature));
    }
}
