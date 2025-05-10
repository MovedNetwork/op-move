use {
    crate::{
        json_utils::{self},
        jsonrpc::JsonRpcError,
    },
    alloy::{
        eips::{BlockId, BlockNumberOrTag},
        primitives::{Address, U256},
    },
    moved_app::{ApplicationReader, Dependencies},
};

pub async fn execute(
    request: serde_json::Value,
    app: ApplicationReader<impl Dependencies>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (address, storage_slots, block_number) = parse_params(request)?;

    let response = app
        .proof(address, storage_slots, block_number)
        .ok_or(JsonRpcError::block_not_found(block_number))?;

    // Format the balance as a hex string
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params(request: serde_json::Value) -> Result<(Address, Vec<U256>, BlockId), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] | [_] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [a, b] => {
            let address: Address = json_utils::deserialize(a)?;
            let storage_slots = json_utils::deserialize(b)?;
            Ok((
                address,
                storage_slots,
                BlockId::Number(BlockNumberOrTag::Latest),
            ))
        }
        [a, b, c] => {
            let address: Address = json_utils::deserialize(a)?;
            let storage_slots = json_utils::deserialize(b)?;
            let block_number: BlockId = json_utils::deserialize(c)?;
            Ok((address, storage_slots, block_number))
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::tests::create_app,
        alloy::{hex, primitives::address},
        moved_blockchain::state::ProofResponse,
        moved_shared::primitives::U64,
        std::str::FromStr,
        test_case::test_case,
    };

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    fn test_parse_params_with_block_number(block: &str) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getProof",
            "params": [
                "0x0000000000000000000000000000000000000001",
                [],
                block,
            ],
            "id": 1
        });

        let (address, storage_slots, block_number) = parse_params(request).unwrap();
        assert_eq!(
            address,
            Address::from_str("0x0000000000000000000000000000000000000001").unwrap()
        );
        assert_eq!(storage_slots, Vec::new());
        match block {
            "latest" => assert_eq!(block_number, BlockNumberOrTag::Latest.into()),
            "pending" => assert_eq!(block_number, BlockNumberOrTag::Pending.into()),
            _ => assert_eq!(
                block_number,
                BlockNumberOrTag::Number(U64::from_str(block).unwrap().into_limbs()[0]).into()
            ),
        }
    }

    #[tokio::test]
    async fn test_execute() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getProof",
            "params": [
                "0x4200000000000000000000000000000000000016",
                [],
                "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
            ],
            "id": 1
        });

        let response: ProofResponse =
            serde_json::from_value(execute(request, reader.clone()).await.unwrap()).unwrap();

        assert_eq!(
            response.address,
            address!("4200000000000000000000000000000000000016")
        );
        assert_eq!(response.balance, U256::ZERO);
        assert_eq!(response.nonce, 0);
        assert_eq!(
            response.code_hash,
            hex!("fa8c9db6c6cab7108dea276f4cd09d575674eb0852c0fa3187e59e98ef977998")
        );
        assert_eq!(response.storage_proof, Vec::new());

        for bytes in response.account_proof {
            let list: Vec<alloy::rlp::Bytes> = alloy::rlp::decode_exact(bytes).unwrap();
            // Leaf and extension nodes have length 2; branch nodes have length 17
            assert!(list.len() == 2 || list.len() == 17);
        }
    }
}
