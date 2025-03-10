use {
    super::*, moved_evm_ext::storage::InMemoryStorageTrieRepository,
    moved_genesis::config::CHAIN_ID,
};

/// Represents the base token state for a test transaction
#[derive(Debug)]
pub enum TestBaseToken {
    /// No base token state
    Empty,
    /// Contains moved base token accounts information
    Moved(MovedBaseTokenAccounts),
}

/// Represents a test transaction with associated metadata
#[derive(Debug)]
pub struct TestTransaction {
    /// The normalized transaction envelope
    pub tx: NormalizedExtendedTxEnvelope,
    /// Transaction hash
    pub tx_hash: B256,
    /// L1 cost associated with the transaction
    pub l1_cost: u64,
    /// L2 gas limit associated with the transaction
    pub l2_gas_limit: u64,
    /// L2 gas price associated with the transaction
    pub l2_gas_price: U256,
    /// Base token state for the transaction
    pub base_token: TestBaseToken,
}

impl TestTransaction {
    /// Creates a new TestTransaction with default values
    ///
    /// # Arguments
    /// * `tx` - The normalized transaction envelope
    /// * `tx_hash` - The transaction hash
    pub fn new(tx: NormalizedExtendedTxEnvelope, tx_hash: B256) -> Self {
        let gas_limit = tx.gas_limit();
        Self {
            tx,
            tx_hash,
            l1_cost: 0,
            base_token: TestBaseToken::Empty,
            l2_gas_limit: gas_limit,
            l2_gas_price: U256::ZERO,
        }
    }

    /// Sets the L1 cost and base token state for the transaction
    ///
    /// # Arguments
    /// * `l1_cost` - The L1 cost to set
    /// * `base_token` - The moved base token accounts to set
    /// * `l2_gas_limit` - The L2 gas limit to set
    /// * `l2_gas_price` - The L2 gas price to set
    pub fn with_cost_and_token(
        &mut self,
        l1_cost: u64,
        base_token: MovedBaseTokenAccounts,
        l2_gas_limit: u64,
        l2_gas_price: U256,
    ) {
        self.l1_cost = l1_cost;
        self.base_token = TestBaseToken::Moved(base_token);
        self.l2_gas_limit = l2_gas_limit;
        self.l2_gas_price = l2_gas_price;
    }
}

/// TestContext provides a simplified interface for testing Move contracts
/// by managing state, transactions, and contract deployment.
pub struct TestContext {
    /// The in-memory state for testing
    pub state: InMemoryState,
    pub evm_storage: InMemoryStorageTrieRepository,
    /// Genesis configuration
    pub genesis_config: GenesisConfig,
    /// Transaction signer
    pub signer: Signer,
    /// Move address for contract deployment
    pub move_address: AccountAddress,
}

impl TestContext {
    /// Creates a new test context with initialized state and default signer
    pub fn new() -> Self {
        let genesis_config = GenesisConfig::default();
        let mut state = InMemoryState::new();
        let (changes, tables) = moved_genesis_image::load();
        moved_genesis::apply(changes, tables, &genesis_config, &mut state);

        Self {
            state,
            evm_storage: InMemoryStorageTrieRepository::new(),
            genesis_config,
            signer: Signer::new(&PRIVATE_KEY),
            move_address: EVM_ADDRESS.to_move_address(),
        }
    }

    /// Deploys a Move contract module and returns its ModuleId
    ///
    /// # Arguments
    /// * `module_name` - Name of the module to deploy
    ///
    /// # Returns
    /// The ModuleId of the deployed contract
    pub fn deploy_contract(&mut self, module_name: &str) -> ModuleId {
        let module_bytes = self.compile_module(module_name, self.move_address);
        let (tx_hash, tx) = create_transaction(&mut self.signer, TxKind::Create, module_bytes);
        let transaction = TestTransaction::new(tx, tx_hash);
        let outcome = self.execute_tx(&transaction).unwrap();
        self.state.apply(outcome.changes).unwrap();

        let module_id = ModuleId::new(self.move_address, Identifier::new(module_name).unwrap());
        assert!(
            self.state
                .resolver()
                .get_module(&module_id)
                .unwrap()
                .is_some(),
            "Code should be deployed"
        );
        module_id
    }

