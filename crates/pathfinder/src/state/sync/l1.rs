use pathfinder_ethereum::{L1StateUpdate, StarknetEthereumClient};
use tokio::sync::mpsc::Sender;

/// Syncs L1 state updates.
pub async fn sync(
    tx_event: Sender<L1StateUpdate>,
    ethereum_client: StarknetEthereumClient,
    start_delay: std::time::Duration,
    poll_interval: std::time::Duration,
) -> anyhow::Result<()> {
    tokio::time::sleep(start_delay).await;

    loop {
        tokio::time::sleep(poll_interval).await;

        match ethereum_client.get_starknet_state().await {
            Ok(state) => tx_event.send(state).await?,
            Err(e) => tracing::error!("L1 call failed: {e:?}"),
        }
    }
}
