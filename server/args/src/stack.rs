use {
    crate::declaration::{Config, OptionalConfig},
    std::{convert::Infallible, error::Error as StdError},
    thiserror::Error,
};

#[derive(Debug, Clone, Default)]
pub struct ConfigBuilder<L>(L);

impl ConfigBuilder<()> {
    pub const fn new() -> Self {
        Self(())
    }
}

pub trait Layer {
    type Err: StdError;

    fn try_load(self) -> Result<OptionalConfig, Self::Err>;
}

impl Layer for () {
    type Err = Infallible;

    fn try_load(self) -> Result<OptionalConfig, Self::Err> {
        Ok(OptionalConfig::default())
    }
}

pub struct WithLayers<L1, L2>(L1, L2);

#[derive(Debug, Clone, Error)]
pub enum WithLayerError<BackErr, FrontErr> {
    #[error(transparent)]
    Back(BackErr),
    #[error(transparent)]
    Front(FrontErr),
}

impl<BackErr, FrontErr> From<Infallible> for WithLayerError<BackErr, FrontErr> {
    fn from(value: Infallible) -> Self {
        match value {}
    }
}

impl<Back: Layer, Front: Layer> Layer for WithLayers<Back, Front> {
    type Err = WithLayerError<Back::Err, Front::Err>;

    fn try_load(self) -> Result<OptionalConfig, Self::Err> {
        Ok(self
            .0
            .try_load()
            .map_err(WithLayerError::Back)?
            .apply(self.1.try_load().map_err(WithLayerError::Front)?))
    }
}

impl<L> ConfigBuilder<L> {
    pub fn layer<L2: Layer>(self, layer: L2) -> ConfigBuilder<WithLayers<L, L2>> {
        ConfigBuilder(WithLayers(self.0, layer))
    }
}

impl<L: Layer> ConfigBuilder<L> {
    pub fn try_build(self) -> Result<Config, Box<dyn StdError>>
    where
        <L as Layer>::Err: 'static,
    {
        Ok(self.0.try_load()?.try_into()?)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::declaration::{AuthSocket, HttpSocket, OptionalAuthSocket, OptionalHttpSocket},
    };

    pub struct StubLayer(OptionalConfig);

    impl Layer for StubLayer {
        type Err = Infallible;

        fn try_load(self) -> Result<OptionalConfig, Self::Err> {
            Ok(self.0)
        }
    }

    #[test]
    fn test_second_layer_overrides_first_layer() {
        let auth_addr = "0.0.0.0:11".parse().unwrap();
        let overridden_http_addr = "0.0.0.0:1".parse().unwrap();
        let http_addr = "0.0.0.0:2".parse().unwrap();
        let actual_config = ConfigBuilder::new()
            .layer(StubLayer(OptionalConfig {
                auth: Some(OptionalAuthSocket {
                    addr: Some(auth_addr),
                    jwt_secret: Some(String::new()),
                }),
                http: Some(OptionalHttpSocket {
                    addr: Some(overridden_http_addr),
                }),
            }))
            .layer(StubLayer(OptionalConfig {
                auth: None,
                http: Some(OptionalHttpSocket {
                    addr: Some(http_addr),
                }),
            }))
            .try_build()
            .unwrap();
        let expected_config = Config {
            auth: AuthSocket {
                addr: auth_addr,
                jwt_secret: String::new(),
            },
            http: HttpSocket { addr: http_addr },
        };

        assert_eq!(actual_config, expected_config);
    }
}
