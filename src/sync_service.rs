use std::{cmp::min, env, str::FromStr, sync::Arc};

use alloy_primitives::Address;
use alloy_rlp::RlpEncodable;
use alloy_sol_types::{private::FixedBytes, SolEventInterface};
use ethers::{
    providers::{Http, Middleware, Provider},
    types::H160,
};
use reth_db::{
    database::Database,
    mdbx::{tx::Tx, RW},
    tables,
    transaction::{DbTx, DbTxMut},
    DatabaseEnv,
};
use tokio::sync::oneshot;
use tracing::info;

use crate::L1MessageQueue::L1MessageQueueEvents;

/*
 * 1. Fetch the last synced block (LSB)
 * 2. Set from from to LSB
 * 3. Set To to Latest confirmed block
 * 4. Fetch Messages/Events from `from` to `to` using jumps of DEFAULT_SIZE
 * 5. Update the LSB after each successful fetch
 * 6.
 */

#[derive(Clone, Debug, RlpEncodable)]
pub struct L1MessageTx {
    queue_index: u64,
    gas: alloy_primitives::Uint<256, 4>,
    to: Address,
    value: alloy_primitives::Uint<256, 4>,
    data: alloy_primitives::Bytes,
    sender: Address,
}

#[derive(Debug)]
pub struct SyncService {
    db: Arc<DatabaseEnv>,
    last_synced_block: Option<u64>,
    provider: Provider<Http>,
}

impl SyncService {
    pub fn new(db: Arc<DatabaseEnv>, provider: Provider<Http>) -> Self {
        let tx = db.tx_mut().expect("Could not create transaction");
        let mut last_synced_block = tx
            .get::<tables::SyncL1LastBlockNumber>("LastSyncedL1BlockNumber".to_string())
            .unwrap();

        // Put the genesis block if the last synced block is None
        info!("Last synced block: {:?}", last_synced_block);
        if last_synced_block.is_none() {
            // last_synced_block = Some(18306000);
            last_synced_block = Some(19972300);
            tx.put::<tables::SyncL1LastBlockNumber>(
                "LastSyncedL1BlockNumber".to_string(),
                last_synced_block.unwrap(),
            )
            .unwrap();
        }
        Self {
            db,
            last_synced_block,
            provider,
        }
    }

    pub async fn start(&self, terminate_rx: oneshot::Receiver<()>) {
        info!("Sync service started");
        let mut tx = self.db.tx_mut().unwrap();
        loop {
            tokio::select! {
                _ = self.fetch_messages(&mut tx) => {
                    break;
                }
                _ = terminate_rx => {
                    info!("Received a message to stop the sync service");
                    break;
                }

            }
        }
        tx.commit().expect("Could not commit transaction");
        info!("Sync service stopped");
    }
    async fn fetch_messages(&self, tx_mut: &mut Tx<RW>) {
        let from = self.last_synced_block.unwrap();
        let to = self.provider.get_block_number().await.unwrap().as_u64();
        info!(
            "-------------------Fetching messages from {} to {}",
            from, to
        );

        for block_number in (from..to).step_by(100) {
            info!("###Block number: {:?}", block_number);
            let (logs, last_queried_block) = self
                .get_filtered_logs(block_number, min(to, block_number + 100))
                .await;
            info!("***Logs: {:?}", logs.len());
            info!("***Block number: {:?}", block_number);

            tx_mut
                .put::<tables::SyncL1LastBlockNumber>(
                    "LastSyncedL1BlockNumber".to_string(),
                    last_queried_block,
                )
                .unwrap();
            for event in logs {
                match event {
                    L1MessageQueueEvents::QueueTransaction(tx) => {
                        let l1_msg_tx = L1MessageTx {
                            queue_index: tx.queueIndex,
                            gas: tx.gasLimit,
                            to: tx.target,
                            value: tx.value,
                            data: tx.data,
                            sender: tx.sender,
                        };

                        let rlp_encoded_l1_msg_tx = alloy_rlp::encode(&l1_msg_tx);
                        let _ = tx_mut.put::<tables::SyncL1MessageQueue>(
                            format!("L1{}", l1_msg_tx.queue_index),
                            rlp_encoded_l1_msg_tx,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    pub async fn get_filtered_logs(&self, from: u64, to: u64) -> (Vec<L1MessageQueueEvents>, u64) {
        let mut filtered_logs = vec![];
        let l1_scroll_messenger = env::var("L1_SCROLL_MESSENGER").unwrap();
        info!("Fetching logs from {} to {}", from, to);
        for block_number in (from..to + 1).step_by(1) {
            info!("Block number: {:?}\n", block_number);
            let receipts = self.provider.get_block_receipts(block_number).await;
            if receipts.is_err() {
                info!(
                    "Error fetching receipts for block number: {:?}",
                    block_number
                );

                return (filtered_logs, block_number - 1);
            }
            let receipts = receipts.unwrap();

            info!(
                "block number: {:?},, receipts length: {:?}",
                block_number,
                receipts.len()
            );

            let new_logs: Vec<L1MessageQueueEvents> = receipts
                .iter()
                .filter(|receipt| receipt.to == Some(H160::from_str(&l1_scroll_messenger).unwrap()))
                .flat_map(|receipt| receipt.logs.iter().map(move |log| (receipt, log)))
                .filter_map(|(_receipt, log)| {
                    let topics: Vec<_> = log
                        .topics
                        .iter()
                        .map(|topic| FixedBytes::new(topic.to_fixed_bytes()))
                        .collect();
                    L1MessageQueueEvents::decode_raw_log(&topics, &log.data, true)
                        .ok()
                        .map(|event| (event))
                })
                .filter(|event| match event {
                    L1MessageQueueEvents::QueueTransaction(_) => true,
                    _ => false,
                })
                .collect();

            if new_logs.len() > 0 {
                info!(
                    "New logs: {:?}, block_number {:?}",
                    new_logs.len(),
                    block_number
                );
                filtered_logs.extend(new_logs);
            }
        }

        return (filtered_logs, to);
    }
}
