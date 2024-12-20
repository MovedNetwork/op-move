script {
    use 0x4200000000000000000000000000000000000019::base_fee_vault;
    use Evm::evm::is_result_success;

    fun main(owner: &signer) {
        let result = base_fee_vault::version(owner);
        assert!(is_result_success(&result), 0);
    }
}