    /// Executes a Move script with arguments
    ///
    /// # Arguments
    /// * `script_name` - Name of the script to execute
    /// * `local_deps` - Local module dependencies needed by the script
    /// * `args` - Arguments to pass to the script
    pub fn run_script(
        &mut self,
        script_name: &str,
        local_deps: &[&str],
        args: Vec<TransactionArgument>,
    ) -> Vec<Log<LogData>> {
        let script_bytes = self.compile_script(script_name, local_deps, args);
        let (tx_hash, tx) = create_transaction(&mut self.signer, TxKind::Create, script_bytes);
        let transaction = TestTransaction::new(tx, tx_hash);
        let outcome = self.execute_tx(&transaction).unwrap();
        self.state.apply(outcome.changes).unwrap();
        // Script transaction should succeed
        outcome.vm_outcome.unwrap();
        outcome.logs
    }

    /// Transfers ETH to a specified address with L1 cost considerations
    ///
    /// # Arguments
    /// * `to` - Destination address for the transfer
    /// * `amount` - Amount of ETH to transfer
    /// * `l1_cost` - L1 cost associated with the transaction
    /// * `l2_cost` - L2 cost associated with the transaction
    ///
    /// # Returns
    /// The execution outcome from the transfer
    pub fn transfer(
        &mut self,
        to: Address,
        amount: U256,
        l1_cost: u64,
        l2_gas_limit: u64,
        l2_gas_price: U256,
    ) -> moved_shared::error::Result<TransactionExecutionOutcome> {
        let (tx_hash, tx) = create_transaction_with_value(
            &mut self.signer,
            TxKind::Call(to),
            Vec::new(),
            U256::from(amount),
        );

        // Default base token is ETH token in address 0x1
        let treasury_address = AccountAddress::ONE;
        let base_token = MovedBaseTokenAccounts::new(treasury_address);
        let mut transaction = TestTransaction::new(tx, tx_hash);
        transaction.with_cost_and_token(l1_cost, base_token, l2_gas_limit, l2_gas_price);
        let outcome = self.execute_tx(&transaction)?;
        self.state.apply(outcome.changes.clone())?;
        let l2_gas_fee = CreateMovedL2GasFee.with_default_gas_fee_multiplier();
        let used_gas_input = L2GasFeeInput::new(outcome.gas_used, outcome.l2_price);
        let l2_cost = l2_gas_fee.l2_fee(used_gas_input);

        let treasury_balance = self.get_balance(treasury_address.to_eth_address());
        assert_eq!(
            treasury_balance,
            U256::from(l1_cost).saturating_add(l2_cost)
        );
        Ok(outcome)
    }

