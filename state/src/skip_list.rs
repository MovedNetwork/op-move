use {
    alloy::primitives::FixedBytes,
    eth_trie::{DB, EthTrie, Trie, TrieError},
    move_core_types::{
        account_address::AccountAddress, effects::Op, identifier::Identifier,
        language_storage::StructTag,
    },
    rand::{
        Rng,
        distributions::{Bernoulli, Distribution},
    },
    serde::{Deserialize, Serialize, de::DeserializeOwned},
    std::{borrow::Cow, marker::PhantomData},
};

const TRIE_SERIALIZATION_MESSAGE: &str = "SkipList serialization must succeed";

pub fn update_trie<T, U, D, R>(
    account: &AccountAddress,
    element: &T,
    update: &Op<U>,
    trie: &mut EthTrie<D>,
    rng: &mut R,
) -> Result<(), TrieError>
where
    T: Clone + Listable + Ord + Serialize + DeserializeOwned + 'static,
    D: DB,
    R: Rng,
{
    match update {
        Op::New(_) => insert_item(*account, element, trie, rng),
        Op::Delete => delete_item(*account, element, trie),
        Op::Modify(_) => Ok(()),
    }
}

pub fn delete_item<T, D>(
    account: AccountAddress,
    element: &T,
    trie: &mut EthTrie<D>,
) -> Result<(), TrieError>
where
    T: Clone + Listable + Ord + Serialize + DeserializeOwned + 'static,
    D: DB,
{
    let bottom_key = SkipListKey::new(account, 0, element);

    // The key is not present, so there is nothing to do.
    if bottom_key.trie_value(trie)?.is_none() {
        return Ok(());
    }

    let head_key = SkipListHeadKey::<T>::new(account);
    let head = head_key.trie_value(trie)?;

    let Some(first_value) = head.first_value else {
        // The list is empty, so there is nothing to delete.
        return Ok(());
    };

    let prevs = collect_predecessors(account, head.max_levels, first_value, element, trie)?;

    // Search returns keys that are strictly less than the search element.
    // If the search element is in the list then it will be the value of the returned key
    for (key, value) in prevs.into_iter().flatten() {
        if value.next_value.as_deref() != Some(element) {
            continue;
        }

        let delete_key = SkipListKey::new(account, key.level, element);
        let delete_trie_key = delete_key.key_hash();
        let next_value = SkipListValue::<T>::read_trie(&delete_trie_key, trie)?;
        trie.remove(delete_trie_key.as_slice())?;
        if let Some(next_value) = next_value {
            trie.insert(key.key_hash().as_slice(), &next_value.serialize())?;
        }
    }

    Ok(())
}

