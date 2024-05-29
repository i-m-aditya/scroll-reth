#![allow(dead_code, unused_imports)]

use std::{
    env,
    fs::File,
    io::{Error, Read, Write},
    ops::Add,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use alloy_primitives::{address, Address, TxHash};

use alloy_sol_types::{private::FixedBytes, sol, SolEventInterface, SolInterface};
use ethers::{
    abi::{Abi, Bytes},
    core::k256::elliptic_curve::rand_core::block,
    providers::{Middleware, Provider},
    types::{Transaction, H160, H256, U64},
};
use reth_db::{
    database::Database,
    mdbx::DatabaseArguments,
    models::client_version::ClientVersion,
    tables,
    test_utils::{ERROR_DB_CREATION, ERROR_TABLE_CREATION, ERROR_TEMPDIR},
    transaction::{DbTx, DbTxMut},
    DatabaseEnv, DatabaseEnvKind,
};
use rollup_sync_service::RollupSyncService;
use rollup_sync_service_util::decode_chunk_block_ranges;

use alloy_rlp::{RlpDecodable, RlpEncodable};
use serde::Deserialize;
use serde_json::Value;

mod rollup_sync_service;
mod rollup_sync_service_util;
mod sync_service;

sol!(L1MessageQueue, "l1_message_queue.json");
sol!(ScrollChain, "scroll_chain_abi.json");
use sync_service::SyncService;
use tokio::{signal::ctrl_c, sync::mpsc};
use L1MessageQueue::{L1MessageQueueCalls, L1MessageQueueEvents};
use ScrollChain::{ScrollChainCalls, ScrollChainEvents};

fn create_test_db(kind: DatabaseEnvKind, path: &Path) -> Arc<DatabaseEnv> {
    if !path.exists() {
        println!("Database does not exist, creating new one...");
        let env = DatabaseEnv::open(path, kind, DatabaseArguments::new(ClientVersion::default()))
            .expect(ERROR_DB_CREATION);
        env.create_tables().expect(ERROR_TABLE_CREATION);
        Arc::new(env)
    } else {
        println!("Opening existing database...");
        Arc::new(
            DatabaseEnv::open(path, kind, DatabaseArguments::new(ClientVersion::default()))
                .expect("Could not open database at path"),
        )
    }
}
#[tokio::main]
async fn main() {
    println!("Hello, world!");
    // Opening database at a specific path
    let path = env::current_dir().unwrap().join("scroll-db");
    let db = create_test_db(DatabaseEnvKind::RW, path.as_path());
    // let tx_mut = db.tx_mut().expect("Could not create transaction");
    println!("Database opened successfully");
    /*
     * Start the sync service
     * initialize the sync service
     * and then start
     */

    let provider = Provider::try_from(
        "https://black-proportionate-haze.quiknode.pro/5dc77552bcfa565227baa9701c48da45f8b37b34/"
            .to_string(),
    )
    .unwrap();

    let (l1_tx, mut l1_rx) = mpsc::channel(1);

    // println!("Here");
    let sync_service = SyncService::new(db.clone(), provider.clone());

    let sync_handle = tokio::spawn(async move {
        sync_service.start(&mut l1_rx).await;
    });

    // println!("Hello");
    // // println!("Sync Service: {:?}", sync_service);

    // // TODO be included later
    // // ctrl_c()
    // //     .await
    // //     .expect("Failed to listen for termination signal");

    // // tx.send(())
    // //     .await
    // //     .expect("Failed to send termination signal");

    sync_handle.await.expect("Sync service task panicked");

    println!("Sync service has been gracefully shut down.");

    // Now start the rollup sync service in background
    println!("Rollup sync service starting...");

    let rollup_sync_service = RollupSyncService::new(db.clone(), provider.clone());

    let (rollup_tx, mut rollup_rx) = mpsc::channel(1);

    let rollup_handle = tokio::spawn(async move {
        rollup_sync_service.start(&mut rollup_rx).await;
    });

    // To prevent the main function from exiting immediately, you can wait for a signal or sleep
    tokio::select! {
        _ = rollup_handle => {
            println!("Rollup sync service has been gracefully shut down.");
        }
        _ = ctrl_c() => {
            println!("Termination signal received. Shutting down.");
            rollup_tx.send(()).await.expect("Failed to send termination signal to rollup sync service");
        }
    }

    // ctrl_c()
    //     .await
    //     .expect("Failed to listen for termination signal");
    // tx.send(())
    //     .await
    //     .expect("Failed to send termination signal");
}
