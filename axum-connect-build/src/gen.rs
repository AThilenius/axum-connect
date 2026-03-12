use proc_macro2::TokenStream;
use prost_build::{Method, Service, ServiceGenerator};
use quote::{format_ident, quote};
use syn::parse_str;

pub struct AxumConnectServiceGenerator {
    generate_server: bool,
    generate_client: bool,
}

impl AxumConnectServiceGenerator {
    pub fn new(server: bool, client: bool) -> Self {
        Self {
            generate_server: server,
            generate_client: client,
        }
    }

    fn generate_service(&mut self, service: Service, buf: &mut String) {
        let path_root = format!("{}.{}", service.package, service.proto_name);

        if self.generate_server {
            self.generate_server_service(&service, &path_root, buf);
        }
    }

    // ── Server codegen ──────────────────────────────────────────────

    fn generate_server_service(
        &mut self,
        service: &Service,
        path_root: &str,
        buf: &mut String,
    ) {
        let service_name = format_ident!("{}", service.name);
        let methods = service.methods.iter().filter_map(|m| {
            // Don't currently support client streaming. Will-do soon.
            if m.client_streaming {
                return None;
            }

            Some(self.generate_server_method(m, path_root))
        });

        buf.push_str(
            quote! {
                pub struct #service_name;

                #[allow(dead_code)]
                impl #service_name {
                    #(#methods)*
                }
            }
            .to_string()
            .as_str(),
        );
    }

    fn generate_server_method(&mut self, method: &Method, path_root: &str) -> TokenStream {
        let method_name = format_ident!("{}", method.name);
        let method_name_unary_get = format_ident!("{}_unary_get", method.name);
        let input_type: syn::Type = parse_str(&method.input_type).unwrap();
        let output_type: syn::Type = parse_str(&method.output_type).unwrap();
        let path = format!("/{}/{}", path_root, method.proto_name);

        if method.server_streaming {
            quote! {
                pub fn #method_name<T, H, S>(
                    handler: H
                ) -> impl FnOnce(axum::Router<S>) -> axum_connect::router::RpcRouter<S>
                where
                    H: axum_connect::handler::RpcHandlerStream<#input_type, #output_type, T, S>,
                    T: 'static,
                    S: Clone + Send + Sync + 'static,
                {
                    move |router: axum::Router<S>| {
                        router.route(
                            #path,
                            axum::routing::post(|
                                axum::extract::State(state): axum::extract::State<S>,
                                request: axum::http::Request<axum::body::Body>
                            | async move {
                                handler.call(request, state).await
                            }),
                        )
                    }
                }
            }
        } else {
            quote! {
                pub fn #method_name<T, H, S>(
                    handler: H
                ) -> impl FnOnce(axum::Router<S>) -> axum_connect::router::RpcRouter<S>
                where
                    H: axum_connect::handler::RpcHandlerUnary<#input_type, #output_type, T, S>,
                    T: 'static,
                    S: Clone + Send + Sync + 'static,
                {
                    move |router: axum::Router<S>| {
                        router.route(
                            #path,
                            axum::routing::post(|
                                axum::extract::State(state): axum::extract::State<S>,
                                request: axum::http::Request<axum::body::Body>
                            | async move {
                                handler.call(request, state).await
                            }),
                        )
                    }
                }

                pub fn #method_name_unary_get<T, H, S>(
                    handler: H
                ) -> impl FnOnce(axum::Router<S>) -> axum_connect::router::RpcRouter<S>
                where
                    H: axum_connect::handler::RpcHandlerUnary<#input_type, #output_type, T, S>,
                    T: 'static,
                    S: Clone + Send + Sync + 'static,
                {
                    move |router: axum::Router<S>| {
                        router.route(
                            #path,
                            axum::routing::get(|
                                axum::extract::State(state): axum::extract::State<S>,
                                request: axum::http::Request<axum::body::Body>
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