    /// Executes a Move entry function with the given arguments
    ///
    /// This is the recommended way to call Move functions in tests as it handles
    /// argument serialization and transaction creation.
    ///
    /// # Arguments
    /// * `module_id` - The ModuleId containing the function to execute
    /// * `function` - Name of the function to call
    /// * `args` - List of Move values to pass as arguments
    pub fn execute<'a>(
        &mut self,
        module_id: &ModuleId,
        function: &str,
        args: impl IntoIterator<Item = &'a MoveValue>,
    ) {
        let args = args
            .into_iter()
            .map(|arg| bcs::to_bytes(arg).unwrap())
            .collect();
        let (tx_hash, tx) = create_test_tx(&mut self.signer, module_id, function, args);
        let transaction = TestTransaction::new(tx, tx_hash);
        let outcome = self.execute_tx(&transaction).unwrap();
        // Entry function transaction should succeed
        outcome.vm_outcome.unwrap();
        self.state.apply(outcome.changes).unwrap();
    }

    /// Executes a Move entry function expecting it to fail
    ///
    /// # Arguments
    /// * `module_id` - The ModuleId containing the function
    /// * `function` - Name of the function to call
    /// * `args` - Vector of Move values to pass as arguments
    ///
    /// # Returns
    /// The error that occurred during execution
    pub fn execute_err<'a>(
        &mut self,
        module_id: &ModuleId,
        function: &str,
        args: impl IntoIterator<Item = &'a MoveValue>,
    ) -> moved_shared::error::Error {
        let args = args
            .into_iter()
            .map(|arg| bcs::to_bytes(arg).unwrap())
            .collect();
        let (tx_hash, tx) = create_test_tx(&mut self.signer, module_id, function, args);
        let transaction = TestTransaction::new(tx, tx_hash);
        self.execute_tx(&transaction).unwrap_err()
    }

    /// Low-level transaction execution
    ///
    /// This internal method handles the actual execution of a transaction against the Move VM.
    /// Most tests should use `execute()` instead unless they need fine-grained control.
    ///
    /// # Arguments
    /// * `tx` - The test transaction to execute
    ///
    /// # Returns
    /// The transaction execution outcome or error
    pub(crate) fn execute_tx(
        &mut self,
        tx: &TestTransaction,
    ) -> moved_shared::error::Result<TransactionExecutionOutcome> {
        let l2_fee = CreateMovedL2GasFee.with_default_gas_fee_multiplier();
        let l2_gas_input = L2GasFeeInput::new(tx.l2_gas_limit, tx.l2_gas_price);
        let tx_hash = tx.tx_hash;
        let l1_cost = tx.l1_cost;

        match &tx.base_token {
            TestBaseToken::Empty => execute_transaction(match &tx.tx {
                NormalizedExtendedTxEnvelope::Canonical(tx) => CanonicalExecutionInput {
                    tx,
                    tx_hash: &tx_hash,
                    state: self.state.resolver(),
                    storage_trie: &self.evm_storage,
                    genesis_config: &self.genesis_config,
                    l1_cost: 0,
                    l2_fee,
                    l2_input: l2_gas_input,
                    base_token: &(),
                    block_header: Default::default(),
                }
                .into(),
                NormalizedExtendedTxEnvelope::DepositedTx(tx) => DepositExecutionInput {
                    tx,
                    tx_hash: &tx_hash,
                    state: self.state.resolver(),
                    storage_trie: &self.evm_storage,
                    genesis_config: &self.genesis_config,
                    block_header: Default::default(),
                }
                .into(),
            }),
            TestBaseToken::Moved(moved_base_token) => execute_transaction(match &tx.tx {
                NormalizedExtendedTxEnvelope::Canonical(tx) => CanonicalExecutionInput {
                    tx,
                    tx_hash: &tx_hash,
                    state: self.state.resolver(),
                    storage_trie: &self.evm_storage,
                    genesis_config: &self.genesis_config,
                    l1_cost,
                    l2_fee,
                    l2_input: l2_gas_input,
                    base_token: moved_base_token,
                    block_header: Default::default(),
                }
                .into(),
                NormalizedExtendedTxEnvelope::DepositedTx(tx) => DepositExecutionInput {
                    tx,
                    tx_hash: &tx_hash,
                    state: self.state.resolver(),
                    storage_trie: &self.evm_storage,
                    genesis_config: &self.genesis_config,
                    block_header: Default::default(),
                }
                .into(),
            }),
        }
    }

    /// Deposits ETH directly to an address
    ///
    /// # Arguments
    /// * `to` - Address to receive the deposit
    /// * `amount` - Amount of ETH to deposit
    pub fn deposit_eth(&mut self, to: Address, amount: U256) {
        let balance_before = self.get_balance(to);
        let tx = ExtendedTxEnvelope::DepositedTx(DepositedTx {
            to,
            value: U256::from(amount),
            source_hash: FixedBytes::default(),
            from: to,
            mint: U256::ZERO,
            gas: U64::from(u64::MAX),
            is_system_tx: false,
            data: Vec::new().into(),
        });
        let tx_hash = {
            let capacity = tx.length();
            let mut bytes = Vec::with_capacity(capacity);
            tx.encode(&mut bytes);
            B256::new(keccak256(bytes).0)
        };
        let transaction = TestTransaction::new(tx.try_into().unwrap(), tx_hash);
        let outcome = self.execute_tx(&transaction).unwrap();
        outcome.vm_outcome.unwrap();
        self.state.apply(outcome.changes).unwrap();

        let balance_after = self.get_balance(to);
        assert_eq!(balance_after, balance_before + amount);
    }

    /// Retrieves a resource from the Move state
    /// Uses `resource_address` in the context
    ///
    /// # Arguments
    /// * `module_name` - Name of the module containing the resource
    /// * `struct_name` - Name of the struct representing the resource
    /// * `address` - Address to retrieve the resource for
    ///
    /// # Returns
    /// The deserialized resource of type T
    pub fn get_resource<T: DeserializeOwned>(
        &self,
        module_name: &str,
        struct_name: &str,
        address: AccountAddress,
    ) -> T {
        // Resource was created on a module struct for a resource address
        let struct_tag = StructTag {
            address: self.move_address,
            module: Identifier::new(module_name).unwrap(),
            name: Identifier::new(struct_name).unwrap(),
            type_args: Vec::new(),
        };
        let data = self
            .state
            .resolver()
            .get_resource(&address, &struct_tag)
            .unwrap()
            .unwrap();
        bcs::from_bytes(data.as_ref()).unwrap()
    }

    /// Gets the ETH balance for an address
    ///
    /// # Arguments
    /// * `address` - Address to check balance for
    ///
    /// # Returns
    /// The balance as a u64
    pub fn get_balance(&self, address: Address) -> U256 {
        quick_get_eth_balance(
            &address.to_move_address(),
            self.state.resolver(),
            &self.evm_storage,
        )
    }

    /// Compiles a Move module
    ///
    /// # Arguments
    /// * `module_name` - Name of the module to compile
    /// * `address` - Address to associate with the module
    ///
    /// # Returns
    /// The compiled module bytes ready for deployment
    fn compile_module(&self, module_name: &str, address: AccountAddress) -> Vec<u8> {
        let module_bytes = ModuleCompileJob::new(module_name, &address)
            .compile()
            .unwrap();
        module_bytes_to_tx_data(module_bytes)
    }

    /// Compiles a Move script
    ///
    /// # Arguments
    /// * `script_name` - Name of the script to compile
    /// * `local_deps` - Local module dependencies
    /// * `args` - Transaction arguments for the script
    ///
    /// # Returns
    /// The compiled script bytes
    fn compile_script(
        &self,
        script_name: &str,
        local_deps: &[&str],
        args: Vec<TransactionArgument>,
    ) -> Vec<u8> {
        let script_code = ScriptCompileJob::new(script_name, local_deps)
            .compile()
            .unwrap();
        let script = Script::new(script_code, Vec::new(), args);
        bcs::to_bytes(&ScriptOrModule::Script(script)).unwrap()
    }
}

