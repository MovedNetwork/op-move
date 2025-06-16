use {
    crate::{declaration::OptionalConfig, stack::Layer},
    serde::de::Error,
    std::{
        error::Error as StdError,
        fs::File,
        io::{self, Read},
        path::Path,
    },
};

#[derive(Debug, Clone)]
pub struct FileLayer<Parser> {
    path: Box<Path>,
    parser: Parser,
}

impl<Parser> FileLayer<Parser> {
    pub fn new(path: impl AsRef<Path>, parser: Parser) -> Self {
        Self {
            path: path.as_ref().into(),
            parser,
        }
    }
}

impl FileLayer<()> {
    pub fn toml() -> FileLayer<impl FnOnce(Box<Path>) -> Result<OptionalConfig, toml::de::Error>> {
        FileLayer::new("Config.toml", |path| match File::open(path) {
            Ok(mut file) => {
                let mut toml = String::new();
                file.read_to_string(&mut toml)
                    .map_err(toml::de::Error::custom)?;
                toml::from_str(toml.as_str())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(OptionalConfig::default()),
            Err(e) => Err(toml::de::Error::custom(e)),
        })
    }
}

impl<F: FnOnce(Box<Path>) -> Result<OptionalConfig, Err>, Err: StdError> Layer for FileLayer<F> {
    type Err = Err;

    fn try_load(self) -> Result<OptionalConfig, Self::Err> {
        (self.parser)(self.path)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::convert::Infallible};

    #[test]
    fn test_file_layer_delegates_to_parser() {
        let parser = |path: Box<Path>| -> Result<OptionalConfig, Infallible> {
            assert_eq!(path.as_ref(), Path::new("test"));
            Ok(OptionalConfig::default())
        };
        let layer = FileLayer::new("test", parser);
        let actual_config = layer.try_load().unwrap();
        let expected_config = OptionalConfig::default();

        assert_eq!(actual_config, expected_config);
    }
}
