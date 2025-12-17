use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use cryptochat_messaging::EncryptedEnvelope;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct NodeStorage {
    db: sled::Db,
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredEnvelope {
    envelope: EncryptedEnvelope,
    pending_peers: Vec<String>,
    acked_peers: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredInbound {
    envelope: EncryptedEnvelope,
    stored_ms: i64,
}

#[derive(Clone)]
pub struct PendingEnvelope {
    pub message_id: String,
    pub envelope: EncryptedEnvelope,
    pub pending_peers: Vec<PeerId>,
}

impl NodeStorage {
    const TREE: &'static str = "replication";
    const INBOUND_TREE: &'static str = "inbound";

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        std::fs::create_dir_all(path)
            .with_context(|| format!("failed to create storage directory {:?}", path))?;
        let db = sled::open(path)
            .with_context(|| format!("failed to open sled database at {:?}", path))?;
        Ok(Self { db })
    }

    fn tree(&self) -> sled::Result<sled::Tree> {
        self.db.open_tree(Self::TREE)
    }

    fn inbound_tree(&self) -> sled::Result<sled::Tree> {
        self.db.open_tree(Self::INBOUND_TREE)
    }

    pub fn insert_outbound(
        &self,
        message_id: &str,
        envelope: &EncryptedEnvelope,
        peers: &[PeerId],
    ) -> Result<()> {
        let tree = self.tree()?;
        let key = message_id.as_bytes();
        let mut record = if let Some(existing) = tree.get(key)? {
            let mut stored: StoredEnvelope = bincode::deserialize(&existing)?;
            stored.envelope = envelope.clone();
            stored
        } else {
            StoredEnvelope {
                envelope: envelope.clone(),
                pending_peers: Vec::new(),
                acked_peers: Vec::new(),
            }
        };

        record.pending_peers = peers.iter().map(|p| p.to_string()).collect();
        record.pending_peers.sort();
        record.pending_peers.dedup();

        let encoded = bincode::serialize(&record)?;
        tree.insert(key, encoded)?;
        tree.flush()?;
        Ok(())
    }

    pub fn mark_peer_success(&self, message_id: &str, peer: &PeerId) -> Result<bool> {
        let tree = self.tree()?;
        let key = message_id.as_bytes();
        let Some(existing) = tree.get(key)? else {
            return Ok(true);
        };

        let mut record: StoredEnvelope = bincode::deserialize(&existing)?;
        let peer_str = peer.to_string();
        record.pending_peers.retain(|p| p != &peer_str);
        if !record.acked_peers.contains(&peer_str) {
            record.acked_peers.push(peer_str);
        }

        if record.pending_peers.is_empty() {
            tree.remove(key)?;
        } else {
            let encoded = bincode::serialize(&record)?;
            tree.insert(key, encoded)?;
        }
        tree.flush()?;
        Ok(record.pending_peers.is_empty())
    }

    pub fn load_pending(&self) -> Result<Vec<PendingEnvelope>> {
        let tree = self.tree()?;
        let mut pending = Vec::new();
        for entry in tree.iter() {
            let (key, value) = entry?;
            let message_id =
                String::from_utf8(key.to_vec()).context("stored key was not valid UTF-8")?;
            let record: StoredEnvelope = bincode::deserialize(&value)?;
            let mut peers = Vec::new();
            for peer_str in &record.pending_peers {
                if let Ok(peer_id) = PeerId::from_str(peer_str) {
                    peers.push(peer_id);
                }
            }
            if peers.is_empty() {
                continue;
            }
            pending.push(PendingEnvelope {
                message_id,
                envelope: record.envelope.clone(),
                pending_peers: peers,
            });
        }
        Ok(pending)
    }

    pub fn store_inbound(&self, envelope: &EncryptedEnvelope) -> Result<()> {
        let tree = self.inbound_tree()?;
        let key = envelope.message_id.to_string();

        let stored_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let record = StoredInbound {
            envelope: envelope.clone(),
            stored_ms,
        };

        let encoded = bincode::serialize(&record)?;
        tree.insert(key.as_bytes(), encoded)?;
        tree.flush()?;
        Ok(())
    }
}
