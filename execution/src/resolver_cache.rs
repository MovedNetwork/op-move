use {
    aptos_table_natives::{TableHandle, TableResolver},
    bytes::Bytes,
    move_binary_format::errors::PartialVMResult,
    move_core_types::{
        account_address::AccountAddress,
        language_storage::{ModuleId, StructTag},
        metadata::Metadata,
        value::MoveTypeLayout,
    },
    move_vm_types::resolver::{ModuleResolver, MoveResolver, ResourceResolver},
    std::{
        cell::RefCell,
        collections::{HashMap, hash_map::Entry},
        hash::Hash,
        ops::Deref,
    },
};

#[derive(Debug, Default)]
pub struct ResolverCache {
    resource_cache: HashMap<(AccountAddress, StructTag), Option<Bytes>>,
    modules_cache: HashMap<ModuleId, Option<Bytes>>,
}

impl ResolverCache {
    pub fn resource_original_size(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
    ) -> usize {
        let cache_key = (*address, struct_tag.clone());
        bytes_len(&cache_key, &self.resource_cache)
    }

    pub fn module_original_size(&self, id: &ModuleId) -> usize {
        bytes_len(id, &self.modules_cache)
    }

    pub fn clear(&mut self) {
        self.resource_cache.clear();
        self.modules_cache.clear();
    }
}

pub struct CachedResolver<'a, 'b, R> {
    inner: &'a R,
    cache: RefCell<&'b mut ResolverCache>,
}

impl<'a, 'b, R> CachedResolver<'a, 'b, R>
where
    R: MoveResolver + TableResolver,
{
    pub fn new(resolver: &'a R, cache: &'b mut ResolverCache) -> Self {
        Self {
            inner: resolver,
            cache: RefCell::new(cache),
        }
    }
}

impl<'b, R> CachedResolver<'_, 'b, R> {
    pub fn borrow_cache<'a>(&'a self) -> impl Deref<Target = &'b mut ResolverCache> + 'a {
        self.cache.borrow()
    }
}

impl<R> ResourceResolver for CachedResolver<'_, '_, R>
where
    R: ResourceResolver,
{
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        metadata: &[Metadata],
        layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<(Option<Bytes>, usize)> {
        let cache_key = (*address, struct_tag.clone());
        match self.cache.borrow_mut().resource_cache.entry(cache_key) {
            Entry::Occupied(entry) => {
                let cache_hit = entry.get();
                let size = cache_hit.as_ref().map(Bytes::len).unwrap_or(0);
                Ok((cache_hit.clone(), size))
            }
            Entry::Vacant(entry) => {
                let (bytes, size) = self.inner.get_resource_bytes_with_metadata_and_layout(
                    address, struct_tag, metadata, layout,
                )?;
                let bytes = entry.insert(bytes);
                Ok((bytes.clone(), size))
            }
        }
    }
}

impl<R> ModuleResolver for CachedResolver<'_, '_, R>
where
    R: ModuleResolver,
{
    fn get_module(&self, id: &ModuleId) -> PartialVMResult<Option<Bytes>> {
        let cache_key = id.clone();
        match self.cache.borrow_mut().modules_cache.entry(cache_key) {
            Entry::Occupied(entry) => {
                let cache_hit = entry.get();
                Ok(cache_hit.clone())
            }
            Entry::Vacant(entry) => {
                let bytes = self.inner.get_module(id)?;
                let bytes = entry.insert(bytes);
                Ok(bytes.clone())
            }
        }
    }

    fn get_module_metadata(&self, module_id: &ModuleId) -> Vec<Metadata> {
        self.inner.get_module_metadata(module_id)
    }
}

impl<R> TableResolver for CachedResolver<'_, '_, R>
where
    R: TableResolver,
{
    fn resolve_table_entry_bytes_with_layout(
        &self,
        handle: &TableHandle,
        key: &[u8],
        maybe_layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<Option<Bytes>> {
        self.inner
            .resolve_table_entry_bytes_with_layout(handle, key, maybe_layout)
    }
}

fn bytes_len<K: Eq + Hash>(key: &K, cache: &HashMap<K, Option<Bytes>>) -> usize {
    cache
        .get(key)
        .map(|bytes| bytes.as_ref().map(Bytes::len).unwrap_or(0))
        .unwrap_or(0)
}
