use std::net::SocketAddrV4;

use async_trait::async_trait;

pub(crate) enum Access {
    Socket(SocketAddrV4),
    Loading,
}

impl From<SocketAddrV4> for Access {
    fn from(value: SocketAddrV4) -> Self {
        Self::Socket(value)
    }
}

#[async_trait]
pub(crate) trait Listener: Send {
    async fn access(&self) -> anyhow::Result<Access>;
    fn is_public(&self) -> bool;
}