/// Converts a contract module bytes into a transaction payload
///
/// # Arguments
/// * `module_bytes` - The compiled module bytes
///
/// # Returns
/// Serialized transaction payload
pub fn module_bytes_to_tx_data(module_bytes: Vec<u8>) -> Vec<u8> {
    bcs::to_bytes(&ScriptOrModule::Module(Module::new(module_bytes))).unwrap()
}

/// Creates a test transaction for a Move entry function
///
/// # Arguments
/// * `signer` - Transaction signer
/// * `module_id` - Target module ID
/// * `function` - Function name to call
/// * `args` - Function arguments
///
/// # Returns
/// Transaction hash and normalized transaction envelope
pub fn create_test_tx(
    signer: &mut Signer,
    module_id: &ModuleId,
    function: &str,
    args: Vec<Vec<u8>>,
) -> (B256, NormalizedExtendedTxEnvelope) {
    let entry_fn = EntryFunction::new(
        module_id.clone(),
        Identifier::new(function).unwrap(),
        Vec::new(),
        args,
    );

    create_transaction(
        signer,
        TxKind::Call(EVM_ADDRESS),
        bcs::to_bytes(&TransactionData::EntryFunction(entry_fn)).unwrap(),
    )
}

/// Creates a basic transaction
///
/// # Arguments
/// * `signer` - Transaction signer
/// * `to` - Destination address or contract creation
/// * `input` - Transaction input data
///
/// # Returns
/// Transaction hash and normalized transaction envelope
pub fn create_transaction(
    signer: &mut Signer,
    to: TxKind,
    input: Vec<u8>,
) -> (B256, NormalizedExtendedTxEnvelope) {
    create_transaction_with_value(signer, to, input, U256::ZERO)
}

