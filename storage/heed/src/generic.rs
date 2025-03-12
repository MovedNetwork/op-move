use {
    heed::{
        byteorder::LittleEndian,
        types::{Bytes, U64},
        BoxedError, BytesDecode, BytesEncode,
    },
    moved_shared::primitives::{Address, B256},
    serde::{Deserialize, Serialize},
    std::{borrow::Cow, fmt::Debug},
};

#[derive(Debug)]
pub struct EncodableB256;

impl<'item> BytesEncode<'item> for EncodableB256 {
    type EItem = B256;

    fn bytes_encode(item: &'item Self::EItem) -> Result<Cow<'item, [u8]>, BoxedError> {
        Ok(Cow::Borrowed(item.as_slice()))
    }
}

impl<'item> BytesDecode<'item> for EncodableB256 {
    type DItem = B256;

    fn bytes_decode(bytes: &'item [u8]) -> Result<Self::DItem, BoxedError> {
        Ok(B256::try_from(bytes)?)
    }
}

#[derive(Debug)]
pub struct EncodableAddress;

impl<'item> BytesEncode<'item> for EncodableAddress {
    type EItem = Address;

    fn bytes_encode(item: &'item Self::EItem) -> Result<Cow<'item, [u8]>, BoxedError> {
        Ok(Cow::Borrowed(item.as_slice()))
    }
}

impl<'item> BytesDecode<'item> for EncodableAddress {
    type DItem = Address;

    fn bytes_decode(bytes: &'item [u8]) -> Result<Self::DItem, BoxedError> {
        Ok(Address::try_from(bytes)?)
    }
}

pub type EncodableU64 = U64<LittleEndian>;
pub type EncodableBytes = Bytes;

/// Describes a type that is [`Serialize`]/[`Deserialize`] and uses `serde_json` to do so.
pub struct SerdeJson<T>(std::marker::PhantomData<T>);

impl<'a, T: 'a> BytesEncode<'a> for SerdeJson<T>
where
    T: Serialize,
{
    type EItem = T;

    fn bytes_encode(item: &'a Self::EItem) -> Result<Cow<'a, [u8]>, BoxedError> {
        serde_json::to_vec(item).map(Cow::Owned).map_err(Into::into)
    }
}

impl<'a, T: 'a> BytesDecode<'a> for SerdeJson<T>
where
    T: Deserialize<'a>,
{
    type DItem = T;

    fn bytes_decode(bytes: &'a [u8]) -> Result<Self::DItem, BoxedError> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }
}

unsafe impl<T> Send for SerdeJson<T> {}

unsafe impl<T> Sync for SerdeJson<T> {}
