use {
    crate::generate::{generate_erc20_contracts, l2_abi_to_move},
    aptos_framework::{BuildOptions, BuiltPackage, ReleaseBundle, ReleasePackage},
    move_package::{
        package_hooks::register_package_hooks, BuildConfig as MoveBuildConfig, LintFlag,
    },
    once_cell::sync::Lazy,
    std::{
        collections::BTreeMap,
        fs::{copy, read_to_string, remove_dir_all, write, OpenOptions},
        io::{BufRead, BufReader},
        path::PathBuf,
        process::Command,
    },
    sui_framework::SystemPackage,
    sui_move_build::{BuildConfig, SuiPackageHooks},
    sui_types::base_types::ObjectID,
};

mod generate;

pub const APTOS_SNAPSHOT_NAME: &str = "aptos.mrb";
pub const SUI_SNAPSHOT_NAME: &str = "sui.mrb";
pub const MOVE_STDLIB_FOLDER_NAME: &str = "move-stdlib";
pub const SUI_FRAMEWORK_FOLDER_NAME: &str = "sui-framework";
pub const SUI_STDLIB_PACKAGE_ID: ObjectID = small_object_id(0x21);
pub const SUI_FRAMEWORK_PACKAGE_ID: ObjectID = small_object_id(0x22);

const APTOS_REPO: &str = "https://github.com/aptos-labs/aptos-core";
const APTOS_REPO_TAG: &str = "aptos-node-v1.14.0";
const ORIGINAL_SUI_STDLIB_PACKAGE_ID: ObjectID = small_object_id(0x1);
const ORIGINAL_SUI_FRAMEWORK_PACKAGE_ID: ObjectID = small_object_id(0x2);
const SUI_REPO: &str = "https://github.com/MystenLabs/sui";
const SUI_REPO_TAG: &str = "testnet-v1.28.3";
const MOVE_TOML: &str = "Move.toml";

const TOKEN_LIST_REPO: &str = "https://github.com/ethereum-optimism/ethereum-optimism.github.io";

static TARGET_ROOT: Lazy<PathBuf> = Lazy::new(|| {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Workspace root directory should exist")
        .join("target")
});

static BUILDER_ROOT: Lazy<PathBuf> = Lazy::new(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));

static MOVED_FRAMEWORK_DIR: Lazy<PathBuf> =
    Lazy::new(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("framework"));

static APTOS_DIR: Lazy<PathBuf> = Lazy::new(|| TARGET_ROOT.join("aptos-core"));
static APTOS_FRAMEWORK_DIR: Lazy<PathBuf> =
    Lazy::new(|| APTOS_DIR.join("aptos-move").join("framework"));
static APTOS_PACKAGE_PATHS: Lazy<Vec<PathBuf>> = Lazy::new(|| {
    vec![
        APTOS_FRAMEWORK_DIR.join("move-stdlib"),
        APTOS_FRAMEWORK_DIR.join("aptos-stdlib"),
        APTOS_FRAMEWORK_DIR.join("aptos-framework"),
        APTOS_FRAMEWORK_DIR.join("aptos-token"),
        APTOS_FRAMEWORK_DIR.join("aptos-token-objects"),
        MOVED_FRAMEWORK_DIR.join("eth-token"),
        MOVED_FRAMEWORK_DIR.join("evm"),
        MOVED_FRAMEWORK_DIR.join("l2"),
        MOVED_FRAMEWORK_DIR.join("erc20"),
    ]
});
static APTOS_ADDRESS_MAPPING: Lazy<BTreeMap<&str, &str>> = Lazy::new(|| {
    BTreeMap::from([
        ("\"0x0\"", "\"0x10\""),
        // TODO: Map std, aptos_std and aptos_framework address from 0x1 to 0x11
        ("\"0x3\"", "\"0x13\""),
        ("\"0x4\"", "\"0x14\""),
        ("\"0xA\"", "\"0x1A\""),
        ("\"0xA550C18\"", "\"0x15\""),
    ])
});

