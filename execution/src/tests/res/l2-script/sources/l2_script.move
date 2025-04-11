script {
    use 0x4200000000000000000000000000000000000019::base_fee_vault;
    use Evm::evm::{as_fixed_bytes, abi_decode_params, evm_output, is_result_success, SolidityFixedBytes, B20};
    use std::string::{bytes, String};

    fun main(owner: &signer) {
        // Test different decoding cases by generating more RLP bytes at https://abi.hashex.org
        let test_bool = abi_decode_params<bool>(x"0000000000000000000000000000000000000000000000000000000000000001");
        assert!(test_bool == true, 0);

        let test_u8 = abi_decode_params<u8>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u8 == 42);

        let test_u16 = abi_decode_params<u16>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u16 == 42);

        let test_u32 = abi_decode_params<u32>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u32 == 42);

        let test_u64 = abi_decode_params<u64>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u64 == 42);

        let test_u128 = abi_decode_params<u128>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u128 == 42);

        let test_u256 = abi_decode_params<u256>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u256 == 42);

        let test_vec_u8 = abi_decode_params<vector<u8>>(x"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000040102030400000000000000000000000000000000000000000000000000000000");
        assert!(test_vec_u8 == x"01020304");

        let test_str = abi_decode_params<String>(x"0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000568656c6c6f000000000000000000000000000000000000000000000000000000");
        assert!(*bytes(&test_str) == b"hello");

        let bytes_32 = x"0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        let test_fixed_bytes = abi_decode_params<SolidityFixedBytes<B20>>(bytes_32);
        let expected_fixed_bytes = as_fixed_bytes<B20>(bytes_32);
        assert!(test_fixed_bytes == expected_fixed_bytes);


        /* ************************ */
        /* L2 Contract Call Testing */
        /* ************************ */

        // Call to the `version()` has string output "1.4.1"
        let result = base_fee_vault::version(owner);
        assert!(is_result_success(&result), 8);
        let version_string = abi_decode_params<String>(evm_output(&result));
        assert!(*bytes(&version_string) == b"1.4.1");
    }
}
