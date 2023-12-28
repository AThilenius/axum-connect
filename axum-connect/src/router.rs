use axum::Router;

pub trait RpcRouterExt<S>: Sized {
    fn rpc<F>(self, register: F) -> Self
    where
        F: FnOnce(Self) -> RpcRouter<S>;
}

impl<S> RpcRouterExt<S> for Router<S> {
    fn rpc<F>(self, register: F) -> Self
    where
        F: FnOnce(Self) -> RpcRouter<S>,
    {
        register(self)
    }
}

pub type RpcRouter<S> = Router<S>;
