use std::{env, path::Path, sync::Arc};

use alloy_sol_types::sol;
use anyhow::Result;
use ethers::providers::Provider;
use reth_db::{
    mdbx::DatabaseArguments,
    models::client_version::ClientVersion,
    test_utils::{ERROR_DB_CREATION, ERROR_TABLE_CREATION},
    DatabaseEnv, DatabaseEnvKind,
};
use rollup_sync_service::RollupSyncService;

mod rollup_sync_service;
mod rollup_sync_service_util;
mod sync_service;

sol!(L1MessageQueue, "l1_message_queue.json");
sol!(ScrollChain, "scroll_chain_abi.json");
use sync_service::SyncService;
use tokio::{signal::ctrl_c, sync::oneshot};
use tracing::info;

fn create_test_db(kind: DatabaseEnvKind, path: &Path) -> Arc<DatabaseEnv> {
    if !path.exists() {
        info!("Database does not exist, creating new one...");
        let env = DatabaseEnv::open(path, kind, DatabaseArguments::new(ClientVersion::default()))
            .expect(ERROR_DB_CREATION);
        env.create_tables().expect(ERROR_TABLE_CREATION);
        Arc::new(env)
    } else {
        info!("Opening existing database...");
        Arc::new(
            DatabaseEnv::open(path, kind, DatabaseArguments::new(ClientVersion::default()))
                .expect("Could not open database at path"),
        )
    }
}
#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    // Opening database at a specific path
    let path = env::current_dir().unwrap().join("scroll-db");
    let db = create_test_db(DatabaseEnvKind::RW, path.as_path());

    let rpc_url = env::var("L1_RPC_URL").unwrap();

    let provider = Provider::try_from(rpc_url).unwrap();

    let (l1_tx, l1_rx) = oneshot::channel();

    let sync_service = SyncService::new(db.clone(), provider.clone());

    let sync_handle = tokio::spawn(async move {
        sync_service.start(l1_rx).await;
    });

    sync_handle.await.expect("Sync service task panicked");

    info!("Sync service has been gracefully shut down.");

    // Now start the rollup sync service in background
    info!("Rollup sync service starting...");

    let rollup_sync_service = RollupSyncService::new(db.clone(), provider.clone());

    let (rollup_tx, rollup_rx) = oneshot::channel();

    let rollup_handle = tokio::spawn(async move {
        rollup_sync_service.start(rollup_rx).await;
    });

    // To prevent the main function from exiting immediately, you can wait for a signal or sleep
    tokio::select! {
        _ = rollup_handle => {
            info!("Rollup sync service has been gracefully shut down.");
        }
        _ = ctrl_c() => {
            info!("Termination signal received. Shutting down.");
            let _ = rollup_tx.send(());
            let _ = l1_tx.send(());
        }
    }
    Ok(())
}
