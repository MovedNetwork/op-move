mod get_proof;
mod integration;

use {
    crate::{validate_jwt, Claims},
    aptos_types::transaction::{EntryFunction, TransactionPayload},
    jsonwebtoken::{EncodingKey, Header},
    move_core_types::{
        account_address::AccountAddress,
        ident_str,
        language_storage::{ModuleId, StructTag, TypeTag},
    },
    std::time::SystemTime,
};

#[tokio::test]
async fn test_authorized_request() -> anyhow::Result<()> {
    std::env::set_var("JWT_SECRET", "00");
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    let token = jsonwebtoken::encode(
        &Header::default(),
        &Claims { iat: now.as_secs() },
        &EncodingKey::from_secret(&hex::decode("00")?),
    )?;
    let filter = validate_jwt();
    let res = warp::test::request()
        .header("authorization", ["Bearer", &token].join(" "))
        .filter(&filter)
        .await;
    assert_eq!(res.unwrap(), token);
    Ok(())
}

#[tokio::test]
async fn test_unauthorized_requests() -> anyhow::Result<()> {
    std::env::set_var("JWT_SECRET", "00");
    let filter = validate_jwt();
    let res = warp::test::request().filter(&filter).await;
    assert!(res.is_err()); // Missing JWT token in the header

    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    let token = jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            iat: now.as_secs() - 100, // 100 seconds ago
        },
        &EncodingKey::from_secret(&hex::decode("00")?),
    )?;
    let res = warp::test::request()
        .header("authorization", ["Bearer", &token].join(" "))
        .filter(&filter)
        .await;
    assert!(res.is_err()); // Expired JWT token error
    Ok(())
}

#[tokio::test]
async fn test_entry_function_payload() -> anyhow::Result<()> {
    let to = AccountAddress::TEN;
    let amount: u64 = 100;
    let eth_coin_type: TypeTag = TypeTag::Struct(Box::new(StructTag {
        address: AccountAddress::ONE,
        module: ident_str!("Ethereum").to_owned(),
        name: ident_str!("ETH").to_owned(),
        type_args: vec![],
    }));
    let payload = TransactionPayload::EntryFunction(EntryFunction::new(
        ModuleId::new(
            // TODO: Use 20 byte address length
            AccountAddress::new([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ]),
            ident_str!("coin").to_owned(),
        ),
        ident_str!("transfer").to_owned(),
        vec![eth_coin_type],
        vec![bcs::to_bytes(&to).unwrap(), bcs::to_bytes(&amount).unwrap()],
    ));
    let serialized_payload = bcs::to_bytes(&payload)?;
    assert_eq!(serialized_payload[0], 2); // Starting with 2 indicates an entry function
    Ok(())
}