pub fn insert_item<T, D, R>(
    account: AccountAddress,
    element: &T,
    trie: &mut EthTrie<D>,
    rng: &mut R,
) -> Result<(), TrieError>
where
    T: Clone + Listable + Ord + Serialize + DeserializeOwned + 'static,
    D: DB,
    R: Rng,
{
    let insert_key = SkipListKey::new(account, 0, element);

    // The key is already present, so there is nothing to do.
    if insert_key.trie_value(trie)?.is_some() {
        return Ok(());
    }

    let head_key = SkipListHeadKey::<T>::new(account);
    let trie_head_key = head_key.key_hash();
    let head = SkipListHeadValue::<T>::read_trie(&trie_head_key, trie)?;
    let end_of_list = SkipListValue::<T> { next_value: None }.serialize();

    let Some(first_value) = head.first_value else {
        // The list is empty, so we just insert this value
        let new_head = SkipListHeadValue::new(0, element);
        trie.insert(trie_head_key.as_slice(), &new_head.serialize())?;

        trie.insert(insert_key.key_hash().as_slice(), &end_of_list)?;

        return Ok(());
    };

    if element < first_value.as_ref() {
        // The new element needs to become the head of the list.

        // Randomly pick the level for the old head (now second element).
        let reinsert_level = pick_insert_level(rng) as u32;

        // Update head value
        let max_levels = std::cmp::max(reinsert_level as u32, head.max_levels);
        let new_head = SkipListHeadValue::new(max_levels, element);
        trie.insert(trie_head_key.as_slice(), &new_head.serialize())?;

        // Insert new keys for new head element
        let insert_value = SkipListValue::<T> {
            next_value: Some(Cow::Borrowed(&first_value)),
        }
        .serialize();
        for level in 0..=reinsert_level {
            let fv_key = SkipListKey::new(account, level, first_value.as_ref());
            let insert_key = SkipListKey::new(account, level, element);

            let fv_value = fv_key.trie_value(trie)?;
            match fv_value {
                Some(_) => trie.insert(insert_key.key_hash().as_slice(), &insert_value)?,
                None => {
                    trie.insert(insert_key.key_hash().as_slice(), &insert_value)?;
                    trie.insert(fv_key.key_hash().as_slice(), &end_of_list)?;
                }
            }
        }
        for level in (reinsert_level + 1)..=max_levels {
            let fv_key = SkipListKey::new(account, level, first_value.as_ref());
            let insert_key = SkipListKey::new(account, level, element);

            let fv_value = fv_key.trie_value(trie)?;
            match fv_value {
                Some(value) => {
                    trie.remove(fv_key.key_hash().as_slice())?;
                    trie.insert(insert_key.key_hash().as_slice(), &value.serialize())?;
                }
                None => trie.insert(insert_key.key_hash().as_slice(), &end_of_list)?,
            }
        }

        return Ok(());
    }

    // For each height the new key occupies, insert it into the corresponding
    // linked list.
    let insert_level = pick_insert_level(rng);
    let insert_value = SkipListValue::<T> {
        next_value: Some(Cow::Borrowed(element)),
    }
    .serialize();
    let prevs = collect_predecessors(account, head.max_levels, first_value.clone(), element, trie)?;
    let mut updated_heights = 0;
    for (key, original_value) in prevs.into_iter().flatten().take(insert_level + 1) {
        trie.insert(key.key_hash().as_slice(), &insert_value)?;

        let insert_key = SkipListKey::new(account, key.level, element);
        trie.insert(
            insert_key.key_hash().as_slice(),
            &original_value.serialize(),
        )?;

        updated_heights += 1;
    }

    if updated_heights <= insert_level {
        // New element is taller than previous max height.
        // Add new keys pointing from first to new element and update head value.
        let mut level = head.max_levels + 1;
        while updated_heights <= insert_level {
            let key = SkipListKey::new(account, level, first_value.as_ref());
            trie.insert(key.key_hash().as_slice(), &insert_value)?;

            let insert_key = SkipListKey::new(account, level, element);
            trie.insert(insert_key.key_hash().as_slice(), &end_of_list)?;

            level += 1;
            updated_heights += 1;
        }

        let new_head = SkipListHeadValue::new(level - 1, first_value.as_ref());
        trie.insert(trie_head_key.as_slice(), &new_head.serialize())?;
    }

    Ok(())
}

/// Each type that implements `Listable` must have a unique value for `kind`.
/// It is used as a discriminator to make the byte-level representation of the keys
/// in the trie unique.
pub trait Listable {
    fn kind() -> u8;
}

impl Listable for StructTag {
    fn kind() -> u8 {
        0
    }
}

impl Listable for Identifier {
    fn kind() -> u8 {
        1
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SkipListHeadKey<T> {
    account: AccountAddress,
    list_kind: u8,
    phantom: PhantomData<T>,
}

impl<T> SkipListHeadKey<T> {
    pub fn key_hash(&self) -> FixedBytes<32> {
        let bytes =
            bcs::to_bytes(&(&self.list_kind, &self.account)).expect(TRIE_SERIALIZATION_MESSAGE);
        alloy::primitives::keccak256(bytes)
    }
}

impl<T: Listable> SkipListHeadKey<T> {
    pub fn new(account: AccountAddress) -> Self {
        Self {
            account,
            list_kind: T::kind(),
            phantom: PhantomData,
        }
    }
}

impl<T: Clone + DeserializeOwned + 'static> SkipListHeadKey<T> {
    pub fn trie_value<D: DB>(
        &self,
        trie: &EthTrie<D>,
    ) -> Result<SkipListHeadValue<'static, T>, TrieError> {
        let trie_key = self.key_hash();
        SkipListHeadValue::read_trie(&trie_key, trie)
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkipListHeadValue<'a, T: Clone> {
    pub max_levels: u32,
    pub first_value: Option<Cow<'a, T>>,
}

impl<T: Clone> Default for SkipListHeadValue<'_, T> {
    fn default() -> Self {
        Self {
            max_levels: 0,
            first_value: None,
        }
    }
}