static SUI_DIR: Lazy<PathBuf> = Lazy::new(|| TARGET_ROOT.join("sui"));
static SUI_FRAMEWORK_DIR: Lazy<PathBuf> = Lazy::new(|| {
    SUI_DIR
        .join("crates")
        .join("sui-framework")
        .join("packages")
});
static SUI_COIN_PATH: Lazy<PathBuf> = Lazy::new(|| {
    SUI_FRAMEWORK_DIR
        .join(SUI_FRAMEWORK_FOLDER_NAME)
        .join("sources")
        .join("coin.move")
});

static TOKEN_LIST_DIR: Lazy<PathBuf> = Lazy::new(|| TARGET_ROOT.join("superchain_token_list"));

static OPTIMISM_BEDROCK_DIR: Lazy<PathBuf> = Lazy::new(|| {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Workspace root directory should exist")
        .join("server/src/tests/optimism/packages/contracts-bedrock/snapshots/abi/")
});

const fn small_object_id(value: u8) -> ObjectID {
    ObjectID::from_single_byte(value)
}

fn main() -> anyhow::Result<()> {
    println!("Starting genesis package generation");
    register_package_hooks(Box::new(SuiPackageHooks));

    clone_repos()?;

    generate_erc20_contracts()?;

    fix_aptos_packages()?;
    l2_abi_to_move()?;
    build_aptos_packages()?;

    fix_sui_packages()?;
    build_sui_packages()?;
    Ok(())
}

fn clone_repos() -> anyhow::Result<()> {
    // Always start with fresh copies of framework and token list repos
    if APTOS_DIR.try_exists()? {
        remove_dir_all(APTOS_DIR.as_path())?;
    }
    if SUI_DIR.try_exists()? {
        remove_dir_all(SUI_DIR.as_path())?;
    }
    if TOKEN_LIST_DIR.try_exists()? {
        remove_dir_all(TOKEN_LIST_DIR.as_path())?;
    }

    // Clone the Aptos and Sui repos which contain the framework packages
    Command::new("git")
        .current_dir(TARGET_ROOT.as_path())
        .args(["clone", "--depth", "1", "-b", APTOS_REPO_TAG, APTOS_REPO])
        .output()?;
    Command::new("git")
        .current_dir(TARGET_ROOT.as_path())
        .args(["clone", "--depth", "1", "-b", SUI_REPO_TAG, SUI_REPO])
        .output()?;
    // Clone the superchain ERC20 token list
    Command::new("git")
        .current_dir(TARGET_ROOT.as_path())
        .args([
            "clone",
            "--depth",
            "1",
            TOKEN_LIST_REPO,
            TOKEN_LIST_DIR.file_name().unwrap().to_str().unwrap(),
        ])
        .output()?;
    Ok(())
}

fn fix_aptos_packages() -> anyhow::Result<()> {
    // Addresses are mapped from 0x1, 0x3, 0x4 to 0x11, 0x13, 0x14 etc for conflict resolution
    for path in APTOS_PACKAGE_PATHS.iter() {
        let move_toml_file = &path.join(MOVE_TOML);
        let mut content = read_to_string(move_toml_file)?;
        for (from, to) in APTOS_ADDRESS_MAPPING.iter() {
            content = content.replace(from, to);
        }
        write(move_toml_file, content)?;
    }

    // Object module should also friend the new u256 version of the fungible store
    let object_move_file = &APTOS_FRAMEWORK_DIR
        .join("aptos-framework")
        .join("sources")
        .join("object.move");
    let mut content = read_to_string(object_move_file)?;
    content = content.replace(
        "friend aptos_framework::primary_fungible_store;",
        "friend aptos_framework::primary_fungible_store;\n    friend aptos_framework::primary_fungible_store_u256;"
    );
    write(object_move_file, content)?;

    // Include the u256 version of fungible asset and store modules into the aptos_framework
    copy_aptos_file("fungible_asset_u256.move")?;
    copy_aptos_file("primary_fungible_store_u256.move")?;
    Ok(())
}

fn build_aptos_packages() -> anyhow::Result<()> {
    // Build the framework packages
    let packages = APTOS_PACKAGE_PATHS
        .iter()
        .map(|path| {
            ReleasePackage::new(
                BuiltPackage::build(path.to_owned(), BuildOptions::default())
                    .expect("Aptos package failed to build"),
            )
            .expect("Release package generation failed")
        })
        .collect::<Vec<ReleasePackage>>();
    // Save the packages as a bundle into a single file
    let bundle = ReleaseBundle::new(
        packages,
        APTOS_PACKAGE_PATHS
            .iter()
            .map(|path| path.to_string_lossy().clone().to_string())
            .collect(),
    );
    let binary = bcs::to_bytes(&bundle)?;
    write(TARGET_ROOT.join(APTOS_SNAPSHOT_NAME), binary)?;
    println!("Generated Aptos snapshot bundle");
    Ok(())
}

