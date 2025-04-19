//! This module is designed to parse the
//! [Superchain Token List](https://github.com/ethereum-optimism/ethereum-optimism.github.io).
//! The idea is that we will automatically bridge the same tokens that exist on Optimism
//! onto our network. We will only do this for standard bridged tokens. Therefore we ignore
//! any tokens marked as `nonstandard` or `nobridge` as well as any tokens that override
//! the Optimism bridge (i.e. did not use the standard bridge).

use {
    alloy::{
        dyn_abi::DynSolValue,
        primitives::{Address, U256, address},
    },
    anyhow::{Context, Result},
    bytes::Bytes,
    move_binary_format::errors::PartialVMResult,
    move_core_types::{
        account_address::AccountAddress,
        effects::ChangeSet,
        language_storage::{ModuleId, StructTag},
        metadata::Metadata,
        value::MoveTypeLayout,
    },
    move_vm_types::resolver::{ModuleResolver, ResourceResolver},
    moved_evm_ext::{
        Changes, HeaderForExecution, NativeEVMContext, evm_transact_with_native,
        extract_evm_changes_from_native,
        state::{InMemoryStorageTrieRepository, StorageTrieRepository},
    },
    std::{
        fs::{read_dir, read_to_string},
        path::Path,
    },
};

const FACTORY_ADDRESS: Address = address!("4200000000000000000000000000000000000012");
/// createOptimismMintableERC20WithDecimals selector
const SELECTOR: [u8; 4] = [0x8c, 0xf0, 0x62, 0x9c];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgedToken {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub ethereum_address: Address,
}

pub fn parse_token_list(path: &Path) -> Result<Vec<BridgedToken>> {
    let mut result = Vec::new();

    // If the path is not a directory then we assume it is a single
    // file containing a JSON list of the token entries.
    if !path.is_dir() {
        let data = read_to_string(path).context(format!("Path: {path:?}"))?;
        let json: serde_json::Value = serde_json::from_str(&data)?;
        let array = json
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Single file must contain an array"))?;
        for value in array {
            if let Some(token) = parse_json(value)? {
                result.push(token);
            }
        }
        return Ok(result);
    }

    for maybe_entry in read_dir(path)? {
        let entry = maybe_entry?;
        if entry.metadata()?.is_dir() {
            let data_path = entry.path().join("data.json");
            if let Some(token) = parse_single_data_file(&data_path)? {
                result.push(token);
            }
        }
    }

    Ok(result)
}

pub fn deploy_bridged_tokens(
    mut l2_changes: Changes,
    tokens: Vec<BridgedToken>,
) -> Result<Changes> {
    let resolver = ChangesBasedResolver {
        changes: &l2_changes.accounts,
    };
    let trie_storage = InMemoryStorageTrieRepository::new();
    trie_storage.apply(l2_changes.storage.clone())?;
    let block_header = HeaderForExecution::default();
    let mut ctx = NativeEVMContext::new(&resolver, &trie_storage, &(), block_header);
    for token in tokens {
        let data = encode_params(token);
        let outcome = evm_transact_with_native(
            &mut ctx,
            Address::default(),
            FACTORY_ADDRESS.into(),
            Default::default(),
            data,
            u64::MAX,
        )
        .map_err(|_e| {
            anyhow::anyhow!("Bridged token deployment failed: evm_transact_with_native")
        })?;
        if !outcome.result.is_success() {
            anyhow::bail!("Bridged token deployment failed: EVM outcome");
        }
    }
    let new_changes = extract_evm_changes_from_native(&ctx);
    l2_changes.accounts.squash(new_changes.accounts)?;
    for (address, trie_changes) in new_changes.storage {
        l2_changes.storage = l2_changes.storage.with_trie_changes(address, trie_changes);
    }
    Ok(l2_changes)
}

fn encode_params(token: BridgedToken) -> Vec<u8> {
    [
        SELECTOR.as_slice(),
        &DynSolValue::Tuple(vec![
            DynSolValue::Address(token.ethereum_address),
            DynSolValue::String(token.name),
            DynSolValue::String(token.symbol),
            DynSolValue::Uint(U256::from(token.decimals), 8),
        ])
        .abi_encode_params(),
    ]
    .concat()
}

fn parse_single_data_file(path: &Path) -> Result<Option<BridgedToken>> {
    let data = read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&data)?;
    parse_json(&json)
}

