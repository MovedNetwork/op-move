//! Logic for converting Move values to/from JSON representation.

use {
    crate::state::{MoveResourceResponse, MoveValueResponse},
    aptos_api_types::{Address, HexEncodedBytes, U64, U128, U256},
    move_bytecode_utils::compiled_module_viewer::CompiledModuleView,
    move_core_types::{
        language_storage::{StructTag, TypeTag},
        value::{MoveStruct, MoveStructLayout, MoveTypeLayout, MoveValue},
    },
    move_resource_viewer::MoveValueAnnotator,
    umi_shared::error::{Result, UserError},
};

pub struct UmiMoveConverter<'a, V> {
    inner: MoveValueAnnotator<CompiledModuleViewReference<'a, V>>,
}

impl<'a, V: CompiledModuleView> UmiMoveConverter<'a, V> {
    pub fn new(resolver: &'a V) -> Self {
        Self {
            inner: MoveValueAnnotator::new(CompiledModuleViewReference { inner: resolver }),
        }
    }

    pub fn view_resource(&self, tag: &StructTag, blob: &[u8]) -> Result<MoveResourceResponse> {
        let resource = self
            .inner
            .view_resource(tag, blob)
            .map_err(|_| UserError::IncorrectTypeLayout)?;
        let response: MoveResourceResponse = resource
            .try_into()
            .map_err(|_| UserError::MoveToJsonConversionFailed)?;
        Ok(response)
    }

    pub fn view_value(&self, tag: &TypeTag, blob: &[u8]) -> Result<MoveValueResponse> {
        let value = self
            .inner
            .view_value(tag, blob)
            .map_err(|_| UserError::IncorrectTypeLayout)?;
        let response: MoveValueResponse = value
            .try_into()
            .map_err(|_| UserError::MoveToJsonConversionFailed)?;
        Ok(response)
    }

    pub fn try_into_vm_value(&self, tag: &TypeTag, value: serde_json::Value) -> Result<MoveValue> {
        let layout = self
            .inner
            .get_type_layout_with_types(tag)
            .map_err(incorrect_type_layout)?;

        self.try_into_vm_value_from_layout(&layout, value)
    }

    pub fn try_into_vm_value_from_layout(
        &self,
        layout: &MoveTypeLayout,
        value: serde_json::Value,
    ) -> Result<MoveValue> {
        let value = match layout {
            MoveTypeLayout::Bool => {
                MoveValue::Bool(serde_json::from_value(value).map_err(incorrect_type_layout)?)
            }
            MoveTypeLayout::U8 => {
                MoveValue::U8(serde_json::from_value(value).map_err(incorrect_type_layout)?)
            }
            MoveTypeLayout::U16 => {
                MoveValue::U16(serde_json::from_value(value).map_err(incorrect_type_layout)?)
            }
            MoveTypeLayout::U32 => {
                MoveValue::U32(serde_json::from_value(value).map_err(incorrect_type_layout)?)
            }
            MoveTypeLayout::U64 => serde_json::from_value::<U64>(value)
                .map_err(incorrect_type_layout)?
                .into(),
            MoveTypeLayout::U128 => serde_json::from_value::<U128>(value)
                .map_err(incorrect_type_layout)?
                .into(),
            MoveTypeLayout::U256 => serde_json::from_value::<U256>(value)
                .map_err(incorrect_type_layout)?
                .into(),
            MoveTypeLayout::Address => serde_json::from_value::<Address>(value)
                .map_err(incorrect_type_layout)?
                .into(),
            MoveTypeLayout::Vector(item_layout) => {
                self.try_into_vm_value_vector(item_layout, value)?
            }
            MoveTypeLayout::Struct(struct_layout) => {
                self.try_into_vm_value_struct(struct_layout, value)?
            }
            MoveTypeLayout::Signer | MoveTypeLayout::Native(..) => {
                // Signer and Native types are not supported
                return Err(UserError::IncorrectTypeLayout.into());
            }
        };
        Ok(value)
    }

    pub fn try_into_vm_value_vector(
        &self,
        layout: &MoveTypeLayout,
        value: serde_json::Value,
    ) -> Result<MoveValue> {
        if matches!(layout, MoveTypeLayout::U8) {
            Ok(serde_json::from_value::<HexEncodedBytes>(value)
                .map_err(incorrect_type_layout)?
                .into())
        } else if let serde_json::Value::Array(list) = value {
            let vals = list
                .into_iter()
                .map(|v| self.try_into_vm_value_from_layout(layout, v))
                .collect::<Result<_>>()?;

            Ok(MoveValue::Vector(vals))
        } else {
            Err(UserError::IncorrectTypeLayout.into())
        }
    }

    pub fn try_into_vm_value_struct(
        &self,
        layout: &MoveStructLayout,
        value: serde_json::Value,
    ) -> Result<MoveValue> {
        let (struct_tag, field_layouts) =
            if let MoveStructLayout::WithTypes { type_, fields } = layout {
                (type_, fields)
            } else {
                return Err(UserError::IncorrectTypeLayout.into());
            };
        if MoveValueResponse::is_utf8_string(struct_tag) {
            let string = value
                .as_str()
                .ok_or_else(|| UserError::IncorrectTypeLayout)?;
            return Ok(aptos_api_types::new_vm_utf8_string(string));
        }

        let mut field_values = if let serde_json::Value::Object(fields) = value {
            fields
        } else {
            return Err(UserError::IncorrectTypeLayout.into());
        };
        let fields = field_layouts
            .iter()
            .map(|field_layout| {
                let name = field_layout.name.as_str();
                let value = field_values
                    .remove(name)
                    .ok_or_else(|| UserError::IncorrectTypeLayout)?;
                let move_value = self.try_into_vm_value_from_layout(&field_layout.layout, value)?;
                Ok(move_value)
            })
            .collect::<Result<_>>()?;

        Ok(MoveValue::Struct(MoveStruct::Runtime(fields)))
    }
}

fn incorrect_type_layout<E>(_e: E) -> UserError {
    UserError::IncorrectTypeLayout
}

/// Implement `CompiledModuleView` for any reference to a type that already
/// implements `CompiledModuleView`.
struct CompiledModuleViewReference<'a, V> {
    inner: &'a V,
}

impl<V: CompiledModuleView> CompiledModuleView for CompiledModuleViewReference<'_, V> {
    type Item = V::Item;

    fn view_compiled_module(
        &self,
        id: &move_core_types::language_storage::ModuleId,
    ) -> anyhow::Result<Option<Self::Item>> {
        self.inner.view_compiled_module(id)
    }
}
