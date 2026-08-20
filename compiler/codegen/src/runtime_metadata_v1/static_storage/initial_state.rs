use std::collections::BTreeMap;

use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::values::{BasicValueEnum, GlobalValue, UnnamedAddress};
use scoop_lir::{
    StaticStorageRelocationTableArtifactV1, StrongStaticStorageInitialArtifactPlanV1,
    StrongStaticStorageInitialStatePlanV1, StrongStaticStorageRegistrationPlanV1,
};

use super::RuntimeMetadataV1Types;
use crate::CodegenError;

/// The emitted backing storage for a static initial-state relocation span.
#[derive(Clone, Copy, Debug)]
pub enum EmittedStaticStorageRelocationTableV1<'ctx> {
    SharedEmptySentinel {
        global: GlobalValue<'ctx>,
    },
    Defined {
        atom: scoop_lir::ObjectDefinitionAtomId,
        global: GlobalValue<'ctx>,
    },
}

impl<'ctx> EmittedStaticStorageRelocationTableV1<'ctx> {
    pub const fn global(self) -> GlobalValue<'ctx> {
        match self {
            Self::SharedEmptySentinel { global } | Self::Defined { global, .. } => global,
        }
    }
}

/// The emitted initial-state artifacts referenced by one static descriptor.
#[derive(Clone, Copy, Debug)]
pub enum EmittedStaticStorageInitialStateV1<'ctx> {
    ZeroedForRuntimeUnit {
        template_sentinel: GlobalValue<'ctx>,
        relocations: EmittedStaticStorageRelocationTableV1<'ctx>,
    },
    EncodedStaticValue {
        template_atom: scoop_lir::ObjectDefinitionAtomId,
        template: GlobalValue<'ctx>,
        relocations: EmittedStaticStorageRelocationTableV1<'ctx>,
    },
}

impl<'ctx> EmittedStaticStorageInitialStateV1<'ctx> {
    pub const fn template(self) -> GlobalValue<'ctx> {
        match self {
            Self::ZeroedForRuntimeUnit {
                template_sentinel, ..
            } => template_sentinel,
            Self::EncodedStaticValue { template, .. } => template,
        }
    }

    pub const fn relocations(self) -> EmittedStaticStorageRelocationTableV1<'ctx> {
        match self {
            Self::ZeroedForRuntimeUnit { relocations, .. }
            | Self::EncodedStaticValue { relocations, .. } => relocations,
        }
    }
}

pub(super) fn emit_initial_state<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    plan: &StrongStaticStorageRegistrationPlanV1,
    empty_template: GlobalValue<'ctx>,
    empty_relocations: GlobalValue<'ctx>,
    immortal_registrations: &mut BTreeMap<String, GlobalValue<'ctx>>,
) -> Result<EmittedStaticStorageInitialStateV1<'ctx>, CodegenError> {
    match (plan.semantic().initial_state(), plan.initial_artifacts()) {
        (
            StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit,
            StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit,
        ) => Ok(EmittedStaticStorageInitialStateV1::ZeroedForRuntimeUnit {
            template_sentinel: empty_template,
            relocations: EmittedStaticStorageRelocationTableV1::SharedEmptySentinel {
                global: empty_relocations,
            },
        }),
        (
            StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
                initial_template,
                immortal_relocations,
            },
            StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
                template_atom,
                relocation_table,
            },
        ) => {
            let bytes = initial_template
                .iter()
                .map(|byte| context.i8_type().const_int(u64::from(*byte), false))
                .collect::<Vec<_>>();
            let value = context.i8_type().const_array(&bytes);
            let template = emit_private_constant(llvm, &template_symbol(plan), value.into());
            let relocations = match (immortal_relocations.as_slice(), relocation_table) {
                ([], StaticStorageRelocationTableArtifactV1::SharedEmptySentinel) => {
                    EmittedStaticStorageRelocationTableV1::SharedEmptySentinel {
                        global: empty_relocations,
                    }
                }
                (relocations, StaticStorageRelocationTableArtifactV1::Defined { atom })
                    if relocations.len() == plan.immortal_registration_symbols().len() =>
                {
                    let values = relocations
                        .iter()
                        .zip(plan.immortal_registration_symbols())
                        .map(|(relocation, symbol)| {
                            let target = require_or_declare_immortal_registration(
                                llvm,
                                types,
                                *symbol,
                                immortal_registrations,
                            );
                            types.static_immortal_relocation.const_named_struct(&[
                                context
                                    .i64_type()
                                    .const_int(relocation.pointer_offset(), false)
                                    .into(),
                                target.as_pointer_value().into(),
                            ])
                        })
                        .collect::<Vec<_>>();
                    let value = types.static_immortal_relocation.const_array(&values);
                    EmittedStaticStorageRelocationTableV1::Defined {
                        atom,
                        global: emit_private_constant(llvm, &relocation_symbol(plan), value.into()),
                    }
                }
                _ => {
                    return Err(CodegenError(format!(
                        "static storage `{}` initial-state artifact plan disagrees with its relocation set",
                        plan.semantic().symbol().symbol()
                    )));
                }
            };
            Ok(EmittedStaticStorageInitialStateV1::EncodedStaticValue {
                template_atom,
                template,
                relocations,
            })
        }
        _ => Err(CodegenError(format!(
            "static storage `{}` initial-state semantics disagree with its artifact plan",
            plan.semantic().symbol().symbol()
        ))),
    }
}

fn require_or_declare_immortal_registration<'ctx>(
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    request: scoop_lir::PersistentSymbolRequest,
    registrations: &mut BTreeMap<String, GlobalValue<'ctx>>,
) -> GlobalValue<'ctx> {
    let name = request.symbol().as_str().to_string();
    if let Some(global) = registrations.get(&name).copied() {
        return global;
    }
    let global = llvm.get_global(&name).unwrap_or_else(|| {
        let global = llvm.add_global(types.immortal_object_descriptor, None, &name);
        global.set_linkage(Linkage::External);
        global
    });
    registrations.insert(name, global);
    global
}

pub(super) fn emit_private_constant<'ctx>(
    llvm: &LlvmModule<'ctx>,
    symbol: &str,
    value: BasicValueEnum<'ctx>,
) -> GlobalValue<'ctx> {
    let global = llvm.add_global(value.get_type(), None, symbol);
    global.set_linkage(Linkage::Private);
    global.set_constant(true);
    global.set_unnamed_address(UnnamedAddress::None);
    global.set_initializer(&value);
    global
}

pub(super) fn template_symbol(plan: &StrongStaticStorageRegistrationPlanV1) -> String {
    format!(
        "{}.initial_template",
        plan.semantic().symbol().symbol().as_str()
    )
}

pub(super) fn relocation_symbol(plan: &StrongStaticStorageRegistrationPlanV1) -> String {
    format!(
        "{}.initial_relocations",
        plan.semantic().symbol().symbol().as_str()
    )
}
