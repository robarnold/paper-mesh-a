//! Session-bound reply transport independent of protocol encoding.
//!
//! # Design choices
//!
//! - Keep command handlers independent of the physical transport and executor.
//! - Let each backend select reply storage without requiring allocation in platform.
//!
//! See the [reply transport contract](../../../docs/services/connectivity.md#reply-transport).

use core::future::Future;

/// An identity assigned by a transport to one connection session.
/// Treat this value as opaque outside the backend that issued it.
/// Identities are scoped to that transport, not globally unique.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionId(u32);

impl SessionId {
    /// Wrap a backend identity. The backend must distinguish active and stale sessions.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// A reply transport whose identities distinguish connection sessions.
pub trait ReplyTransport {
    /// Encoded reply storage transferred to the backend without a required copy.
    type Reply: AsRef<[u8]>;

    /// Return whether the original session remains connected.
    /// This observation does not reserve the connection for a later send.
    fn is_session_active(&self, session: SessionId) -> bool;

    /// Send the complete reply to its original session within a bounded wait.
    /// Return true only after transport completion, not merely queue acceptance.
    /// This does not establish that the remote application processed the reply.
    /// Reject stale sessions and never continue a reply into a replacement session.
    /// Dropping the future must invalidate pending writes, including queued packets.
    /// Bytes already transmitted cannot be recalled.
    fn send_reply(&self, session: SessionId, reply: Self::Reply) -> impl Future<Output = bool>;
}
