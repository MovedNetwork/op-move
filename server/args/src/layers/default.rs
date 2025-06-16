use {
    crate::{declaration::OptionalConfig, stack::Layer},
    std::convert::Infallible,
};

#[derive(Debug, Clone, Default)]
pub struct DefaultLayer(OptionalConfig);

impl DefaultLayer {
    pub const fn new(default: OptionalConfig) -> Self {
        Self(default)
    }
}

impl Layer for DefaultLayer {
    type Err = Infallible;

    fn try_load(self) -> Result<OptionalConfig, Self::Err> {
        Ok(self.0)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::declaration::{OptionalAuthSocket, OptionalHttpSocket},
    };

    #[test]
    fn test_default_layer_passes_given_config_unchanged() {
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
        let layer = DefaultLayer(expected_config.clone());
        let actual_config = layer.try_load().unwrap();

        assert_eq!(actual_config, expected_config);
    }
}