impl<'a, T: Clone> SkipListHeadValue<'a, T> {
    pub fn new(max_levels: u32, first_value: &'a T) -> Self {
        Self {
            max_levels,
            first_value: Some(Cow::Borrowed(first_value)),
        }
    }
}

impl<T: Clone + Serialize> SkipListHeadValue<'_, T> {
    pub fn serialize(&self) -> Vec<u8> {
        bcs::to_bytes(&self).expect(TRIE_SERIALIZATION_MESSAGE)
    }
}

impl<T: Clone + DeserializeOwned> SkipListHeadValue<'static, T> {
    pub fn deserialize(bytes: &[u8]) -> Self {
        bcs::from_bytes(bytes).expect("Trie must contain valid SkipListHeadValue")
    }

    pub fn read_trie<D: DB>(
        trie_key: &FixedBytes<32>,
        trie: &EthTrie<D>,
    ) -> Result<Self, TrieError> {
        let trie_bytes = trie.get(trie_key.as_slice())?;
        let value = trie_bytes
            .map(|bytes| Self::deserialize(&bytes))
            .unwrap_or_default();
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipListKey<'a, T: Clone> {
    pub account: AccountAddress,
    pub level: u32,
    pub value: Cow<'a, T>,
}

impl<'a, T: Clone> SkipListKey<'a, T> {
    pub fn new(account: AccountAddress, level: u32, value: &'a T) -> Self {
        Self {
            account,
            level,
            value: Cow::Borrowed(value),
        }
    }

    pub fn from_cow(account: AccountAddress, level: u32, value: Cow<'a, T>) -> Self {
        Self {
            account,
            level,
            value,
        }
    }
}

impl<T: Clone + Serialize> SkipListKey<'_, T> {
    pub fn key_hash(&self) -> FixedBytes<32> {
        let bytes = bcs::to_bytes(&(&self.account, self.level, &self.value))
            .expect(TRIE_SERIALIZATION_MESSAGE);
        alloy::primitives::keccak256(bytes)
    }
}

impl<T: Clone + Serialize + DeserializeOwned + 'static> SkipListKey<'_, T> {
    pub fn trie_value<D: DB>(
        &self,
        trie: &EthTrie<D>,
    ) -> Result<Option<SkipListValue<'static, T>>, TrieError> {
        let trie_key = self.key_hash();
        SkipListValue::read_trie(&trie_key, trie)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkipListValue<'a, T: Clone> {
    pub next_value: Option<Cow<'a, T>>,
}

impl<T: Clone + Serialize> SkipListValue<'_, T> {
    pub fn serialize(&self) -> Vec<u8> {
        bcs::to_bytes(&self).expect(TRIE_SERIALIZATION_MESSAGE)
    }
}

impl<T: Clone + DeserializeOwned> SkipListValue<'static, T> {
    pub fn read_trie<D: DB>(
        trie_key: &FixedBytes<32>,
        trie: &EthTrie<D>,
    ) -> Result<Option<Self>, TrieError> {
        let trie_bytes = trie.get(trie_key.as_slice())?;
        let value = trie_bytes.map(|bytes| Self::deserialize(&bytes));
        Ok(value)
    }

    pub fn deserialize(bytes: &[u8]) -> Self {
        bcs::from_bytes(bytes).expect("Trie must contain valid SkipListValue")
    }
}

pub struct SkipListIterator<'t, T: Clone + 'static, D: DB> {
    trie: &'t EthTrie<D>,
    level: u32,
    next_key: Option<SkipListKey<'static, T>>,
}

