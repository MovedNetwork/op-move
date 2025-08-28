use {
    crate::{allow, dependency, initialize_app},
    alloy::{
        consensus::transaction::TxEnvelope,
        eips::{BlockNumberOrTag, Encodable2718},
        primitives::{hex, Address, B256},
        rpc::types::TransactionRequest,
    },
    move_core_types::{identifier::Identifier, language_storage::StructTag},
    serde::de::DeserializeOwned,
    std::future::Future,
    umi_api::{
        jsonrpc::JsonRpcResponse,
        schema::{
            mv::{TableHandle, TableItemRequest},
            ForkchoiceUpdatedResponseV1, GetBlockResponse, GetPayloadResponseV3,
        },
    },
    umi_app::{ApplicationReader, CommandQueue},
    umi_blockchain::{
        block::{Block, BlockHash, ExtendedBlock, Header},
        receipt::TransactionReceipt,
        state::MoveResourceResponse,
    },
    umi_execution::U256,
    umi_genesis::config::GenesisConfig,
    umi_server_args::Database,
};

const DEPOSIT_TX: &[u8] = &hex!("7ef8f8a032595a51f0561028c684fbeeb46c7221a34be9a2eedda60a93069dd77320407e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000000000000000000000000000000000000006807cdc800000000000000220000000000000000000000000000000000000000000000000000000000a68a3a000000000000000000000000000000000000000000000000000000000000000198663a8bf712c08273a02876877759b43dc4df514214cc2f6008870b9a8503380000000000000000000000008c67a7b8624044f8f672e9ec374dfa596f01afb9");

pub struct TestContext<'test> {
    pub genesis_config: GenesisConfig,
    pub queue: CommandQueue,
    pub reader: ApplicationReader<'test, dependency::ReaderDependency>,
    head: B256,
    pub timestamp: u64,
    path: &'static str,
}

