use {
    alloy::json_abi::{InternalType, JsonAbi, StateMutability},
    handlebars::{handlebars_helper, Handlebars},
    regex::Regex,
    serde::Serialize,
    std::{
        collections::BTreeMap,
        fs::{read_dir, read_to_string, File},
        path::Path,
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

handlebars_helper!(capitalize: |s: String| {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
});

handlebars_helper!(slug: |s: String| {
    // ALL_CAPS variables are used as is
    if s.to_uppercase() == s {
        s
    } else {
        let mut result = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && c.is_uppercase()  {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        }
        // Reverse back abbreviations, like e_r_c_20 to ERC20
        result = result.replace("e_t_h", "eth");
        result = result.replace("e_r_c20", "erc20");
        result = result.replace("e_r_c721", "erc721");
        result
    }
});

pub fn l2_abi_to_move() -> anyhow::Result<()> {
    println!("Converting L2 Solidity ABIs to Move modules");
    let directory_path = "server/src/tests/optimism/packages/contracts-bedrock/snapshots/abi/";
    let directory = read_dir(directory_path)?;
    let l2_contract_names = get_l2_contract_names()?;

    let mut handlebars = Handlebars::new();
    handlebars.register_helper("capitalize", Box::new(capitalize));
    handlebars.register_helper("slug", Box::new(slug));

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

        let functions = abi
            .functions
            .into_iter()
            .flat_map(|(name, funs)| {
                // Solidity supports function overloading, but it doesn't exist in L2 contracts.
                funs.iter()
                    .map(|fun| {
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

                        function
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        let mut path = Path::new("genesis-builder/framework/l2/sources").join(&name);
        path.set_extension("move");
        let mut output_file = File::create(path)?;
        handlebars.register_template_file("move", "genesis-builder/l2_move_template.hbs")?;

        let module = L2Module {
            name,
            functions,
            structs,
            has_fungible_asset,
        };
        handlebars.render_to_write("move", &module, &mut output_file)?;
    }

    Ok(())
}

fn get_l2_contract_names() -> anyhow::Result<Vec<String>> {
    let move_toml = read_to_string("genesis-builder/framework/l2/Move.toml")?;
    // Capture the contract name where the address starts with 0x42
    let mut names = Vec::new();
    let re = Regex::new("^(?<name>.*) = \"0x42\\d+\"$")?;
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
        // TODO: Use fixed byte array for bytesN
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
