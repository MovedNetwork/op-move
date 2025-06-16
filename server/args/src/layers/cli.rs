use {
    crate::{declaration::OptionalConfig, stack::Layer},
    clap::Parser,
    std::{
        env::{self, ArgsOs},
        ffi::OsString,
    },
};

#[derive(Debug, Clone, Default)]
pub struct CliLayer<Args>(Args);

impl CliLayer<ArgsOs> {
    pub fn new() -> Self {
        Self(env::args_os())
    }
}

impl<Args: IntoIterator<Item: Into<OsString> + Clone>> Layer for CliLayer<Args> {
    type Err = clap::Error;

    fn try_load(self) -> Result<OptionalConfig, Self::Err> {
        OptionalConfig::try_parse_from(self.0)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::declaration::{OptionalAuthSocket, OptionalHttpSocket},
    };

    #[test]
    fn test_cli_layer_parses_arguments_successfully() {
        let layer = CliLayer(vec![
            "",
            "--http.addr",
            "0.0.0.0:1",
            "--auth.addr",
            "0.0.0.0:2",
            "--auth.jwt-secret",
            "test",
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
            ..Default::default()
        };

        assert_eq!(actual_config, expected_config);
    }
}
