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

// #![allow(dead_code, unused_imports)]

// use std::{
//     env,
//     fs::File,
//     io::{Error, Read, Write},
//     ops::Add,
//     path::{Path, PathBuf},
//     str::FromStr,
//     sync::Arc,
// };

// use alloy_primitives::{address, Address, TxHash};

// use alloy_sol_types::{private::FixedBytes, sol, SolEventInterface, SolInterface};
// use ethers::{
//     abi::{Abi, Bytes},
//     core::k256::elliptic_curve::rand_core::block,
//     providers::{Middleware, Provider},
//     types::{Transaction, H160, H256, U64},
// };
// use reth_db::{
//     database::Database,
//     mdbx::DatabaseArguments,
//     models::client_version::ClientVersion,
//     tables,
//     test_utils::{ERROR_DB_CREATION, ERROR_TABLE_CREATION, ERROR_TEMPDIR},
//     transaction::{DbTx, DbTxMut},
//     DatabaseEnv, DatabaseEnvKind,
// };
// use rollup_sync_service::decode_chunk_block_ranges;

// use alloy_rlp::{RlpDecodable, RlpEncodable};
// use serde::Deserialize;
// use serde_json::Value;

// mod rollup_sync_service;

// sol!(L1MessageQueue, "l1_message_queue.json");
// use L1MessageQueue::{L1MessageQueueCalls, L1MessageQueueEvents};
// ``
// #[derive(Clone, Debug, RlpEncodable)]
// pub struct L1MessageTx {
//     queue_index: u64,
//     gas: alloy_primitives::Uint<256, 4>,
//     to: Address,
//     value: alloy_primitives::Uint<256, 4>,
//     data: Vec<u8>,
//     sender: Address,
// }

// fn create_test_db(kind: DatabaseEnvKind, path: &Path) -> Arc<DatabaseEnv> {
//     if !path.exists() {
//         println!("Database does not exist, creating new one...");
//         let env = DatabaseEnv::open(path, kind, DatabaseArguments::new(ClientVersion::default()))
//             .expect(ERROR_DB_CREATION);
//         env.create_tables().expect(ERROR_TABLE_CREATION);
//         Arc::new(env)
//     } else {
//         println!("Opening existing database...");
//         Arc::new(
//             DatabaseEnv::open(path, kind, DatabaseArguments::new(ClientVersion::default()))
//                 .expect("Could not open database at path"),
//         )
//     }
// }

// // Define your struct to represent the ABI method inputs
// #[derive(Debug, Deserialize)]
// struct CommitBatchArgs {
//     version: u8,
//     parent_batch_header: Bytes,
//     chunks: Vec<Bytes>,
//     skipped_l1_message_bitmap: Bytes,
// }
// #[tokio::main]
// async fn main() {
//     /***
//      * Goal is to get a scroll contract transaction and then decode its tx data into chunksBlockRanges
//      */
//     // let provider = Provider::try_from(
//     //     "https://black-proportionate-haze.quiknode.pro/5dc77552bcfa565227baa9701c48da45f8b37b34/"
//     //         .to_string(),
//     // )
//     // .unwrap();

//     // let tx_hash =
//     //     H256::from_str("0x079bfd5ef1393f326fb389908366de8fe656730ee934465150865c64dddaff13")
//     //         .unwrap();
//     // let tx = provider.get_transaction(tx_hash).await.unwrap().unwrap();

//     // // let input_data = tx.input.to_vec();

//     // let mut file = File::open("scroll_chain_abi.json").unwrap();

//     // let mut abi_json = String::new();
//     // file.read_to_string(&mut abi_json).unwrap();

//     // let parsed_json: Value = serde_json::from_str(&abi_json).unwrap();

//     // let abi_array = parsed_json["abi"].clone();
//     // // Parse the ABI
//     // let abi: Abi = serde_json::from_value(abi_array).unwrap();

//     // // Example tx_data (replace with actual data)
//     // let tx_data = tx.input.to_vec();

//     // // Decode the tx_data
//     // match decode_chunk_block_ranges(tx_data, &abi) {
//     //     Ok(chunk_block_ranges) => {
//     //         println!("Decoded chunk block ranges: {:?}", chunk_block_ranges);
//     //     }
//     //     Err(e) => {
//     //         eprintln!("Failed to decode chunk block ranges: {}", e);
//     //     }
//     // }
//     // *******************************************************************
//     // *******************************************************************
//     // *******************************************************************
//     // *******************************************************************
//     // *******************************************************************
//     // *******************************************************************
//     // *******************************************************************
//     // *******************************************************************
//     // *******************************************************************
//     // *******************************************************************
//     // // let db = create_test_db(DatabaseEnvKind::RW);
//     let path = env::current_dir().unwrap().join("scroll-db");
//     let db = create_test_db(DatabaseEnvKind::RW, path.as_path());
//     // // create_test_db_with_path(kind, path)
//     let tx = db.tx_mut().expect("Failed to get tx");

