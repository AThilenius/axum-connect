use axum::Router;

pub trait RpcRouterExt<S, B>: Sized {
    fn rpc<F>(self, register: F) -> Self
    where
        F: FnOnce(Self) -> RpcRouter<S, B>;
}

impl<S, B> RpcRouterExt<S, B> for Router<S, B> {
    fn rpc<F>(self, register: F) -> Self
    where
        F: FnOnce(Self) -> RpcRouter<S, B>,
    {
        register(self)
        // unsafe { std::mem::transmute::<RpcRouter<S, B>, Router<S, B>>(register(self)) }
    }
}

pub type RpcRouter<S, B> = Router<S, B>;
