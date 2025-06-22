use {
    clap::{Args, Parser, ValueEnum},
    serde::Deserialize,
    std::{fmt::Debug, net::SocketAddr, path::Path},
    thiserror::Error,
};

#[derive(PartialEq, Debug, Clone)]
pub struct Config {
    pub auth: AuthSocket,
    pub http: HttpSocket,
    pub db: Database,
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

impl OptionalConfig {
    pub fn apply(mut self, other: Self) -> Self {
        let Self {
            auth,
            http,
            db,
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