fn fix_sui_packages() -> anyhow::Result<()> {
    // Addresses are mapped from 0x1, 0x2 to 0x21, 0x22 for conflict resolution
    let move_toml_file = &SUI_FRAMEWORK_DIR
        .join(MOVE_STDLIB_FOLDER_NAME)
        .join(MOVE_TOML);
    let content = read_to_string(move_toml_file)?;
    let content = content.replace(
        &ORIGINAL_SUI_STDLIB_PACKAGE_ID.to_hex_literal(),
        &SUI_STDLIB_PACKAGE_ID.to_hex_literal(),
    );
    write(move_toml_file, content)?;
    let move_toml_file = &SUI_FRAMEWORK_DIR
        .join(SUI_FRAMEWORK_FOLDER_NAME)
        .join(MOVE_TOML);
    let content = read_to_string(move_toml_file)?;
    let content = content.replace(
        &ORIGINAL_SUI_FRAMEWORK_PACKAGE_ID.to_hex_literal(),
        &SUI_FRAMEWORK_PACKAGE_ID.to_hex_literal(),
    );
    write(move_toml_file, content)?;

    // Remove #[deprecated(..)] annotations as they cause Move compilation issues
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(SUI_COIN_PATH.as_path())
        .expect("coin.move file doesn't exist");
    let lines = BufReader::new(file)
        .lines()
        .map(|line| line.unwrap())
        .filter(|line| !line.contains("#[deprecated"))
        .collect::<Vec<String>>()
        .join("\n");
    write(SUI_COIN_PATH.as_path(), lines).expect("Can't write to `coin` module");
    Ok(())
}

fn build_sui_packages() -> anyhow::Result<()> {
    let config = MoveBuildConfig {
        generate_docs: true,
        warnings_are_errors: true,
        install_dir: Some(PathBuf::from(".")),
        lint_flag: LintFlag::LEVEL_NONE,
        default_edition: None,
        ..Default::default()
    };
    debug_assert!(!config.test_mode);

    build_packages_with_move_config(config)?;
    Ok(())
}

fn build_packages_with_move_config(config: MoveBuildConfig) -> anyhow::Result<()> {
    let build_config = BuildConfig {
        config: config.clone(),
        run_bytecode_verifier: false,
        print_diags_to_stderr: false,
    };

    let move_stdlib_path = &SUI_FRAMEWORK_DIR.join(MOVE_STDLIB_FOLDER_NAME);
    let sui_framework_path = &SUI_FRAMEWORK_DIR.join(SUI_FRAMEWORK_FOLDER_NAME);
    let stdlib_cmp_pkg = build_config.clone().build(move_stdlib_path)?;
    let framework_cmp_pkg = build_config.build(sui_framework_path)?;

    let packages = vec![
        SystemPackage {
            id: SUI_STDLIB_PACKAGE_ID,
            bytes: stdlib_cmp_pkg.get_package_bytes(false),
            dependencies: vec![],
        },
        SystemPackage {
            id: SUI_FRAMEWORK_PACKAGE_ID,
            bytes: framework_cmp_pkg.get_package_bytes(false),
            dependencies: vec![SUI_STDLIB_PACKAGE_ID],
        },
    ];

    // Serialize packages and write to a single file
    let binary = bcs::to_bytes(&packages)?;
    write(TARGET_ROOT.join(SUI_SNAPSHOT_NAME), binary)?;
    println!("Generated Sui snapshot bundle");
    Ok(())
}

fn copy_aptos_file(name: &str) -> Result<u64, std::io::Error> {
    copy(
        MOVED_FRAMEWORK_DIR
            .join("aptos-framework")
            .join("sources")
            .join(name),
        APTOS_FRAMEWORK_DIR
            .join("aptos-framework")
            .join("sources")
            .join(name),
    )
}
