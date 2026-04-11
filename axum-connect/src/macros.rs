macro_rules! implement_new {
    ($code:ident) => {
        ::paste::paste! {
            pub fn [<new_ $code:snake>](message: String) -> Self {
                Self {
                    code: RpcErrorCode::$code,
                    message: message,
                    details: vec![],
                }
            }
        }
    };
}