impl<'t, T, D> SkipListIterator<'t, T, D>
where
    D: DB,
    T: Clone + Ord + Listable + Serialize + DeserializeOwned + 'static,
{
    pub fn new(
        account: AccountAddress,
        start: Option<&T>,
        trie: &'t EthTrie<D>,
    ) -> Result<Self, TrieError> {
        match start {
            Some(start) => Self::new_from_start(account, start, trie, 0),
            None => Self::new_from_head(account, trie, 0),
        }
    }

    fn new_from_head(
        account: AccountAddress,
        trie: &'t EthTrie<D>,
        level: u32,
    ) -> Result<Self, TrieError> {
        let head_key = SkipListHeadKey::new(account);
        let value = head_key.trie_value(trie)?;
        Ok(Self {
            trie,
            level,
            next_key: value
                .first_value
                .map(|t| SkipListKey::from_cow(account, level, t)),
        })
    }

    fn new_from_start(
        account: AccountAddress,
        start: &T,
        trie: &'t EthTrie<D>,
        level: u32,
    ) -> Result<Self, TrieError> {
        let start_key = SkipListKey::new(account, 0, start);
        let Some(start_value) = start_key.trie_value(trie)? else {
            let head_key = SkipListHeadKey::new(account);
            let head = head_key.trie_value(trie)?;
            let Some(first_value) = head.first_value else {
                // Return empty iterator if list is empty
                return Ok(Self {
                    trie,
                    level,
                    next_key: None,
                });
            };
            let mut prevs =
                collect_predecessors(account, head.max_levels, first_value, start, trie)?;
            let mut this = Self {
                trie,
                level,
                next_key: prevs[level as usize].take().map(|(k, _)| k),
            };
            // We want to start on the key after `start`, so we need to call `next` once
            // since we got from the search the key before `start`.
            this.next();
            return Ok(this);
        };
        Ok(Self {
            trie,
            level,
            next_key: start_value
                .next_value
                .map(|t| SkipListKey::from_cow(account, level, t)),
        })
    }
}

impl<T, D> Iterator for SkipListIterator<'_, T, D>
where
    D: DB,
    T: Clone + Listable + Serialize + DeserializeOwned + 'static,
{
    type Item = Result<T, TrieError>;

    fn next(&mut self) -> Option<Self::Item> {
        let key = self.next_key.take()?;
        let value = match key.trie_value(self.trie).transpose()? {
            Ok(value) => value,
            Err(e) => {
                return Some(Err(e));
            }
        };
        let next_key = value
            .next_value
            .map(|t| SkipListKey::from_cow(key.account, self.level, t));
        self.next_key = next_key;
        Some(Ok(key.value.into_owned()))
    }
}

type SkipListPair<T> = (SkipListKey<'static, T>, SkipListValue<'static, T>);

fn collect_predecessors<T, D>(
    account: AccountAddress,
    max_levels: u32,
    first_value: Cow<'static, T>,
    element: &T,
    trie: &EthTrie<D>,
) -> Result<Vec<Option<SkipListPair<T>>>, TrieError>
where
    D: DB,
    T: Clone + Listable + Ord + Serialize + DeserializeOwned + 'static,
{
    let mut prevs: Vec<Option<(SkipListKey<'static, T>, SkipListValue<'static, T>)>> =
        vec![None; (max_levels + 1) as usize];

    // If new element is before the first element then there is nothing to do.
    if element < first_value.as_ref() {
        return Ok(prevs);
    }

    let mut current_key = SkipListKey::from_cow(account, max_levels, first_value);

    // Loop invariant: by design, `current_key.value < element`
    loop {
        let current_value = current_key
            .trie_value(trie)?
            .unwrap_or_else(|| panic!("Failed to find trie value for key account={account} level={} max_level={max_levels}", current_key.level));
        let Some(next_value) = current_value.next_value.as_ref() else {
            prevs[current_key.level as usize] = Some((current_key.clone(), current_value));
            let Some(lower_level) = current_key.level.checked_sub(1) else {
                return Ok(prevs);
            };
            current_key.level = lower_level;
            continue;
        };

        // new element is after the next value too, keep going
        if next_value.as_ref() < element {
            current_key.value = current_value.next_value.expect("next_value is some");
            continue;
        }

        // New element is between current element and next element
        prevs[current_key.level as usize] = Some((current_key.clone(), current_value));
        let Some(lower_level) = current_key.level.checked_sub(1) else {
            return Ok(prevs);
        };
        current_key.level = lower_level;
        continue;
    }
}

