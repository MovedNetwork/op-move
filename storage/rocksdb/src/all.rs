use crate::{block, evm_storage_trie, payload, receipt, state, transaction, trie};

pub const COLUMN_FAMILIES: [&str; 11] = [
    block::BLOCK_COLUMN_FAMILY,
    block::HEIGHT_COLUMN_FAMILY,
    state::COLUMN_FAMILY,
    state::HEIGHT_COLUMN_FAMILY,
    trie::TRIE_COLUMN_FAMILY,
    trie::ROOT_COLUMN_FAMILY,
    evm_storage_trie::TRIE_COLUMN_FAMILY,
    evm_storage_trie::ROOT_COLUMN_FAMILY,
    transaction::COLUMN_FAMILY,
    receipt::COLUMN_FAMILY,
    payload::COLUMN_FAMILY,
];

#[cfg(test)]
mod tests {
    use {super::*, std::collections::HashSet};

    #[test]
    fn test_column_families_have_unique_names() {
        let expected_unique_len = COLUMN_FAMILIES.len();
        let actual_unique_len = HashSet::from(COLUMN_FAMILIES).len();

        assert_eq!(actual_unique_len, expected_unique_len);
    }
}