//     // // let _ = tx.put::<tables::TransactionBlocks>(1, 2).unwrap();
//     // // tx.commit().expect("Failed to commit");

//     // let insert_result = tx.get::<tables::TransactionBlocks>(1);

//     // match insert_result {
//     //     Ok(Some(value)) => {
//     //         println!("Success: {:?}", value);
//     //     }
//     //     Ok(None) => {
//     //         println!("No value found");
//     //     }
//     //     Err(e) => {
//     //         println!("Error: {:?}", e);
//     //     }
//     // }

//     let provider: Provider<ethers::providers::Http> = Provider::try_from(
//         "https://black-proportionate-haze.quiknode.pro/5dc77552bcfa565227baa9701c48da45f8b37b34/"
//             .to_string(),
//     )
//     .unwrap();

//     // let block_number = provider.get_block_number().await.unwrap();
//     let block_number: ethers::types::U64 = U64::from(19959319);

//     // // // get the blocks using reth at this block number
//     // // let block = provider.get_block(block_number).await.unwrap().unwrap();

//     // // let block_with_transaction = provider
//     // //     .get_block_with_txs(block_number)
//     // //     .await
//     // //     .unwrap()
//     // //     .unwrap();

//     let receipts: Vec<ethers::types::TransactionReceipt> =
//         provider.get_block_receipts(block_number).await.unwrap();

//     println!("receipts length: {:?}", receipts.len());

//     let filtered_logs: Vec<(U64, L1MessageQueueEvents)> = receipts
//         .iter()
//         .filter(|receipt| {
//             receipt.to
//                 == Some(H160::from_str("0x6774Bcbd5ceCeF1336b5300fb5186a12DDD8b367").unwrap())
//         })
//         .flat_map(|receipt| {
//             println!("Hello ");
//             receipt.logs.iter().map(move |log| (receipt, log))
//         })
//         .filter_map(|(receipt, log)| {
//             let topics: Vec<_> = log
//                 .topics
//                 .iter()
//                 .map(|topic| FixedBytes::new(topic.to_fixed_bytes()))
//                 .collect();
//             println!("{:?}\n\n", log.block_number);
//             L1MessageQueueEvents::decode_raw_log(&topics, &log.data, true)
//                 .ok()
//                 .map(|event| (receipt.transaction_index, event))
//         })
//         .filter(|(_, event)| match event {
//             L1MessageQueueEvents::QueueTransaction(_) => true,
//             _ => false,
//         })
//         .collect();

//     println!("filtered logs length: {:?}", filtered_logs.len());
//     let event = &filtered_logs[0].1;
//     match event {
//         L1MessageQueueEvents::QueueTransaction(L1MessageQueue::QueueTransaction {
//             sender,
//             target,
//             value,
//             queueIndex,
//             gasLimit,
//             data,
//         }) => {
//             let l1_msg_tx = L1MessageTx {
//                 queue_index: *queueIndex,
//                 gas: *gasLimit,
//                 to: *target,
//                 value: *value,
//                 data: data.to_vec(),
//                 sender: *sender,
//             };

//             let rlp_encoded_l1_msg_tx = alloy_rlp::encode(&l1_msg_tx);
//             let _ = tx.put::<tables::SyncL1MessageQueue>(
//                 format!("L1{}", l1_msg_tx.queue_index),
//                 rlp_encoded_l1_msg_tx,
//             );

//             let insert_result =
//                 tx.get::<tables::SyncL1MessageQueue>(format!("L1{}", l1_msg_tx.queue_index));

//             println!("Result\n {:?}", insert_result.unwrap());
//             // println!("{:?}", l1_msg_tx);
//         }
//         _ => {
//             println!("Other event");
//         }
//     }

//     // println!("{:?}", event.);

//     // for tx in block.transactions {
//     //     let tx = provider.get_transaction(tx).await.unwrap().unwrap();
//     //     println!("{:?}", tx);
//     // }
//     // println!("block tx length: {:?}", block.transactions.len());
//     // let mut transactions = vec![];
//     // for tx in block.transactions {
//     //     let tx = provider.get_transaction(tx).await.unwrap().unwrap();
//     //     // println!("{:?}", tx);
//     //     transactions.push(tx);
//     // }

//     // let filtered_transactions: Vec<_> = transactions
//     //     .iter()
//     //     .filter(|tx| {
//     //         tx.from == H160::from_str("0x21b8a9F5a4640c3FC13E19C48e776173e1210995").unwrap()
//     //             && tx.to
//     //                 == Some(c).unwrap())
//     //     })
//     //     .collect();

//     // println!("filtered tx length: {:?}", filtered_transactions.len());
//     // println!("filtered tx: {:?}", filtered_transactions[0]);
// }

// // let chain_id = provider.get_chainid().await.unwrap();
// // println!("Chain ID: {:?}", chain_id);
// // println!("Block number: {:?}", block_number);

// // Ok(())
