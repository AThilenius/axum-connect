use bytes::{Buf, BufMut};

use crate::prelude::RpcError;

/// Wrap Prost traits in our own, so I can start decoupling Prost because it's driving me insane.
pub trait Message: prost::Message {
    const TYPE_URL: &'static str;

    fn encode<B>(&self, buf: &mut B)
    where
        B: BufMut,
        Self: Sized;

    fn decode<B>(buf: B) -> Result<Self, RpcError>
    where
        B: Buf,
        Self: Default;
}