/// Creates a transaction with a specific value
///
/// # Arguments
/// * `signer` - Transaction signer
/// * `to` - Destination address or contract creation
/// * `input` - Transaction input data
/// * `value` - ETH value to transfer
///
/// # Returns
/// Transaction hash and normalized transaction envelope
pub fn create_transaction_with_value(
    signer: &mut Signer,
    to: TxKind,
    input: Vec<u8>,
    value: U256,
) -> (B256, NormalizedExtendedTxEnvelope) {
    let mut tx = TxEip1559 {
        chain_id: CHAIN_ID,
        nonce: signer.nonce,
        gas_limit: u64::MAX,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to,
        value,
        access_list: Default::default(),
        input: input.into(),
    };
    signer.nonce += 1;
    let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();
    let signed_tx = TxEnvelope::Eip1559(tx.into_signed(signature));
    let tx_hash = *signed_tx.tx_hash();
    let normalized_tx = NormalizedExtendedTxEnvelope::Canonical(signed_tx.try_into().unwrap());

    (tx_hash, normalized_tx)
}

/// Trait for compilation jobs with common functionality
pub trait CompileJob {
    /// Gets the target files to compile
    fn targets(&self) -> Vec<String>;

    /// Gets the dependency files needed for compilation
    fn deps(&self) -> Vec<String>;

    /// Gets the named addresses mapping
    fn named_addresses(&self) -> BTreeMap<String, NumericalAddress>;

    /// Gets the known attributes for compilation
    fn known_attributes(&self) -> BTreeSet<String> {
        BTreeSet::new()
    }

    /// Compiles the Move code
    ///
    /// # Returns
    /// Compiled bytes or error
    fn compile(&self) -> anyhow::Result<Vec<u8>> {
        let targets = self.targets();
        let error_context = format!("Failed to compile {targets:?}");
        let compiler = Compiler::from_files(
            targets,
            self.deps(),
            self.named_addresses(),
            Flags::empty(),
            &self.known_attributes(),
        );
        let (_, result) = compiler.build().context(error_context)?;
        let compiled_unit = result.unwrap().0.pop().unwrap().into_compiled_unit();
        let bytes = compiled_unit.serialize(None);
        Ok(bytes)
    }
}

pub struct ModuleCompileJob {
    targets_inner: Vec<String>,
    named_addresses_inner: BTreeMap<String, NumericalAddress>,
}

impl ModuleCompileJob {
    pub fn new(package_name: &str, address: &AccountAddress) -> Self {
        let named_address_mapping: std::collections::BTreeMap<_, _> = std::iter::once((
            package_name.to_string(),
            NumericalAddress::new(address.into(), NumberFormat::Hex),
        ))
        .chain(custom_framework_named_addresses())
        .chain(aptos_framework::named_addresses().clone())
        .collect();

        let base_dir = format!("../execution/src/tests/res/{package_name}").replace('_', "-");
        let targets = vec![format!("{base_dir}/sources/{package_name}.move")];

        Self {
            targets_inner: targets,
            named_addresses_inner: named_address_mapping,
        }
    }
}

impl CompileJob for ModuleCompileJob {
    fn targets(&self) -> Vec<String> {
        self.targets_inner.clone()
    }

    fn deps(&self) -> Vec<String> {
        let mut framework = aptos_framework::testnet_release_bundle()
            .files()
            .expect("Must be able to find Aptos Framework files");
        let genesis_base = "../genesis-builder/framework/aptos-framework/sources";
        framework.append(&mut vec![
            format!("{genesis_base}/fungible_asset_u256.move"),
            format!("{genesis_base}/primary_fungible_store_u256.move"),
        ]);
        add_custom_framework_paths(&mut framework);
        framework
    }

    fn named_addresses(&self) -> BTreeMap<String, NumericalAddress> {
        self.named_addresses_inner.clone()
    }
}

