//! Message request system for one-way contact initiation.
//!
//! This module enables users to receive messages from contacts they haven't
//! explicitly added yet. Recipients can review and accept/reject requests.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{ConversationId, DeviceId};

/// Status of a message request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequestStatus {
    /// Request is pending review by recipient
    Pending,
    /// Request was accepted by recipient
    Accepted,
    /// Request was rejected/blocked by recipient
    Rejected,
}

/// A message request from an unknown contact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRequest {
    /// Unique ID for this request
    pub request_id: Uuid,

    /// The conversation this request initiated
    pub conversation_id: ConversationId,

    /// PGP fingerprint of the requester
    pub sender_fingerprint: String,

    /// Requester's device ID
    pub sender_device: DeviceId,

    /// Requester's public key (ASCII-armored PGP)
    pub sender_public_key: String,

    /// When the request was created
    pub created_ms: i64,

    /// Current status of the request
    pub status: RequestStatus,

    /// When the status was last updated
    pub status_updated_ms: i64,

    /// Preview of the first message (encrypted)
    pub first_message_preview: Option<String>,
}

impl MessageRequest {
    /// Create a new pending message request
    pub fn new(
        conversation_id: ConversationId,
        sender_fingerprint: String,
        sender_device: DeviceId,
        sender_public_key: String,
        first_message_preview: Option<String>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Self {
            request_id: Uuid::new_v4(),
            conversation_id,
            sender_fingerprint,
            sender_device,
            sender_public_key,
            created_ms: now,
            status: RequestStatus::Pending,
            status_updated_ms: now,
            first_message_preview,
        }
    }

    /// Accept the message request
    pub fn accept(&mut self) {
        self.status = RequestStatus::Accepted;
        self.status_updated_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
    }

    /// Reject/block the message request
    pub fn reject(&mut self) {
        self.status = RequestStatus::Rejected;
        self.status_updated_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
    }

    /// Check if this request is still pending
    pub fn is_pending(&self) -> bool {
        self.status == RequestStatus::Pending
    }
}

/// Contact entry created after accepting a message request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    /// PGP fingerprint (used as primary key)
    pub fingerprint: String,

    /// Public key (ASCII-armored)
    pub public_key: String,

    /// Optional display name
    pub display_name: Option<String>,

    /// When this contact was added
    pub added_ms: i64,

    /// Last conversation with this contact
    pub last_conversation_id: Option<ConversationId>,
}

impl Contact {
    /// Create a new contact from an accepted message request
    pub fn from_request(request: &MessageRequest) -> Self {
        Self {
            fingerprint: request.sender_fingerprint.clone(),
            public_key: request.sender_public_key.clone(),
            display_name: None,
            added_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
            last_conversation_id: Some(request.conversation_id.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_lifecycle() {
        let conv_id = ConversationId::new();
        let device_id = DeviceId::new();

        let mut request = MessageRequest::new(
            conv_id,
            "ABC123".to_string(),
            device_id,
            "-----BEGIN PGP PUBLIC KEY BLOCK-----".to_string(),
            Some("Hello!".to_string()),
        );

        assert!(request.is_pending());

        request.accept();
        assert_eq!(request.status, RequestStatus::Accepted);
        assert!(!request.is_pending());
    }

    #[test]
    fn test_contact_from_request() {
        let conv_id = ConversationId::new();
        let device_id = DeviceId::new();

        let request = MessageRequest::new(
            conv_id.clone(),
            "ABC123".to_string(),
            device_id,
            "-----BEGIN PGP PUBLIC KEY BLOCK-----".to_string(),
            None,
        );

        let contact = Contact::from_request(&request);
        assert_eq!(contact.fingerprint, "ABC123");
        assert_eq!(contact.last_conversation_id, Some(conv_id));
    }
}