fn pick_insert_level<R: Rng>(rng: &mut R) -> usize {
    let mut result = 0;
    let d = Bernoulli::new(0.5).expect("Probability is valid.");
    while d.sample(rng) {
        result += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        eth_trie::MemoryDB,
        rand::{SeedableRng, rngs::SmallRng},
        std::sync::Arc,
    };

    impl Listable for u64 {
        fn kind() -> u8 {
            64
        }
    }

    // Creates a new SkipList with the following shape:
    // 0 ----------------> Nil
    // 0 ------> 5 ------> Nil
    // 0 -> 2 -> 5 -> 8 -> Nil
    fn mock_list() -> EthTrie<MemoryDB> {
        let keys: Vec<(SkipListKey<u64>, SkipListValue<u64>)> = vec![
            (
                SkipListKey::from_cow(AccountAddress::ZERO, 2, Cow::Owned(0)),
                SkipListValue { next_value: None },
            ),
            (
                SkipListKey::from_cow(AccountAddress::ZERO, 1, Cow::Owned(0)),
                SkipListValue {
                    next_value: Some(Cow::Owned(5)),
                },
            ),
            (
                SkipListKey::from_cow(AccountAddress::ZERO, 0, Cow::Owned(0)),
                SkipListValue {
                    next_value: Some(Cow::Owned(2)),
                },
            ),
            (
                SkipListKey::from_cow(AccountAddress::ZERO, 1, Cow::Owned(5)),
                SkipListValue { next_value: None },
            ),
            (
                SkipListKey::from_cow(AccountAddress::ZERO, 0, Cow::Owned(5)),
                SkipListValue {
                    next_value: Some(Cow::Owned(8)),
                },
            ),
            (
                SkipListKey::from_cow(AccountAddress::ZERO, 0, Cow::Owned(2)),
                SkipListValue {
                    next_value: Some(Cow::Owned(5)),
                },
            ),
            (
                SkipListKey::from_cow(AccountAddress::ZERO, 0, Cow::Owned(8)),
                SkipListValue { next_value: None },
            ),
        ];

        let db = Arc::new(MemoryDB::new(false));
        let mut result = EthTrie::new(db);

        let head_key = SkipListHeadKey::<u64>::new(AccountAddress::ZERO);
        let head = SkipListHeadValue::<u64>::new(2, &0);

        result
            .insert(head_key.key_hash().as_slice(), &head.serialize())
            .unwrap();

        for (k, v) in keys {
            result
                .insert(k.key_hash().as_slice(), &v.serialize())
                .unwrap();
        }

        result.root_hash().unwrap();

        result
    }

    #[test]
    fn test_collect_predecessors() {
        let trie = mock_list();

        assert_eq!(
            collect_predecessors::<u64, MemoryDB>(
                AccountAddress::ZERO,
                2,
                Cow::Owned(0),
                &3,
                &trie
            )
            .unwrap(),
            vec![
                Some((
                    SkipListKey::from_cow(AccountAddress::ZERO, 0, Cow::Owned(2)),
                    SkipListValue {
                        next_value: Some(Cow::Owned(5))
                    }
                )),
                Some((
                    SkipListKey::from_cow(AccountAddress::ZERO, 1, Cow::Owned(0)),
                    SkipListValue {
                        next_value: Some(Cow::Owned(5))
                    }
                )),
                Some((
                    SkipListKey::from_cow(AccountAddress::ZERO, 2, Cow::Owned(0)),
                    SkipListValue { next_value: None }
                )),
            ]
        )
    }

    #[test]
    fn test_iterator() {
        let trie = mock_list();
        assert_eq!(values_at_level(&trie, 0), vec![0, 2, 5, 8]);
        assert_eq!(values_at_level(&trie, 1), vec![0, 5]);
        assert_eq!(values_at_level(&trie, 2), vec![0]);
        assert!(values_at_level(&trie, 3).is_empty());

        let iter =
            SkipListIterator::<u64, MemoryDB>::new_from_start(AccountAddress::ZERO, &3, &trie, 0)
                .unwrap();
        let values: Vec<u64> = iter.map(|v| v.unwrap()).collect();
        assert_eq!(values, vec![5, 8]);
    }

    #[test]
    fn test_delete() {
        let mut trie = mock_list();
        delete_item::<u64, MemoryDB>(AccountAddress::ZERO, &5, &mut trie).unwrap();

        // After delete skip list structure is:
        // 0 ----------------> Nil
        // 0 ----------------> Nil
        // 0 -> 2 ------> 8 -> Nil
        assert_eq!(values_at_level(&trie, 0), vec![0, 2, 8]);
        assert_eq!(values_at_level(&trie, 1), vec![0]);
        assert_eq!(values_at_level(&trie, 2), vec![0]);

        let mut trie = mock_list();
        delete_item::<u64, MemoryDB>(AccountAddress::ZERO, &8, &mut trie).unwrap();

        // After delete skip list structure is:
        // 0 ----------------> Nil
        // 0 ------> 5 ------> Nil
        // 0 -> 2 -> 5 ------> Nil
        assert_eq!(values_at_level(&trie, 0), vec![0, 2, 5]);
        assert_eq!(values_at_level(&trie, 1), vec![0, 5]);
        assert_eq!(values_at_level(&trie, 2), vec![0]);
    }

    #[test]
    fn test_insert() {
        let mut rng = SmallRng::seed_from_u64(1234);
        let mut trie = mock_list();

        insert_item::<u64, MemoryDB, SmallRng>(AccountAddress::ZERO, &6, &mut trie, &mut rng)
            .unwrap();
        trie.root_hash().unwrap();

        // After insert skip list structure is:
        // 0 --------> 6 ---------> Nil
        // 0 --------> 6 ---------> Nil
        // 0 ------> 5 -> 6 ------> Nil
        // 0 -> 2 -> 5 -> 6 -> 8 -> Nil
        // (the extra 6 on top is because the rng generated an insert height of 4)

        // Confirm this structure using the iterator at each level
        assert_eq!(values_at_level(&trie, 0), vec![0, 2, 5, 6, 8]);
        assert_eq!(values_at_level(&trie, 1), vec![0, 5, 6]);
        assert_eq!(values_at_level(&trie, 2), vec![0, 6]);
        assert_eq!(values_at_level(&trie, 3), vec![0, 6]);
        assert!(values_at_level(&trie, 4).is_empty()); // there is no level 4
    }

    #[test]
    fn test_build_list() {
        let mut rng = SmallRng::seed_from_u64(777);

        // Start with an empty trie
        let db = Arc::new(MemoryDB::new(false));
        let mut trie = EthTrie::new(db);

        // Insert the first value
        insert_item(AccountAddress::ZERO, &7_u64, &mut trie, &mut rng).unwrap();
        trie.root_hash().unwrap();

        // Confirm the value was inserted
        assert_eq!(values_at_level(&trie, 0), vec![7]);
        assert!(values_at_level(&trie, 1).is_empty());

        // Insert a new value at the head of the list
        insert_item(AccountAddress::ZERO, &3_u64, &mut trie, &mut rng).unwrap();
        trie.root_hash().unwrap();

        assert_eq!(values_at_level(&trie, 0), vec![3, 7]);

        // Insert a new value at the end of the list
        insert_item(AccountAddress::ZERO, &20_u64, &mut trie, &mut rng).unwrap();
        trie.root_hash().unwrap();

        assert_eq!(values_at_level(&trie, 0), vec![3, 7, 20]);

        // Insert a new value in the middle
        insert_item(AccountAddress::ZERO, &8_u64, &mut trie, &mut rng).unwrap();
        trie.root_hash().unwrap();

        assert_eq!(values_at_level(&trie, 0), vec![3, 7, 8, 20]);

        // Insert another value at the start
        insert_item(AccountAddress::ZERO, &1_u64, &mut trie, &mut rng).unwrap();
        trie.root_hash().unwrap();

        assert_eq!(values_at_level(&trie, 0), vec![1, 3, 7, 8, 20]);

        // With this rng seed the final list as the form:
        // 1 -> 3 ------> 8 -------> Nil
        // 1 -> 3 ------> 8 -> 20 -> Nil
        // 1 -> 3 -> 7 -> 8 -> 20 -> Nil
        assert_eq!(
            SkipListHeadKey::<u64>::new(AccountAddress::ZERO)
                .trie_value(&trie)
                .unwrap()
                .max_levels,
            2
        );
        assert_eq!(values_at_level(&trie, 0), vec![1, 3, 7, 8, 20]);
        assert_eq!(values_at_level(&trie, 1), vec![1, 3, 8, 20]);
        assert_eq!(values_at_level(&trie, 2), vec![1, 3, 8]);
        assert!(values_at_level(&trie, 3).is_empty());
    }

    fn values_at_level(trie: &EthTrie<MemoryDB>, level: u32) -> Vec<u64> {
        let iter =
            SkipListIterator::<u64, MemoryDB>::new_from_head(AccountAddress::ZERO, trie, level)
                .unwrap();
        iter.map(|v| v.unwrap()).collect()
    }
}
