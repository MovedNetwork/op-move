use {
    crate::BUILDER_ROOT,
    alloy::json_abi::{InternalType, JsonAbi, StateMutability},
    anyhow::Context,
    convert_case::{Case, Casing},
    handlebars::{handlebars_helper, Handlebars},
    regex::Regex,
    serde::{Deserialize, Serialize},
    std::{
        collections::BTreeMap,
        fs::{read_dir, read_to_string, File},
    },
};

#[derive(Serialize)]
struct L2Module {
    name: String,
    functions: Vec<L2Function>,
    structs: BTreeMap<String, Vec<L2Input>>,
    has_fungible_asset: bool,
}

#[derive(Default, Serialize)]
struct L2Function {
    name: String,
    selector: [u8; 4],
    inputs: Vec<L2Input>,
    has_value: bool,
}

#[derive(Serialize)]
struct L2Input {
    name: String,
    ty: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TokenData {
    name: String,
    symbol: String,
    decimals: u32,
    website: Option<String>,
    /// Absent in source json, filled during iterations
    logo_uri: Option<String>,
    tokens: TokenChains,
}

#[derive(Debug, Deserialize, Serialize)]
struct TokenChains {
    ethereum: Option<ChainAddress>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChainAddress {
    address: String,
}

handlebars_helper!(pascal: |s: String| s.to_case(Case::Pascal));

handlebars_helper!(snake: |s: String| to_snake_case(s));

pub fn l2_abi_to_move() -> anyhow::Result<()> {
    println!("Converting L2 Solidity ABIs to Move modules");
    let directory =
        read_dir(crate::OPTIMISM_BEDROCK_DIR.as_path()).context("No bedrock dir for abi found")?;
    let l2_contract_names = get_l2_contract_names()?;

    let mut handlebars = Handlebars::new();
    handlebars.register_helper("pascal", Box::new(pascal));
    handlebars.register_helper("snake", Box::new(snake));

    for file in directory {
        let file_path = file?.path();
        let filename = file_path.file_stem().expect("ABI file should exist");
        let name = String::from(filename.to_string_lossy());

        if !l2_contract_names.contains(&name) {
            continue;
        }

        let json = read_to_string(file_path)?;
        let abi: JsonAbi = serde_json::from_str(&json)?;

        let mut structs = BTreeMap::new();
        // If `FungibleAsset` should be imported in the Move file
        let mut has_fungible_asset = false;

        let mut functions = Vec::new();
        let mut unique_function_names = Vec::new();
        for (name, funs) in abi.functions {
            // Solidity supports function overloading, but it doesn't exist in L2 contracts.
            for fun in funs {
                let function_name = to_snake_case(name.clone());
                if unique_function_names.contains(&function_name) {
                    continue;
                }
                unique_function_names.push(function_name);

                let mut function = L2Function {
                    name: name.clone(),
                    selector: fun.selector().0,
                    ..Default::default()
                };

                if fun.state_mutability == StateMutability::Payable {
                    function.has_value = true;
                    has_fungible_asset = true;
                }

                function.inputs = Vec::new();
                fun.inputs.iter().for_each(|input| {
                    let mut name = input.name.trim_start_matches("_").to_string();
                    // Solidity `mapping` leaves out the input name. Fill in a custom name `key`.
                    if name.is_empty() {
                        // Double-mapping (map of a map) has 2 keys
                        if fun.inputs.len() > 1 && !function.inputs.is_empty() {
                            name = "key2".to_string();
                        } else {
                            name = "key".to_string();
                        }
                    }

                    let ty = if input.ty.eq("tuple") {
                        // Complex struct input type given as tuple
                        let Some(tuple) = input.clone().internal_type else {
                            unreachable!("Internal type should exist for tuples");
                        };
                        match tuple {
                            InternalType::Struct { ty, .. } => {
                                if !structs.contains_key(&ty) {
                                    // Struct components will be handled in `solidity_abi`
                                    let components = input
                                        .components
                                        .iter()
                                        .map(|c| {
                                            let name = c.name.clone();
                                            let ty = get_input_match(c.ty.clone());
                                            L2Input { name, ty }
                                        })
                                        .collect::<Vec<_>>();
                                    structs.insert(ty.clone(), components);
                                }
                                ty
                            }
                            _ => panic!("Unsupported internal type: {}", tuple),
                        }
                    } else {
                        get_input_match(input.ty.to_owned())
                    };

                    function.inputs.push(L2Input { name, ty });
                });

                functions.push(function);
            }
        }

        let mut path = BUILDER_ROOT.join("framework/l2/sources").join(&name);
        path.set_extension("move");
        let mut output_file = File::create(path)?;
        handlebars.register_template_file("l2", BUILDER_ROOT.join("l2_move_template.hbs"))?;

        let module = L2Module {
            name,
            functions,
            structs,
            has_fungible_asset,
        };
        handlebars.render_to_write("l2", &module, &mut output_file)?;
    }

    Ok(())
}

pub fn generate_erc20_contracts() -> anyhow::Result<()> {
    println!("Generating l2 erc20 contract wrappers");
    let mut handlebars = Handlebars::new();
    handlebars.register_template_file("erc20", BUILDER_ROOT.join("l2_erc20_template.hbs"))?;
    let tokens_dir = read_dir(crate::TOKEN_LIST_DIR.join("data"))?;

    for entry in tokens_dir {
        let token_folder = entry?;
        let token_file = token_folder.path().join("data.json");
        let json = read_to_string(token_file)?;
        let mut token: TokenData = serde_json::from_str(&json)?;
        // Ignoring legacy Ethereum ERC20
        if token.name == "Ether" {
            continue;
        }
        // Complying with Move identifier specs and leaving `0` of `0x..` addresses out
        if let Some(eth_address) = &mut token.tokens.ethereum {
            eth_address.address.remove(0);
        } else {
            continue;
        }
        token.logo_uri = Some(format!(
            "https://ethereum-optimism.github.io/data/{}/logo.svg",
            token_folder.path().file_stem().unwrap().to_string_lossy()
        ));
        let mut gen_path = BUILDER_ROOT
            .join("framework/erc20/sources")
            .join(&token.symbol);
        gen_path.set_extension("move");
        let mut output_file = File::create(gen_path)?;
        handlebars.render_to_write("erc20", &token, &mut output_file)?;
    }
    Ok(())
}

fn to_snake_case(s: String) -> String {
    let s = s.to_case(Case::Snake);
    // Final touch to fix split common terms
    let s = s.replace("l_1", "l1");
    let s = s.replace("l_2", "l2");
    let s = s.replace("erc_20", "erc20");
    s.replace("erc_721", "erc721")
}

fn get_l2_contract_names() -> anyhow::Result<Vec<String>> {
    let move_toml = read_to_string(BUILDER_ROOT.join("framework/l2/Move.toml"))?;
    // Capture the contract name where the address starts with 0x42
    let mut names = Vec::new();
    let re = Regex::new("^(?<name>.*) = \"0x42.*\"$")?;
    for line in move_toml.lines() {
        if re.is_match(line) {
            names.push(re.replace(line, "$name").to_string());
        }
    }
    Ok(names)
}

fn get_input_match(solidity_type: String) -> String {
    match solidity_type.as_str() {
        "address" => "address",
        "address[]" => "vector<address>",
        "bytes" => "vector<u8>",
        "bytes[]" => "vector<vector<u8>>",
        // TODO: Use SolidityFixedBytes for bytesN, ie `Evm::evm::as_fixed_bytes`
        "bytes32" => "vector<u8>",
        "bytes32[]" => "vector<vector<u8>>",
        "bytes4" => "vector<u8>",
        "string" => "vector<u8>",
        "uint256" => "u256",
        "uint128" => "u128",
        "uint64" => "u64",
        "uint32" => "u32",
        "uint8" => "u8",
        "bool" => "bool",
        ty => panic!("Unknown function input type: {}", ty),
    }
    .to_string()
}
