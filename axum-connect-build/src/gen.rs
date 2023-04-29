use proc_macro2::TokenStream;
use prost_build::{Method, Service, ServiceGenerator};
use quote::{format_ident, quote};
use syn::parse_str;

#[derive(Default)]
pub struct AxumConnectServiceGenerator {}

impl AxumConnectServiceGenerator {
    pub fn new() -> Self {
        Default::default()
    }

    fn generate_service(&mut self, service: Service, buf: &mut String) {
        // Service struct
        let service_name = format_ident!("{}", service.name);
        let methods = service.methods.into_iter().map(|m| {
            self.generate_service_method(m, &format!("{}.{}", service.package, service.proto_name))
        });

        buf.push_str(
            quote! {
                pub struct #service_name;

                impl #service_name {
                    #(#methods)*
                }
            }
            .to_string()
            .as_str(),
        );
    }

    fn generate_service_method(&mut self, method: Method, path_root: &str) -> TokenStream {
        let method_name = format_ident!("{}", method.name);
        let input_type: syn::Type = parse_str(&method.input_type).unwrap();
        let output_type: syn::Type = parse_str(&method.output_type).unwrap();
        let path = format!("/{}/{}", path_root, method.proto_name);

        quote! {
            pub fn #method_name<T, H, R, S, B>(
                handler: H
            ) -> impl FnOnce(axum::Router<S, B>) -> axum_connect::router::RpcRouter<S, B>
            where
                H: axum_connect::handler::HandlerFuture<#input_type, #output_type, R, T, S, B>,
                T: 'static,
                S: Clone + Send + Sync + 'static,
                B: axum::body::HttpBody + Send + 'static,
                B::Data: Send,
                B::Error: Into<axum::BoxError>,
            {
                use axum::response::IntoResponse;

                move |router: axum::Router<S, B>| {
                    router.route(
                        #path,
                        axum::routing::post(|axum::extract::State(state): axum::extract::State<S>, request: axum::http::Request<B>| async move {
                            let res = handler.call(request, state).await;
                            res.into_response()
                        }),
                    )
                }
            }
        }
    }
}

impl ServiceGenerator for AxumConnectServiceGenerator {
    fn generate(&mut self, service: Service, buf: &mut String) {
        self.generate_service(service, buf);
    }

    fn finalize(&mut self, buf: &mut String) {
        // Add serde import (because that's less effort than hacking pbjson).
        buf.push_str("\nuse axum_connect::serde;\n");
    }
}
