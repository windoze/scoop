//! Concrete global storage, constants and native declarations.

use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_extern_functions(&mut self) {
        for (source_id, source) in self.source.extern_functions.iter() {
            let owner = self
                .source
                .functions
                .iter()
                .find_map(|(function_id, function)| {
                    matches!(&function.kind, export::FunctionKind::Extern(id) if *id == source_id)
                        .then_some(function_id)
                })
                .expect("every extern declaration has one source function owner");
            let source_contract = self
                .source
                .source_native_contracts
                .get(export::HirSourceNativeContractOwner::Function(owner))
                .expect("every extern function has a source-native contract")
                .clone();
            let params = source
                .params
                .iter()
                .map(|ty| self.lower_type(*ty, &[]))
                .collect();
            let return_type = self.lower_type(source.return_type, &[]);
            let id = self.extern_functions.alloc(concrete::ExternFunction {
                source_contract,
                source_name: source.source_name.clone(),
                native_symbol: source.native_symbol.clone(),
                library: source.library.clone(),
                abi: source.abi,
                calling_convention: source.calling_convention,
                gc_effect: source.gc_effect,
                safety: source.safety,
                params,
                return_type,
            });
            self.extern_map.insert(source_id, id);
        }
    }

    pub(super) fn lower_globals(&mut self) {
        // Allocate ids first because expressions in function bodies may refer
        // to any global regardless of declaration order.
        for (source_id, source) in self.source.globals.iter() {
            let ty = self.lower_type(source.ty, &[]);
            let storage = self.lower_global_storage(source_id, &source.storage);
            let storage_owner = self.property_storage_owner(source_id, source);
            let id = self.globals.alloc(concrete::Global {
                name: source.name.clone(),
                storage_owner,
                ty,
                mutable: source.mutable,
                storage,
                span: source.span,
            });
            self.global_map.insert(source_id, id);
        }
    }

    fn property_storage_owner(
        &self,
        global_id: export::GlobalId,
        global: &export::Global,
    ) -> concrete::PropertyStorageOwner {
        let property = &self.source.properties[global.property];
        let owner = self.source.property_identities[global.property].property_owner();
        match &property.representation {
            export::PropertyRepresentation::Stored(export::StoredProperty {
                backing: export::PropertyBacking::TopLevelGlobal { storage, .. },
            }) if *storage == global_id => concrete::PropertyStorageOwner::Backing(owner),
            export::PropertyRepresentation::Delegated { storage }
                if matches!(
                    self.source.delegate_storages[*storage].location,
                    export::DelegateStorageLocation::ManagedGlobal(id) if id == global_id
                ) =>
            {
                concrete::PropertyStorageOwner::Delegate(owner)
            }
            export::PropertyRepresentation::NativeStorage { storage } if *storage == global_id => {
                concrete::PropertyStorageOwner::Backing(owner)
            }
            export::PropertyRepresentation::Stored(_)
            | export::PropertyRepresentation::AccessorOnly
            | export::PropertyRepresentation::Delegated { .. }
            | export::PropertyRepresentation::GenericDelegated { .. }
            | export::PropertyRepresentation::Const { .. }
            | export::PropertyRepresentation::NativeStorage { .. } => {
                unreachable!(
                    "validated Export HIR binds every global to its physical property role"
                )
            }
        }
    }

    pub(super) fn lower_global_storage(
        &mut self,
        global: export::GlobalId,
        storage: &export::GlobalStorage,
    ) -> concrete::GlobalStorage {
        match storage {
            export::GlobalStorage::Managed { state } => concrete::GlobalStorage::Managed {
                state: match state {
                    export::HirStaticInitialState::EncodedStaticValue { payload } => {
                        concrete::HirStaticInitialState::EncodedStaticValue {
                            payload: self.lower_constant(payload),
                        }
                    }
                    export::HirStaticInitialState::ZeroedForRuntimeUnit { unit } => {
                        concrete::HirStaticInitialState::ZeroedForRuntimeUnit {
                            unit: self.request_initialization_unit(*unit),
                        }
                    }
                },
            },
            export::GlobalStorage::Local {
                thread_local,
                initializer,
            } => concrete::GlobalStorage::Local {
                thread_local: *thread_local,
                initializer: self.lower_constant(initializer),
            },
            export::GlobalStorage::Extern {
                library,
                native_symbol,
                thread_local,
            } => concrete::GlobalStorage::Extern {
                source_contract: Box::new(
                    self.source
                        .source_native_contracts
                        .get(export::HirSourceNativeContractOwner::Global(global))
                        .expect("every extern global has a source-native contract")
                        .clone(),
                ),
                library: library.clone(),
                native_symbol: native_symbol.clone(),
                thread_local: *thread_local,
            },
        }
    }

    pub(super) fn lower_constant(
        &mut self,
        value: &export::HirConstantImage,
    ) -> concrete::HirConstantImage {
        match value {
            export::HirConstantImage::ImportedEnumUnit { ty, variant } => {
                let ty = self.lower_type(*ty, &[]);
                let concrete::TypeKind::Enum(enumeration) = self.types[ty].kind else {
                    unreachable!("an imported enum constant retains its enum type")
                };
                let index = self.enums[enumeration]
                    .variants
                    .iter()
                    .position(|value| value.identity == *variant)
                    .expect("an imported enum constant retains its declared unit variant");
                concrete::HirConstantImage::EnumUnit {
                    variant: concrete::EnumVariantRef::checked(
                        &self.enums,
                        enumeration,
                        concrete::VariantId::from_raw(index as u32),
                    )
                    .expect("the unit variant belongs to the resolved enum"),
                }
            }
            export::HirConstantImage::ImportedStruct { ty, fields } => {
                let ty = self.lower_type(*ty, &[]);
                let concrete::TypeKind::Struct(struct_id) = self.types[ty].kind else {
                    unreachable!("an imported struct constant retains its struct type")
                };
                concrete::HirConstantImage::Struct {
                    struct_id,
                    fields: fields
                        .iter()
                        .map(|field| self.lower_constant(field))
                        .collect(),
                }
            }
            export::HirConstantImage::Integer(value) => concrete::HirConstantImage::Integer(*value),
            export::HirConstantImage::Boolean(value) => concrete::HirConstantImage::Boolean(*value),
            export::HirConstantImage::String(value) => {
                concrete::HirConstantImage::String(value.clone())
            }
            export::HirConstantImage::NullPointer(kind) => {
                concrete::HirConstantImage::NullPointer(match kind {
                    export::HirPointerNullKind::Raw => concrete::HirPointerNullKind::Raw,
                    export::HirPointerNullKind::Code => concrete::HirPointerNullKind::Code,
                })
            }
            export::HirConstantImage::EnumUnit { variant } => {
                concrete::HirConstantImage::EnumUnit {
                    variant: self.lower_applied_enum_variant_ref(*variant, &[]),
                }
            }
            export::HirConstantImage::Struct {
                application,
                fields,
            } => {
                let struct_id = self.lower_struct_application(*application, &[]);
                concrete::HirConstantImage::Struct {
                    struct_id,
                    fields: fields
                        .iter()
                        .map(|field| self.lower_constant(field))
                        .collect(),
                }
            }
        }
    }
}
