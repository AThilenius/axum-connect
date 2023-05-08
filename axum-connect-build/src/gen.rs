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
        let methods =
            service.methods.into_iter().filter_map(|m| {
                // Don't currently support client streaming. Will-do soon.
                if m.client_streaming {
                    return None;
                }

                Some(self.generate_service_method(
                    m,
                    &format!("{}.{}", service.package, service.proto_name),
                ))
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

        if method.server_streaming {
            quote! {
                pub fn #method_name<T, H, S, B>(
                    handler: H
                ) -> impl FnOnce(axum::Router<S, B>) -> axum_connect::router::RpcRouter<S, B>
                where
                    H: axum_connect::handler::RpcHandlerStream<#input_type, #output_type, T, S, B>,
                    T: 'static,
                    S: Clone + Send + Sync + 'static,
                    B: axum::body::HttpBody + Send + 'static,
                    B::Data: Send,
                    B::Error: Into<axum::BoxError>,
                {
                    move |router: axum::Router<S, B>| {
                        router.route(
                            #path,
                            axum::routing::post(|
                                axum::extract::State(state): axum::extract::State<S>,
                                request: axum::http::Request<B>
                            | async move {
                                handler.call(request, state).await
                            }),
                        )
                    }
                }
            }
        } else {
            quote! {
                pub fn #method_name<T, H, S, B>(
                    handler: H
                ) -> impl FnOnce(axum::Router<S, B>) -> axum_connect::router::RpcRouter<S, B>
                where
                    H: axum_connect::handler::RpcHandlerUnary<#input_type, #output_type, T, S, B>,
                    T: 'static,
                    S: Clone + Send + Sync + 'static,
                    B: axum::body::HttpBody + Send + 'static,
                    B::Data: Send,
                    B::Error: Into<axum::BoxError>,
                {
                    move |router: axum::Router<S, B>| {
                        router.route(
                            #path,
                            axum::routing::post(|
                                axum::extract::State(state): axum::extract::State<S>,
                                request: axum::http::Request<B>
                            | async move {
                                handler.call(request, state).await
                            }),
                        )
                    }
                }
            }
        }
    }
}

impl ServiceGenerator for AxumConnectServiceGenerator {
    fn generate(&mut self, service: Service, buf: &mut String) {
        self.generate_service(service, buf);
    }
}
