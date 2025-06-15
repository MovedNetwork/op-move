use {
    crate::{declaration::OptionalConfig, stack::Layer},
    std::{env, env::Vars},
};

#[derive(Debug, Clone, Default)]
pub struct EnvLayer<Vars>(Vars);

impl EnvLayer<Vars> {
    pub fn new() -> Self {
        Self(env::vars())
    }
}

impl<Vars: IntoIterator<Item = (K, K)>, K: AsRef<str>> Layer for EnvLayer<Vars> {
    type Err = serde_env::Error;

    fn try_load(self) -> Result<OptionalConfig, Self::Err> {
        serde_env::from_iter_with_prefix(self.0, "OP_MOVE")
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::declaration::{OptionalAuthSocket, OptionalHttpSocket},
    };

    #[test]
    fn test_env_layer_parses_prefixed_key_value_pairs() {
        let layer = EnvLayer(vec![
            ("OP_MOVE_HTTP_ADDR", "0.0.0.0:1"),
            ("OP_MOVE_AUTH_ADDR", "0.0.0.0:2"),
            ("OP_MOVE_AUTH_JWT_SECRET", "test"),
        ]);
        let actual_config = layer.try_load().unwrap();
        let expected_config = OptionalConfig {
            auth: Some(OptionalAuthSocket {
                addr: "0.0.0.0:2".parse().ok(),
                jwt_secret: Some("test".to_owned()),
            }),
            http: Some(OptionalHttpSocket {
                addr: "0.0.0.0:1".parse().ok(),
            }),
        };

        assert_eq!(actual_config, expected_config);
    }
}
