use std::{collections::HashMap, hash::Hash};

/// The trait `GroupIterator` is defined by a single [`Self::group_by`] operation.
pub trait GroupIterator<V> {
    /// Consumes the iterator and allocates a [`HashMap`].
    ///
    /// The iterator can extract key [`K`] by passing the function `f` from a reference to value
    /// [`V`]. Two different key-value pairs with equal keys are considered as having duplicate keys.
    ///
    /// The result of this operation is a [`HashMap`] where keys are taken from key-value pairs.
    /// For a key [`K`] a value is a vector of every [`V`] associated with the key in the iterator.
    ///
    /// The keys of type [`K`] are compared using their [`Eq`] implementation and hashed by their
    /// [`Hash`] implementation. The values [`V`] have no bounds.
    ///
    /// This operation is often referred to as "grouping by key," hence the name.
    fn group_by<K: Eq + Hash, F: Fn(&V) -> K>(self, f: F) -> HashMap<K, Vec<V>>;
}

impl<V, I: Iterator<Item = V>> GroupIterator<V> for I {
    fn group_by<K: Eq + Hash, F: Fn(&V) -> K>(self, f: F) -> HashMap<K, Vec<V>> {
        let mut hash_map = match self.size_hint() {
            (_, Some(len)) => HashMap::with_capacity(len),
            (len, None) => HashMap::with_capacity(len),
        };

        for (key, value) in self.map(|v| (f(&v), v)) {
            hash_map
                .entry(key)
                .or_insert_with(|| Vec::with_capacity(1))
                .push(value)
        }

        hash_map
    }
}

/// A trait extension for iterators that turns them into an iterator of pairs of their values.
///
/// It is defined by a single [`Self::pair`] method.
pub trait PairIteratorExt<T, I: Iterator<Item = T>> {
    /// Wraps `self` into an iterator of its `Item` wrapped in a [`PairOrSingle`].
    ///
    /// When [`PairIterator::next`] is called, up to two next items are taken from the inner
    /// iterator. It keeps returning pairs of two values until it reaches the end.
    ///
    /// The iterator always returns [`PairOrSingle::Pair`] with one exception. There is a special case if
    /// the inner iterator has odd amount of values. The [`PairIterator`] returns the last remaining
    /// value as [`PairOrSingle::Single`]. There is no other way this iterator returns this [`PairOrSingle`]
    /// variant.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use umi_shared::iter::{PairIteratorExt, PairOrSingle::{self, Pair, Single}};
    /// let mut iter = [1, 2].into_iter().pair();
    ///
    /// assert_eq!(iter.next(), Some(Pair(1, 2)));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// Using an iterator with odd amount of items:
    ///
    /// ```
    /// # use umi_shared::iter::{PairIteratorExt, PairOrSingle::{self, Pair, Single}};
    /// let mut iter = [1, 2, 3, 4, 5].into_iter().pair();
    ///
    /// assert_eq!(iter.next(), Some(Pair(1, 2)));
    /// assert_eq!(iter.next(), Some(Pair(3, 4)));
    /// assert_eq!(iter.next(), Some(Single(5)));
    /// assert_eq!(iter.next(), None);
    /// ```
    fn pair(self) -> PairIterator<T, I>;
}

impl<T, I: Iterator<Item = T>> PairIteratorExt<T, I> for I {
    fn pair(self) -> PairIterator<T, I> {
        PairIterator::new(self)
    }
}

/// A single or a pair of values of type `T`.
///
/// # Variants
/// * `Pair` represents an ordered tuple of two values of the same type.
/// * `Single` represents a single value.
#[derive(Debug, PartialEq)]
pub enum PairOrSingle<T> {
    Pair(T, T),
    Single(T),
}

/// Turns any iterator into an iterator over a [`PairOrSingle`] items wrapping the original type.
///
/// See [extension trait documentation] for more information.
///
/// [extension trait documentation]: PairIteratorExt::pair
pub struct PairIterator<T, I: Iterator<Item = T>> {
    inner: I,
}

impl<T, I: Iterator<Item = T>> PairIterator<T, I> {
    pub fn new(inner: I) -> Self {
        Self { inner }
    }
}

impl<T, I> Iterator for PairIterator<T, I>
where
    I: Iterator<Item = T>,
{
    type Item = PairOrSingle<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let first = self.inner.next()?;

        Some(match self.inner.next() {
            Some(second) => PairOrSingle::Pair(first, second),
            None => PairOrSingle::Single(first),
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        PairOrSingle::{Pair, Single},
        std::collections::HashMap,
        test_case::test_case,
    };

    #[test_case([], []; "Empty")]
    #[test_case([1, 2, 3], [(1, vec![1]), (2, vec![2]), (3, vec![3])]; "All unique")]
    #[test_case([1, 2, 2], [(1, vec![1]), (2, vec![2, 2])]; "Some unique")]
    #[test_case([1, 1, 2, 2, 2], [(1, vec![1, 1]), (2, vec![2, 2, 2])]; "None unique")]
    fn test_grouping_by_value_puts_equal_values_together(
        values: impl IntoIterator<Item = i32>,
        expected: impl Into<HashMap<i32, Vec<i32>>>,
    ) {
        let expected_groups = expected.into();
        let actual_groups = values.into_iter().group_by(|v| *v);

        assert_eq!(actual_groups, expected_groups)
    }

    #[test_case([], []; "Empty")]
    #[test_case([1], [Single(1)]; "Single")]
    #[test_case([1, 2], [Pair(1, 2)]; "Pair")]
    #[test_case([1, 2, 3], [Pair(1, 2), Single(3)]; "Pair and single")]
    #[test_case([1, 2, 3, 4], [Pair(1, 2), Pair(3, 4)]; "Two pairs")]
    fn test_pairing_values_puts_two_adjacent_values_together(
        values: impl IntoIterator<Item = i32>,
        expected: impl IntoIterator<Item = PairOrSingle<i32>>,
    ) {
        let expected_pairs = expected.into_iter().collect::<Vec<_>>();
        let actual_pairs = values.into_iter().pair().collect::<Vec<_>>();

        assert_eq!(actual_pairs, expected_pairs);
    }
}