pub struct ScriptCompileJob {
    targets_inner: Vec<String>,
    deps_inner: Vec<String>,
}

impl ScriptCompileJob {
    pub fn new(script_name: &str, local_deps: &[&str]) -> Self {
        let base_dir = format!("../execution/src/tests/res/{script_name}").replace('_', "-");
        let targets = vec![format!("{base_dir}/sources/{script_name}.move")];

        let local_deps = local_deps.iter().map(|package_name| {
            let base_dir = format!("../execution/src/tests/res/{package_name}").replace('_', "-");
            format!("{base_dir}/sources/{package_name}.move")
        });
        let deps = {
            let mut framework = aptos_framework::testnet_release_bundle()
                .files()
                .expect("Must be able to find Aptos Framework files");
            let genesis_base = "../genesis-builder/framework/aptos-framework/sources";
            framework.append(&mut vec![
                format!("{genesis_base}/fungible_asset_u256.move"),
                format!("{genesis_base}/primary_fungible_store_u256.move"),
            ]);

            add_custom_framework_paths(&mut framework);
            local_deps.for_each(|d| framework.push(d));

            framework
        };

        Self {
            targets_inner: targets,
            deps_inner: deps,
        }
    }
}

impl CompileJob for ScriptCompileJob {
    fn targets(&self) -> Vec<String> {
        self.targets_inner.clone()
    }

    fn deps(&self) -> Vec<String> {
        self.deps_inner.clone()
    }

    fn named_addresses(&self) -> BTreeMap<String, NumericalAddress> {
        let mut result = aptos_framework::named_addresses().clone();
        for (name, address) in custom_framework_named_addresses() {
            result.insert(name, address);
        }
        result
    }
}

/// Helper function to get custom framework named addresses
///
/// # Returns
/// Iterator of framework name and address pairs
fn custom_framework_named_addresses() -> impl Iterator<Item = (String, NumericalAddress)> {
    let mut named_addresses = vec![
        (
            "EthToken".to_string(),
            NumericalAddress::parse_str("0x1").unwrap(),
        ),
        ("Evm".into(), NumericalAddress::parse_str("0x1").unwrap()),
        (
            "evm_admin".to_string(),
            NumericalAddress::parse_str("0x1").unwrap(),
        ),
    ];
    named_addresses.append(
        &mut get_l2_contracts()
            .into_iter()
            .map(|(name, address)| (name, NumericalAddress::parse_str(&address).unwrap()))
            .collect::<Vec<_>>(),
    );
    named_addresses.into_iter()
}

/// Adds custom framework paths to dependencies
///
/// # Arguments
/// * `files` - Vector to add framework paths to
fn add_custom_framework_paths(files: &mut Vec<String>) {
    add_framework_path("eth-token", "EthToken", files);
    add_framework_path("evm", "Evm", files);
    get_l2_contracts().iter().for_each(|(name, _)| {
        add_framework_path("l2", name, files);
    });
}

/// Adds an individual framework path in genesis builder to the dependency list
///
/// # Arguments
/// * `folder_name` - Name of the framework folder
/// * `source_name` - Name of the source file
/// * `files` - Vector to add the path to
fn add_framework_path(folder_name: &str, source_name: &str, files: &mut Vec<String>) {
    let base_path = Path::new(std::env!("CARGO_MANIFEST_DIR"));
    let eth_token_path = base_path
        .join(format!(
            "../genesis-builder/framework/{folder_name}/sources/{source_name}.move"
        ))
        .canonicalize()
        .unwrap();
    files.push(eth_token_path.to_string_lossy().into());
}

fn get_l2_contracts() -> Vec<(String, String)> {
    let move_toml = read_to_string("../genesis-builder/framework/l2/Move.toml").unwrap();
    // Capture the contract name where the address starts with 0x42
    let mut names_and_addresses = Vec::new();
    let re = Regex::new("^(?<name>.*) = \"(?<address>0x42.*)\"$").unwrap();
    for line in move_toml.lines() {
        if re.is_match(line) {
            names_and_addresses.push((
                re.replace(line, "$name").to_string(),
                re.replace(line, "$address").to_string(),
            ));
        }
    }
    names_and_addresses
}
