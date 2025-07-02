use {
    crate::{json_utils::parse_params_0, jsonrpc::JsonRpcError},
    umi_app::{ApplicationReader, Dependencies},
};

pub async fn execute<'app>(
    request: serde_json::Value,
    app: &ApplicationReader<'app, impl Dependencies<'app>>,
) -> Result<serde_json::Value, JsonRpcError> {
    parse_params_0(request)?;
    // this is a generic server error code
    let response = app.block_number()?;

    // Format the block number as a hex string
    Ok(serde_json::to_value(format!("0x{:x}", response))
        .expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::tests::create_app,
        alloy::primitives::ruint::aliases::U256,
        move_core_types::account_address::AccountAddress,
        std::sync::Arc,
        umi_app::{Application, CommandActor, HybridBlockHashCache, TestDependencies},
        umi_blockchain::{
            block::{Eip1559GasFee, InMemoryBlockQueries, InMemoryBlockRepository, UmiBlockHash},
            in_memory::shared_memory,
            payload::{InMemoryPayloadQueries, InProgressPayloads},
            receipt::{InMemoryReceiptQueries, InMemoryReceiptRepository, receipt_memory},
            state::InMemoryStateQueries,
            transaction::{InMemoryTransactionQueries, InMemoryTransactionRepository},
        },
        umi_evm_ext::state::InMemoryStorageTrieRepository,
        umi_execution::UmiBaseTokenAccounts,
        umi_genesis::config::GenesisConfig,
        umi_state::{InMemoryState, InMemoryTrieDb},
    };

    pub fn create_app_without_genesis() -> (
        ApplicationReader<'static, TestDependencies>,
        Application<'static, TestDependencies>,
    ) {
        let genesis_config = GenesisConfig::default();

        let (memory_reader, memory) = shared_memory::new();
        let block_hash_cache =
            HybridBlockHashCache::new(memory_reader.clone(), InMemoryBlockQueries);
        let repository = InMemoryBlockRepository::new();

        let trie_db = Arc::new(InMemoryTrieDb::empty());
        let state = InMemoryState::empty(trie_db.clone());
        let state_queries = InMemoryStateQueries::new(
            memory_reader.clone(),
            trie_db,
            genesis_config.initial_state_root,
        );
        let evm_storage = InMemoryStorageTrieRepository::new();
        let (receipt_memory_reader, receipt_memory) = receipt_memory::new();
        let in_progress_payloads = InProgressPayloads::default();

        (
            ApplicationReader {
                genesis_config: genesis_config.clone(),
                base_token: UmiBaseTokenAccounts::new(AccountAddress::ONE),
                block_queries: InMemoryBlockQueries,
                payload_queries: InMemoryPayloadQueries::new(in_progress_payloads.clone()),
                receipt_queries: InMemoryReceiptQueries::new(),
                receipt_memory: receipt_memory_reader.clone(),
                storage: memory_reader.clone(),
                state_queries: state_queries.clone(),
                evm_storage: evm_storage.clone(),
                transaction_queries: InMemoryTransactionQueries::new(),
                block_hash_lookup: block_hash_cache.clone(),
            },
            Application {
                mem_pool: Default::default(),
                resolver_cache: Default::default(),
                genesis_config,
                gas_fee: Eip1559GasFee::default(),
                base_token: UmiBaseTokenAccounts::new(AccountAddress::ONE),
                l1_fee: U256::ZERO,
                l2_fee: U256::ZERO,
                block_hash: UmiBlockHash,
                block_queries: InMemoryBlockQueries,
                block_repository: repository,
                on_payload: CommandActor::on_payload_in_memory(),
                on_tx: CommandActor::on_tx_noop(),
                on_tx_batch: CommandActor::on_tx_batch_noop(),
                payload_queries: InMemoryPayloadQueries::new(in_progress_payloads),
                receipt_queries: InMemoryReceiptQueries::new(),
                receipt_repository: InMemoryReceiptRepository::new(),
                receipt_memory,
                storage: memory,
                receipt_memory_reader,
                storage_reader: memory_reader,
                state,
                state_queries,
                evm_storage,
                transaction_queries: InMemoryTransactionQueries::new(),
                transaction_repository: InMemoryTransactionRepository::new(),
                block_hash_writer: block_hash_cache.clone(),
                block_hash_lookup: block_hash_cache,
            },
        )
    }

    #[tokio::test]
    async fn test_execute() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x0""#).unwrap();
        let actual_response = execute(request, &reader).await.unwrap();

        assert_eq!(actual_response, expected_response);
    }

    #[tokio::test]
    #[should_panic = "At least genesis block should exist"]
    async fn test_bad_input() {
        let (reader, _app) = create_app_without_genesis();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [
            ],
            "id": 1
        });

        // invariant violation causes panic upon conversion into `JsonRpcError`
        let _ = execute(request, &reader).await;
    }
}
