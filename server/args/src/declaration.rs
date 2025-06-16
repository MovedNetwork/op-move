use {
    clap::{Args, Parser},
    serde::Deserialize,
    std::net::SocketAddr,
    thiserror::Error,
};

#[derive(PartialEq, Debug, Clone)]
pub struct Config {
    pub auth: AuthSocket,
    pub http: HttpSocket,
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

#[derive(Deserialize, Parser, PartialEq, Debug, Clone, Default)]
pub struct OptionalConfig {
    #[command(flatten)]
    pub auth: Option<OptionalAuthSocket>,
    #[command(flatten)]
    pub http: Option<OptionalHttpSocket>,
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

#[derive(Debug, Clone, Error)]
#[error("Missing field `{0}`")]
pub struct MissingField(&'static str);

impl TryFrom<OptionalConfig> for Config {
    type Error = MissingField;

    fn try_from(value: OptionalConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            auth: value.auth.ok_or(MissingField("auth"))?.try_into()?,
            http: value.http.ok_or(MissingField("http"))?.try_into()?,
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

impl OptionalConfig {
    pub fn apply(mut self, other: Self) -> Self {
        self.auth = match (self.auth, other.auth) {
            (Some(ours), Some(theirs)) => Some(ours.apply(theirs)),
            (ours, theirs) => theirs.or(ours),
        };
        self.http = match (self.http, other.http) {
            (Some(ours), Some(theirs)) => Some(ours.apply(theirs)),
            (ours, theirs) => theirs.or(ours),
        };
        self
    }
}

impl OptionalAuthSocket {
    pub fn apply(mut self, other: Self) -> Self {
        self.addr = other.addr.or(self.addr);
        self.jwt_secret = other.jwt_secret.or(self.jwt_secret);
        self
    }
}

impl OptionalHttpSocket {
    pub fn apply(mut self, other: Self) -> Self {
        self.addr = other.addr.or(self.addr);
        self
    }
}
