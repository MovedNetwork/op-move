use {
    crate::{declaration::OptionalConfig, stack::Layer},
    std::{error::Error as StdError, path::Path},
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