impl TestContext<'static> {
    pub async fn run<'f, F, FU>(mut future: FU) -> anyhow::Result<()>
    where
        F: Future<Output = anyhow::Result<()>> + Send + 'f,
        FU: FnMut(Self) -> F + Send,
    {
        let db = Database::default();
        let genesis_config = GenesisConfig::default();
        let (mut app, reader) = initialize_app(db, &genesis_config);

        let genesis_block = create_test_genesis_block(&app.block_hash, &genesis_config);
        let head = genesis_block.hash;
        let timestamp = genesis_block.block.header.timestamp;
        app.genesis_update(genesis_block).unwrap();

        let (queue, state) = umi_app::create(&mut app, 10);

        let ctx = Self {
            genesis_config,
            queue,
            reader,
            head,
            timestamp,
            path: "/",
        };

        umi_app::run_with_actor(state, future(ctx)).await
    }

    pub fn with_path(&mut self, path: &'static str) {
        self.path = path;
    }

    pub async fn produce_block(&mut self) -> anyhow::Result<B256> {
        self.timestamp += 1;
        let head_hash = self.head;
        let timestamp = self.timestamp;
        let prev_randao = B256::random();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "engine_forkchoiceUpdatedV3",
            "params": [
                {
                    "headBlockHash": format!("{head_hash}"),
                    "safeBlockHash": format!("{head_hash}"),
                    "finalizedBlockHash": format!("{head_hash}")
                },
                {
                    "timestamp": format!("{timestamp:#x}"),
                    "prevRandao": format!("{prev_randao}"),
                    "suggestedFeeRecipient": "0x4200000000000000000000000000000000000011",
                    "withdrawals": [],
                    "parentBeaconBlockRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "transactions": [
                        hex::encode(DEPOSIT_TX)
                    ],
                    "gasLimit": "0x1c9c380"
                }
            ]
        });
        let response: ForkchoiceUpdatedResponseV1 = self.handle_request(&request).await?;
        let payload_id = response.payload_id.unwrap();

        self.queue.wait_for_pending_commands().await;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "engine_getPayloadV3",
            "params": [
               String::from(payload_id),
            ]
        });
        let response: GetPayloadResponseV3 = self.handle_request(&request).await?;

        self.head = response.execution_payload.block_hash;
        Ok(self.head)
    }

    pub async fn send_raw_transaction(&self, tx: TxEnvelope) -> anyhow::Result<B256> {
        let bytes = tx.encoded_2718();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "eth_sendRawTransaction",
            "params": [
                format!("0x{}", hex::encode(bytes)),
            ]
        });
        let tx_hash: B256 = self.handle_request(&request).await?;
        Ok(tx_hash)
    }

    pub async fn get_nonce(&self, address: Address) -> anyhow::Result<u64> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "eth_getTransactionCount",
            "params": [address]
        });
        let result: U256 = self.handle_request(&request).await?;
        Ok(result.saturating_to())
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> anyhow::Result<Option<TransactionReceipt>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "eth_getTransactionReceipt",
            "params": [
                format!("{tx_hash:?}"),
            ]
        });
        let receipt = self.handle_request(&request).await?;
        Ok(receipt)
    }

    pub async fn eth_call(
        &self,
        tx: TransactionRequest,
        block: BlockNumberOrTag,
    ) -> anyhow::Result<String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "eth_call",
            "params": [
                tx,
                block,
            ]
        });
        let result = self.handle_request(&request).await?;
        Ok(result)
    }

    pub async fn mv_list_modules(
        &self,
        address: Address,
        after: Option<&Identifier>,
        limit: Option<u32>,
        block: BlockNumberOrTag,
    ) -> anyhow::Result<Vec<Identifier>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "mv_listModules",
            "params": [
                {
                    "address": address,
                    "after": after,
                    "limit": limit,
                },
                block,
            ]
        });
        let result = self.handle_request(&request).await?;
        Ok(result)
    }

    pub async fn mv_list_resources(
        &self,
        address: Address,
        after: Option<&StructTag>,
        limit: Option<u32>,
        block: BlockNumberOrTag,
    ) -> anyhow::Result<Vec<StructTag>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "mv_listResources",
            "params": [
                {
                    "address": address,
                    "after": after,
                    "limit": limit,
                },
                block,
            ]
        });
        let result = self.handle_request(&request).await?;
        Ok(result)
    }

    pub async fn mv_get_resource(
        &self,
        address: Address,
        resource: &StructTag,
        block: BlockNumberOrTag,
    ) -> anyhow::Result<MoveResourceResponse> {
        let resource_str = format!("0x{}", resource.to_canonical_string());
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "mv_getResource",
            "params": [
                address,
                resource_str,
                block,
            ]
        });
        let result = self.handle_request(&request).await?;
        Ok(result)
    }

    pub async fn mv_get_table_item(
        &self,
        handle: &TableHandle,
        request: TableItemRequest,
        block: BlockNumberOrTag,
    ) -> anyhow::Result<serde_json::Value> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "mv_getTableItem",
            "params": [
                handle.0,
                request,
                block,
            ]
        });
        let result = self.handle_request(&request).await?;
        Ok(result)
    }

    pub async fn execute_transaction(
        &mut self,
        tx: TxEnvelope,
    ) -> anyhow::Result<TransactionReceipt> {
        let tx_hash = self.send_raw_transaction(tx).await?;
        let block_hash = self.produce_block().await?;
        let receipt = self.get_transaction_receipt(tx_hash).await?.unwrap();
        assert_eq!(receipt.inner.block_hash.unwrap(), block_hash);
        Ok(receipt)
    }

    pub async fn get_block_by_number(&self, number: u64) -> anyhow::Result<GetBlockResponse> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "eth_getBlockByNumber",
            "params": [
                format!("{number:#x}"),
                true
            ]
        });
        let block: GetBlockResponse = self.handle_request(&request).await?;
        Ok(block)
    }

    pub async fn eth_get_storage_at(&self, address: Address, index: U256) -> anyhow::Result<U256> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "eth_getStorageAt",
            "params": [address, index, "latest"]
        });
        let value: U256 = self.handle_request(&request).await?;
        Ok(value)
    }

    pub async fn handle_request<T: DeserializeOwned>(
        &self,
        request: &serde_json::Value,
    ) -> anyhow::Result<T> {
        let server = crate::server_filter(&self.queue, &self.reader, &allow::auth, None);

        let response = warp::test::request()
            .method("POST")
            .json(request)
            .path(self.path)
            .reply(&server)
            .await;
        let response: JsonRpcResponse = serde_json::from_slice(response.body()).unwrap();

        if let Some(error) = response.error {
            anyhow::bail!("Error response from request {request:?}: {error:?}");
        }

        let result: T =
            serde_json::from_value(response.result.expect("If not error then has result"))?;
        Ok(result)
    }

    pub async fn shutdown(self) {
        drop(self.queue);
    }
}

/// Test genesis block differs primarily in that it makes gas free and has Move state root.
fn create_test_genesis_block(
    block_hash: &impl BlockHash,
    genesis_config: &GenesisConfig,
) -> ExtendedBlock {
    let genesis_header = Header {
        state_root: genesis_config.initial_state_root,
        ..Default::default()
    };
    let hash = block_hash.block_hash(&genesis_header);
    let genesis_block = Block::new(genesis_header, Vec::new());

    genesis_block
        .into_extended_with_hash(hash)
        .with_value(U256::ZERO)
}
