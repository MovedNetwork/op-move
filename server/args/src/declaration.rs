use {
    clap::{Args, Parser, ValueEnum},
    serde::Deserialize,
    std::{fmt::Debug, net::SocketAddr, path::Path},
    thiserror::Error,
    umi_shared::primitives::{MoveAddress, B256},
};

#[derive(PartialEq, Debug, Clone)]
pub struct Config {
    pub auth: AuthSocket,
    pub http: HttpSocket,
    pub db: Database,
    pub genesis: Genesis,
    pub max_buffered_commands: u32,
}

#[derive(PartialEq, Debug, Clone)]
pub struct AuthSocket {
    pub addr: SocketAddr,
    pub jwt_secret: String,
}

#[derive(PartialEq, Debug, Clone)]
pub struct HttpSocket {
    pub addr: SocketAddr,
}

#[derive(PartialEq, Debug, Clone)]
pub struct Database {
    /// TODO: Currently a dummy, either make it work or remove it
    pub backend: DatabaseBackend,
    pub dir: Box<Path>,
    pub purge: bool,
}

#[derive(PartialEq, Debug, Clone)]
pub struct Genesis {
    pub chain_id: u64,
    pub initial_state_root: B256,
    pub treasury: MoveAddress,
    pub l2_contract_genesis: Box<Path>,
    pub token_list: Box<Path>,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            backend: DatabaseBackend::InMemory,
            dir: Path::new("db").into(),
            purge: false,
        }
    }
}

#[derive(Deserialize, Parser, PartialEq, Debug, Clone, Default)]
pub struct OptionalConfig {
    #[command(flatten)]
    pub auth: Option<OptionalAuthSocket>,
    #[command(flatten)]
    pub http: Option<OptionalHttpSocket>,
    #[command(flatten)]
    pub db: Option<OptionalDatabase>,
    #[command(flatten)]
    pub genesis: Option<OptionalGenesis>,
    #[arg(long)]
    pub max_buffered_commands: Option<u32>,
}

#[derive(Deserialize, Args, PartialEq, Debug, Clone, Default)]
pub struct OptionalAuthSocket {
    #[arg(long = "auth.addr", id = "auth.addr")]
    pub addr: Option<SocketAddr>,
    #[arg(long = "auth.jwt-secret", id = "auth.jwt-secret")]
    pub jwt_secret: Option<String>,
}

#[derive(Deserialize, Args, PartialEq, Debug, Clone, Default)]
pub struct OptionalHttpSocket {
    #[arg(long = "http.addr", id = "http.addr")]
    pub addr: Option<SocketAddr>,
}

#[derive(Deserialize, Parser, PartialEq, Debug, Clone, Default)]
pub struct OptionalDatabase {
    /// TODO: Currently a dummy, either make it work or remove it
    #[arg(long = "db.backend", id = "db.backend")]
    pub backend: Option<DatabaseBackend>,
    #[arg(long = "db.dir", id = "db.dir")]
    pub dir: Option<Box<Path>>,
    #[arg(long = "db.purge", id = "db.purge")]
    pub purge: Option<bool>,
}

#[derive(Deserialize, Parser, PartialEq, Debug, Clone, Default)]
pub struct OptionalGenesis {
    #[arg(long = "genesis.chain-id", id = "genesis.chain-id")]
    pub chain_id: Option<u64>,
    #[arg(long = "genesis.initial-state-root", id = "genesis.initial-state-root")]
    pub initial_state_root: Option<B256>,
    #[arg(long = "genesis.treasury", id = "genesis.treasury")]
    pub treasury: Option<MoveAddress>,
    #[arg(
        long = "genesis.l2-contract-genesis",
        id = "genesis.l2-contract-genesis"
    )]
    pub l2_contract_genesis: Option<Box<Path>>,
    #[arg(long = "genesis.token-list", id = "genesis.token-list")]
    pub token_list: Option<Box<Path>>,
}

#[derive(Deserialize, ValueEnum, PartialEq, Debug, Clone)]
pub enum DatabaseBackend {
    InMemory,
    RocksDb,
    Lmdb,
}

#[derive(Debug, Clone, Error)]
#[error("Missing field `{0}`")]
pub struct MissingField(&'static str);

impl TryFrom<OptionalConfig> for Config {
    type Error = MissingField;

    fn try_from(value: OptionalConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            auth: value.auth.ok_or(MissingField("auth"))?.try_into()?,
            http: value.http.ok_or(MissingField("http"))?.try_into()?,
            db: value.db.ok_or(MissingField("db"))?.try_into()?,
            genesis: value.genesis.ok_or(MissingField("genesis"))?.try_into()?,
            max_buffered_commands: value
                .max_buffered_commands
                .ok_or(MissingField("max_buffered_commands"))?,
        })
    }
}

