use super::{OverlayError, OverlayResult};
use cryptochat_messaging::EncryptedEnvelope;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum OverlayNotification {
    EnvelopeReceived(EncryptedEnvelope),
    ReceiptAcknowledged(String),
    DiscoveryUpdate(String),
}

#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct SubscriptionManager {
    subscribers: Arc<Mutex<Vec<usize>>>,
}

impl SubscriptionManager {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn register(&self) -> OverlayResult<usize> {
        let mut subs = self
            .subscribers
            .lock()
            .map_err(|_| OverlayError::Subscription("lock poisoned".into()))?;
        let id = subs.len();
        subs.push(id);
        Ok(id)
    }

    #[allow(dead_code)]
    pub fn notify(&self, _event: OverlayNotification) -> OverlayResult<()> {
        Err(OverlayError::NotImplemented)
    }
}
