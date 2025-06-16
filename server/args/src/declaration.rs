use {serde::Deserialize, std::net::SocketAddr};

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

#[derive(Deserialize, PartialEq, Debug, Clone)]
pub struct OptionalConfig {
    pub auth: Option<OptionalAuthSocket>,
    pub http: Option<OptionalHttpSocket>,
}

#[derive(Deserialize, PartialEq, Debug, Clone)]
pub struct OptionalAuthSocket {
    pub addr: Option<SocketAddr>,
    pub jwt_secret: Option<String>,
}

#[derive(Deserialize, PartialEq, Debug, Clone)]
pub struct OptionalHttpSocket {
    pub addr: Option<SocketAddr>,
}