impl TryFrom<OptionalAuthSocket> for AuthSocket {
    type Error = MissingField;

    fn try_from(value: OptionalAuthSocket) -> Result<Self, Self::Error> {
        Ok(Self {
            addr: value.addr.ok_or(MissingField("auth.addr"))?,
            jwt_secret: value.jwt_secret.ok_or(MissingField("auth.jwt_secret"))?,
        })
    }
}

impl TryFrom<OptionalHttpSocket> for HttpSocket {
    type Error = MissingField;

    fn try_from(value: OptionalHttpSocket) -> Result<Self, Self::Error> {
        Ok(Self {
            addr: value.addr.ok_or(MissingField("http.addr"))?,
        })
    }
}

impl TryFrom<OptionalDatabase> for Database {
    type Error = MissingField;

    fn try_from(value: OptionalDatabase) -> Result<Self, Self::Error> {
        Ok(Self {
            backend: value.backend.ok_or(MissingField("db.backend"))?,
            dir: value.dir.ok_or(MissingField("db.dir"))?,
            purge: value.purge.ok_or(MissingField("db.purge"))?,
        })
    }
}

impl TryFrom<OptionalGenesis> for Genesis {
    type Error = MissingField;

    fn try_from(value: OptionalGenesis) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id: value.chain_id.ok_or(MissingField("genesis.chain-id"))?,
            initial_state_root: value
                .initial_state_root
                .ok_or(MissingField("genesis.initial-state-root"))?,
            treasury: value.treasury.ok_or(MissingField("genesis.treasury"))?,
            l2_contract_genesis: value
                .l2_contract_genesis
                .ok_or(MissingField("genesis.l2-contract-genesis"))?,
            token_list: value.token_list.ok_or(MissingField("genesis.token-list"))?,
        })
    }
}

impl OptionalConfig {
    pub fn apply(mut self, other: Self) -> Self {
        let Self {
            auth,
            http,
            db,
            genesis,
            max_buffered_commands,
        } = other;

        self.auth = match (self.auth, auth) {
            (Some(ours), Some(theirs)) => Some(ours.apply(theirs)),
            (ours, theirs) => theirs.or(ours),
        };
        self.http = match (self.http, http) {
            (Some(ours), Some(theirs)) => Some(ours.apply(theirs)),
            (ours, theirs) => theirs.or(ours),
        };
        self.db = match (self.db, db) {
            (Some(ours), Some(theirs)) => Some(ours.apply(theirs)),
            (ours, theirs) => theirs.or(ours),
        };
        self.genesis = match (self.genesis, genesis) {
            (Some(ours), Some(theirs)) => Some(ours.apply(theirs)),
            (ours, theirs) => theirs.or(ours),
        };
        self.max_buffered_commands = max_buffered_commands.or(self.max_buffered_commands);

        self
    }
}

impl OptionalAuthSocket {
    pub fn apply(mut self, other: Self) -> Self {
        let Self { addr, jwt_secret } = other;

        self.addr = addr.or(self.addr);
        self.jwt_secret = jwt_secret.or(self.jwt_secret);

        self
    }
}

impl OptionalHttpSocket {
    pub fn apply(mut self, other: Self) -> Self {
        let Self { addr } = other;

        self.addr = addr.or(self.addr);

        self
    }
}

impl OptionalDatabase {
    pub fn apply(mut self, other: Self) -> Self {
        let Self {
            backend,
            dir,
            purge,
        } = other;

        self.purge = purge.or(self.purge);
        self.dir = dir.or(self.dir);
        self.backend = backend.or(self.backend);

        self
    }
}

impl OptionalGenesis {
    pub fn apply(mut self, other: Self) -> Self {
        let Self {
            chain_id,
            initial_state_root,
            treasury,
            l2_contract_genesis,
            token_list,
        } = other;

        self.chain_id = chain_id.or(self.chain_id);
        self.initial_state_root = initial_state_root.or(self.initial_state_root);
        self.treasury = treasury.or(self.treasury);
        self.l2_contract_genesis = l2_contract_genesis.or(self.l2_contract_genesis);
        self.token_list = token_list.or(self.token_list);

        self
    }
}
