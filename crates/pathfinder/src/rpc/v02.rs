use std::sync::Arc;

use super::error::RpcError;
use crate::{core::Chain, state::SyncState};
use crate::{state::PendingData, storage::Storage};

pub mod method;
pub mod types;

#[derive(Clone)]
pub struct RpcContext {
    pub storage: Storage,
    pub pending_data: Option<PendingData>,
    pub sync_status: Arc<SyncState>,
    pub chain: Chain,
}

impl RpcContext {
    pub fn new(storage: Storage, sync_status: Arc<SyncState>, chain: Chain) -> Self {
        Self {
            storage,
            sync_status,
            chain,
            pending_data: None,
        }
    }

    #[cfg(test)]
    pub fn for_tests() -> Self {
        let storage = super::tests::setup_storage();
        let sync_state = Arc::new(SyncState::default());
        Self::new(storage, sync_state, Chain::Testnet)
    }

    pub fn with_pending_data(self, pending_data: PendingData) -> Self {
        Self {
            pending_data: Some(pending_data),
            ..self
        }
    }

    #[cfg(test)]
    pub async fn for_tests_with_pending() -> Self {
        // This is a bit silly with the arc in and out, but since its for tests the ergonomics of
        // having Arc also constructed is nice.
        let context = Self::for_tests();
        let pending_data = super::tests::create_pending_data(context.storage.clone()).await;
        context.with_pending_data(pending_data)
    }
}

/// Registers a JSON-RPC method with the [RpcModule<RpcContext>](jsonrpsee::RpcModule).
///
/// An example signature for `method` is:
/// ```ignore
/// async fn method(context: Arc<RpcContext>, input: Input) -> Result<Ouput, Error>
/// ```
#[allow(dead_code)]
fn register_method<Input, Output, Error, MethodFuture, Method>(
    module: &mut jsonrpsee::RpcModule<RpcContext>,
    method_name: &'static str,
    method: Method,
) -> anyhow::Result<()>
where
    Input: ::serde::de::DeserializeOwned + Send + Sync,
    Output: 'static + ::serde::Serialize + Send + Sync,
    Error: Into<RpcError>,
    MethodFuture: std::future::Future<Output = Result<Output, Error>> + Send,
    Method: (Fn(RpcContext, Input) -> MethodFuture) + Copy + Send + Sync + 'static,
{
    use anyhow::Context;
    use jsonrpsee::types::Params;
    use tracing::Instrument;

    metrics::register_counter!("rpc_method_calls_total", "method" => method_name);

    let method_callback = move |params: Params<'static>, context: Arc<RpcContext>| {
        // why info here? it's the same used in warp tracing filter for example.
        let span = tracing::info_span!("rpc_method", name = method_name);
        async move {
            let input = params.parse::<Input>()?;
            method((*context).clone(), input).await.map_err(|err| {
                let rpc_err: RpcError = err.into();
                jsonrpsee::core::Error::from(rpc_err)
            })
        }
        .instrument(span)
    };

    module
        .register_async_method(method_name, method_callback)
        .with_context(|| format!("Registering {method_name}"))?;

    Ok(())
}

/// Registers a JSON-RPC method with the [RpcModule<RpcContext>](jsonrpsee::RpcModule).
///
/// An example signature for `method` is:
/// ```ignore
/// async fn method(context: Arc<RpcContext>) -> Result<Ouput, Error>
/// ```
#[allow(dead_code)]
fn register_method_with_no_input<Output, Error, MethodFuture, Method>(
    module: &mut jsonrpsee::RpcModule<RpcContext>,
    method_name: &'static str,
    method: Method,
) -> anyhow::Result<()>
where
    Output: 'static + ::serde::Serialize + Send + Sync,
    Error: Into<RpcError>,
    MethodFuture: std::future::Future<Output = Result<Output, Error>> + Send,
    Method: (Fn(RpcContext) -> MethodFuture) + Copy + Send + Sync + 'static,
{
    use anyhow::Context;
    use tracing::Instrument;

    metrics::register_counter!("rpc_method_calls_total", "method" => method_name);

    let method_callback = move |_params, context: Arc<RpcContext>| {
        // why info here? it's the same used in warp tracing filter for example.
        let span = tracing::info_span!("rpc_method", name = method_name);
        async move {
            method((*context).clone()).await.map_err(|err| {
                let rpc_err: RpcError = err.into();
                jsonrpsee::core::Error::from(rpc_err)
            })
        }
        .instrument(span)
    };

    module
        .register_async_method(method_name, method_callback)
        .with_context(|| format!("Registering {method_name}"))?;

    Ok(())
}
