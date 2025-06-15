use {
    clap::{Args, Parser},
    optional_struct::Applicable,
    serde::Deserialize,
    std::net::SocketAddr,
};

#[optional_struct::optional_struct]
#[derive(Deserialize, Parser, PartialEq, Debug, Clone)]
pub struct Config {
    #[optional_rename(OptionalAuthSocket)]
    #[optional_wrap]
    #[command(flatten)]
    pub auth: AuthSocket,
    #[optional_rename(OptionalHttpSocket)]
    #[optional_wrap]
    #[command(flatten)]
    pub http: HttpSocket,
}

#[optional_struct::optional_struct]
#[derive(Deserialize, Args, PartialEq, Debug, Clone)]
pub struct HttpSocket {
    #[arg(long = "http.addr", id = "http.addr")]
    pub addr: SocketAddr,
}

#[optional_struct::optional_struct]
#[derive(Deserialize, Args, PartialEq, Debug, Clone)]
pub struct AuthSocket {
    #[arg(long = "auth.addr", id = "auth.addr")]
    pub addr: SocketAddr,
    #[arg(long = "auth.jwt-secret", id = "auth.jwt-secret")]
    pub jwt_secret: String,
}
