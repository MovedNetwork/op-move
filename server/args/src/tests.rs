use crate::declaration::{OptionalAuthSocket, OptionalConfig, OptionalHttpSocket};

#[test]
fn test_config_parses_from_toml_successfully() {
    let actual_config: OptionalConfig = toml::from_str(
        r#"
        [auth]
        addr = '127.0.0.1:444'
        jwt_secret = 'aaa'

        [http]
        addr = '127.0.0.1:445'
    "#,
    )
    .unwrap();
    let expected_config = OptionalConfig {
        auth: Some(OptionalAuthSocket {
            addr: Some("127.0.0.1:444".parse().unwrap()),
            jwt_secret: Some("aaa".to_owned()),
        }),
        http: Some(OptionalHttpSocket {
            addr: Some("127.0.0.1:445".parse().unwrap()),
        }),
    };

    assert_eq!(actual_config, expected_config);
}
