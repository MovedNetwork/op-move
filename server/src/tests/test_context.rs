use {
    crate::{create_genesis_block, dependency, initialize_app},
    alloy::{
        consensus::transaction::TxEnvelope,
        eips::Encodable2718,
        primitives::{hex, B256},
    },
    moved_api::schema::{ForkchoiceUpdatedResponseV1, GetBlockResponse, GetPayloadResponseV3},
    moved_app::{ApplicationReader, CommandQueue, Dependencies},
    moved_blockchain::{payload::StatePayloadId, receipt::TransactionReceipt},
    moved_genesis::config::GenesisConfig,
    serde::de::DeserializeOwned,
    std::future::Future,
};

const DEPOSIT_TX: &[u8] = &hex!("7ef8f8a032595a51f0561028c684fbeeb46c7221a34be9a2eedda60a93069dd77320407e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000000000000000000000000000000000000006807cdc800000000000000220000000000000000000000000000000000000000000000000000000000a68a3a000000000000000000000000000000000000000000000000000000000000000198663a8bf712c08273a02876877759b43dc4df514214cc2f6008870b9a8503380000000000000000000000008c67a7b8624044f8f672e9ec374dfa596f01afb9");

pub struct TestContext {
    pub genesis_config: GenesisConfig,
    pub queue: CommandQueue,
    pub reader: ApplicationReader<dependency::Dependency>,
    head: B256,
    timestamp: u64,
}

impl TestContext {
    pub async fn run<'f, F, FU>(mut future: FU) -> anyhow::Result<()>
    where
        F: Future<Output = anyhow::Result<()>> + Send + 'f,
        FU: FnMut(Self) -> F + Send,
    {
        let genesis_config = GenesisConfig::default();
        let (mut app, reader) = initialize_app(genesis_config.clone());

        let genesis_block = create_genesis_block(&app.block_hash, &genesis_config);
        let head = genesis_block.hash;
        let timestamp = genesis_block.block.header.timestamp;
        app.genesis_update(genesis_block);

        let (queue, state) = moved_app::create(&mut app, 10);

        let ctx = Self {
            genesis_config,
            queue,
            reader,
            head,
            timestamp,
        };

        moved_app::run(state, future(ctx))
    }

    pub async fn produce_block(&mut self) -> anyhow::Result<B256> {
        self.timestamp += 1;
        let head_hash = self.head;
        let timestamp = self.timestamp;
        let prev_rando = B256::random();
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
                    "prevRandao": format!("{prev_rando}"),
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
        let response: ForkchoiceUpdatedResponseV1 =
            handle_request(request, &self.queue, self.reader.clone()).await?;
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
        let response: GetPayloadResponseV3 =
            handle_request_multiple_tries(request, &self.queue, self.reader.clone()).await?;

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
        let tx_hash: B256 = handle_request(request, &self.queue, self.reader.clone()).await?;
        Ok(tx_hash)
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
        let receipt = handle_request(request, &self.queue, self.reader.clone()).await?;
        Ok(receipt)
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
        let block: GetBlockResponse =
            handle_request(request, &self.queue, self.reader.clone()).await?;
        Ok(block)
    }

    pub async fn shutdown(self) {
        drop(self.queue);
    }
}

pub async fn handle_request<T: DeserializeOwned>(
    request: serde_json::Value,
    queue: &CommandQueue,
    app: ApplicationReader<impl Dependencies>,
) -> anyhow::Result<T> {
    let response = moved_api::request::handle(
        request.clone(),
        queue.clone(),
        |_| true,
        &StatePayloadId,
        app,
    )
    .await;

    if let Some(error) = response.error {
        anyhow::bail!("Error response from request {request:?}: {error:?}");
    }

    let result: T = serde_json::from_value(response.result.expect("If not error then has result"))?;
    Ok(result)
}

pub async fn handle_request_multiple_tries<T: DeserializeOwned>(
    request: serde_json::Value,
    queue: &CommandQueue,
    app: ApplicationReader<impl Dependencies>,
) -> anyhow::Result<T> {
    let max_tries = 300;

    for i in 1..=max_tries {
        let response = moved_api::request::handle(
            request.clone(),
            queue.clone(),
            |_| true,
            &StatePayloadId,
            app.clone(),
        )
        .await;

        if let Some(error) = response.error {
            if i == max_tries {
                anyhow::bail!("Error response from request {request:?}: {error:?}");
            } else {
                continue;
            }
        }

        let result: T =
            serde_json::from_value(response.result.expect("If not error then has result"))?;
        return Ok(result);
    }

    unreachable!()
}
