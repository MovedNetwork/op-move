use {
    core::fmt,
    move_core_types::effects::ChangeSet,
    move_table_extension::{TableChange, TableChangeSet},
    std::fmt::{Debug, DebugStruct, Formatter},
};

/// Represents the difference between two [`State`]s.
///
/// The difference can be [applied]. If the difference is between two states `S` and `S'`, then it
/// must be true that `S' := apply(S, Changes)`.
///
/// `Changes` are usually produced by running a transaction on state `S`, in which case `S'`
/// represents the state after.
///
/// [`State`]: crate::State
/// [applied]: crate::State::apply
pub struct Changes {
    pub accounts: ChangeSet,
    pub tables: TableChangeSet,
}

impl Changes {
    pub fn empty() -> Self {
        Self {
            accounts: ChangeSet::new(),
            tables: TableChangeSet::default(),
        }
    }

    pub const fn new(accounts: ChangeSet, tables: TableChangeSet) -> Self {
        Self { accounts, tables }
    }

    pub fn without_tables(accounts: ChangeSet) -> Self {
        Self {
            accounts,
            tables: TableChangeSet::default(),
        }
    }
}

impl Debug for Changes {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        /// A `Closure` wrapper.
        ///
        /// Implements [`Debug`] if the `Closure` has the same signature as [`Debug::fmt`].
        struct DebugClosure<Closure>(Closure);

        impl<F: Fn(&mut Formatter<'_>) -> fmt::Result> Debug for DebugClosure<F> {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                self.0(f)
            }
        }

        /// An extension trait that provides the closure based debug formatting.
        trait FieldWithStable {
            /// Adds a new field to the generated struct output.
            ///
            /// This method is equivalent to [`DebugStruct::field`], but formats the
            /// value using a provided closure rather than by calling [`Debug::fmt`].
            fn field_with_stable<F>(&mut self, name: &str, value_fmt: F) -> &mut Self
            where
                F: Fn(&mut Formatter<'_>) -> fmt::Result;
        }

        impl FieldWithStable for DebugStruct<'_, '_> {
            fn field_with_stable<F>(&mut self, name: &str, value_fmt: F) -> &mut Self
            where
                F: Fn(&mut Formatter<'_>) -> std::fmt::Result,
            {
                self.field(name, &DebugClosure(value_fmt))
            }
        }

        f.debug_struct("Changes")
            .field("accounts", &self.accounts)
            .field_with_stable("tables", |f| {
                f.debug_struct("TableChangeSet")
                    .field_with_stable("changes", |f| {
                        f.debug_map()
                            .entries(self.tables.changes.iter().map(|(k, v)| (k, &v.entries)))
                            .finish()
                    })
                    .field_with_stable("new_tables", |f| {
                        f.debug_map()
                            .entries(self.tables.new_tables.iter())
                            .finish()
                    })
                    .field_with_stable("removed_tables", |f| {
                        f.debug_set()
                            .entries(self.tables.removed_tables.iter())
                            .finish()
                    })
                    .finish()
            })
            .finish()
    }
}

impl Clone for Changes {
    fn clone(&self) -> Self {
        Self {
            accounts: self.accounts.clone(),
            tables: TableChangeSet {
                new_tables: self.tables.new_tables.clone(),
                removed_tables: self.tables.removed_tables.clone(),
                changes: self
                    .tables
                    .changes
                    .iter()
                    .map(|(k, v)| {
                        (
                            *k,
                            TableChange {
                                entries: v.entries.clone(),
                            },
                        )
                    })
                    .collect(),
            },
        }
    }
}
