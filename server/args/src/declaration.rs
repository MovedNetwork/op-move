use {optional_struct::Applicable, serde::Deserialize, std::net::SocketAddr};

#[optional_struct::optional_struct]
#[derive(Deserialize, PartialEq, Debug, Clone)]
pub struct Config {
    #[optional_rename(OptionalAuthSocket)]
    #[optional_wrap]
    pub auth: AuthSocket,
    #[optional_rename(OptionalHttpSocket)]
    #[optional_wrap]
    pub http: HttpSocket,
}

#[optional_struct::optional_struct]
#[derive(Deserialize, PartialEq, Debug, Clone)]
pub struct HttpSocket {
    pub addr: SocketAddr,
}

#[optional_struct::optional_struct]
#[derive(Deserialize, PartialEq, Debug, Clone)]
pub struct AuthSocket {
    pub addr: SocketAddr,
    pub jwt_secret: String,
}
