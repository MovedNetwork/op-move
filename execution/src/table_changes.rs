use {
    aptos_table_natives::{NativeTableContext, TableChangeSet},
    move_binary_format::errors::PartialVMResult,
    move_vm_runtime::{
        AsFunctionValueExtension, ModuleStorage, native_extensions::NativeContextExtensions,
    },
};

pub fn extract_table_changes(
    extensions: &mut NativeContextExtensions,
    module_storage: &impl ModuleStorage,
) -> PartialVMResult<TableChangeSet> {
    let ctx = extensions.remove::<NativeTableContext>();
    let function_value_extension = module_storage.as_function_value_extension();
    ctx.into_change_set(&function_value_extension)
}
