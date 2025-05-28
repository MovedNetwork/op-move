use {
    bytes::Bytes,
    move_binary_format::errors::PartialVMResult,
    move_core_types::{
        account_address::AccountAddress,
        effects::ChangeSet,
        language_storage::{ModuleId, StructTag},
        metadata::Metadata,
        value::MoveTypeLayout,
    },
    move_vm_types::resolver::{ModuleResolver, ResourceResolver},
};

pub struct ChangesBasedResolver<'a> {
    changes: &'a ChangeSet,
}

impl<'a> ChangesBasedResolver<'a> {
    pub fn new(changes: &'a ChangeSet) -> Self {
        Self { changes }
    }
}

impl ResourceResolver for ChangesBasedResolver<'_> {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        _metadata: &[Metadata],
        _layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<(Option<Bytes>, usize)> {
        let bytes = self
            .changes
            .accounts()
            .get(address)
            .and_then(|account| account.resources().get(struct_tag))
            .and_then(|op| op.clone().ok());
        let size = bytes.as_ref().map(|b| b.len()).unwrap_or(0);
        Ok((bytes, size))
    }
}

impl ModuleResolver for ChangesBasedResolver<'_> {
    fn get_module_metadata(&self, _module_id: &ModuleId) -> Vec<Metadata> {
        Vec::new()
    }

    fn get_module(&self, id: &ModuleId) -> PartialVMResult<Option<Bytes>> {
        let bytes = self
            .changes
            .accounts()
            .get(id.address())
            .and_then(|account| account.modules().get(id.name()))
            .and_then(|op| op.clone().ok());
        Ok(bytes)
    }
}
