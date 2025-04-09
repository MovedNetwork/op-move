module 0x8fd379246834eac74b8419ffda202cf8051f7a03::solidity_fixed_bytes {
    use 0x1::evm::{abi_encode_params, as_fixed_bytes, B1, B16, B32};

    public entry fun encode_fixed_bytes1(
        input: vector<u8>,
    ) {
        let args = as_fixed_bytes<B1>(input);
        abi_encode_params(vector[], args);
    }

    public entry fun encode_fixed_bytes16(
        input: vector<u8>,
    ) {
        let args = as_fixed_bytes<B16>(input);
        abi_encode_params(vector[], args);
    }

    public entry fun encode_fixed_bytes32(
        input: vector<u8>,
    ) {
        let args = as_fixed_bytes<B32>(input);
        abi_encode_params(vector[], args);
    }
}