fn parse_json(json: &serde_json::Value) -> Result<Option<BridgedToken>> {
    let object = json
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Must be object"))?;

    let bool_or_true = |v: &serde_json::Value| -> bool { v.as_bool().unwrap_or(true) };

    // We only automatically bridge standard tokens bridged to Optimism from Ethereum.
    // If these fields are present then this is not the case.
    let is_non_standard = object.get("nonstandard").map(bool_or_true).unwrap_or(false);
    let is_no_bridge = object.get("nobridge").map(bool_or_true).unwrap_or(false);
    if is_non_standard || is_no_bridge {
        return Ok(None);
    }

    let mut name = object
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Name must be a string"))?;
    let mut symbol = object
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Symbol must be a string"))?;
    let mut decimals = object
        .get("decimals")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Decimals must be a number"))?;

    let tokens = object
        .get("tokens")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("Tokens must be an object"))?;
    let Some(ethereum) = tokens.get("ethereum").and_then(|v| v.as_object()) else {
        return Ok(None);
    };
    let Some(optimism) = tokens.get("optimism").and_then(|v| v.as_object()) else {
        return Ok(None);
    };

    // If a different bridge than the Optimism standard bridge was used then
    // we do not automatically bridge it ourselves.
    if has_ethereum_bridge_override(ethereum).unwrap_or(false)
        || has_optimism_bridge_override(optimism).unwrap_or(false)
    {
        return Ok(None);
    }

    let ethereum_address = ethereum
        .get("address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Ethereum address must be a string"))?;
    let ethereum_address: Address = ethereum_address.parse()?;

    // ETH is our base token, so no need to bridge it again
    if ethereum_address.is_zero() {
        return Ok(None);
    }

    // Check if any metadata has overrides.
    if let Some(overrides) = optimism.get("overrides").and_then(|v| v.as_object()) {
        if let Some(alt_name) = overrides.get("name").and_then(|v| v.as_str()) {
            name = alt_name;
        }
        if let Some(alt_symbol) = overrides.get("symbol").and_then(|v| v.as_str()) {
            symbol = alt_symbol;
        }
        if let Some(alt_decimals) = overrides.get("decimals").and_then(|v| v.as_u64()) {
            decimals = alt_decimals;
        }
    }

    if decimals > u8::MAX as u64 {
        anyhow::bail!("Invalid value for token decimals");
    }

    Ok(Some(BridgedToken {
        name: name.into(),
        symbol: symbol.into(),
        decimals: decimals as u8,
        ethereum_address,
    }))
}

fn has_ethereum_bridge_override(
    ethereum: &serde_json::Map<String, serde_json::Value>,
) -> Option<bool> {
    Some(
        ethereum
            .get("overrides")?
            .as_object()?
            .get("bridge")?
            .as_object()?
            .contains_key("optimism"),
    )
}

fn has_optimism_bridge_override(
    optimism: &serde_json::Map<String, serde_json::Value>,
) -> Option<bool> {
    Some(
        optimism
            .get("overrides")?
            .as_object()?
            .contains_key("bridge"),
    )
}

struct ChangesBasedResolver<'a> {
    changes: &'a ChangeSet,
}

impl ResourceResolver for ChangesBasedResolver<'_> {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        _metadata: &[Metadata],
        _layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<(Option<Bytes>, usize)> {
        let bytes = self
            .changes
            .accounts()
            .get(address)
            .and_then(|account| account.resources().get(struct_tag))
            .and_then(|op| op.clone().ok());
        let size = bytes.as_ref().map(|b| b.len()).unwrap_or(0);
        Ok((bytes, size))
    }
}

impl ModuleResolver for ChangesBasedResolver<'_> {
    fn get_module_metadata(&self, _module_id: &ModuleId) -> Vec<Metadata> {
        Vec::new()
    }

    fn get_module(&self, id: &ModuleId) -> PartialVMResult<Option<Bytes>> {
        let bytes = self
            .changes
            .accounts()
            .get(id.address())
            .and_then(|account| account.modules().get(id.name()))
            .and_then(|op| op.clone().ok());
        Ok(bytes)
    }
}

// Test to double check `deploy_bridged_tokens` does
// indeed deploy some tokens.
#[test]
fn test_deploy_bridged_tokens() {
    let config = crate::config::GenesisConfig::default();
    let n_bridged_tokens = config.token_list.len();
    assert!(n_bridged_tokens > 0);
    let state = moved_state::InMemoryState::default();
    let storage = InMemoryStorageTrieRepository::new();
    let l2_changes = crate::l2_contracts::init_state(config.l2_contract_genesis, &state, &storage);
    let new_l2_changes = deploy_bridged_tokens(l2_changes.clone(), config.token_list).unwrap();
    let added_addresses = new_l2_changes
        .storage
        .tries
        .keys()
        .filter(|address| !l2_changes.storage.tries.contains_key(*address));
    assert_eq!(added_addresses.count(), n_bridged_tokens);
}
