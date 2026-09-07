//! Typed monomorphized-function provenance and MIR metadata construction.

use super::*;

fn method_owner_type_arguments(module: &hir::Module, owner: hir::MethodOwner) -> &[hir::TypeId] {
    match owner {
        hir::MethodOwner::Class(id) => &module.classes[id].type_arguments,
        hir::MethodOwner::Struct(id) => &module.structs[id].type_arguments,
        hir::MethodOwner::Enum(id) => &module.enums[id].type_arguments,
        hir::MethodOwner::Interface(id) => &module.interfaces[id].type_arguments,
        hir::MethodOwner::Object(_) => &[],
        hir::MethodOwner::Structural(_) => &[],
    }
}

/// Exact specialization data emitted by HIR. The variants preserve source
/// category and owner/method argument grouping. `None` means this declaration
/// uses ordinary overload mangling; it never means "unknown".
#[derive(Debug, Clone)]
pub(super) enum FunctionInstance {
    GenericFunction {
        origin: hir::GenericFunctionOriginId,
        arguments: hir::NonEmptyVec<hir::TypeId>,
    },
    ParameterizedMethod {
        origin: hir::OwnerParameterizedMethodOriginId,
        owner: hir::MethodOwner,
        owner_arguments: hir::NonEmptyVec<hir::TypeId>,
    },
    GenericMethod {
        origin: hir::GenericMethodOriginId,
        owner: hir::MethodOwner,
        owner_arguments: Vec<hir::TypeId>,
        method_arguments: hir::NonEmptyVec<hir::TypeId>,
    },
}

pub(super) enum LoweredFunctionInstance {
    GenericFunction {
        origin: hir::GenericFunctionOriginId,
        arguments: mir::NonEmptyTypeArguments,
    },
    ParameterizedMethod {
        origin: hir::OwnerParameterizedMethodOriginId,
        owner: mir::MonomorphizedMethodOwner,
        owner_arguments: mir::NonEmptyTypeArguments,
    },
    GenericMethod {
        origin: hir::GenericMethodOriginId,
        owner: mir::MonomorphizedMethodOwner,
        owner_arguments: Vec<mir::Type>,
        method_arguments: mir::NonEmptyTypeArguments,
    },
}

fn non_empty_mir_arguments(arguments: Vec<mir::Type>) -> mir::NonEmptyTypeArguments {
    let mut arguments = arguments.into_iter();
    let first = arguments
        .next()
        .expect("HIR non-empty type arguments remain non-empty after lowering");
    mir::NonEmptyTypeArguments::new(first, arguments.collect())
}

impl FunctionInstance {}

pub(super) fn function_instance(
    module: &hir::Module,
    function: &hir::Function,
) -> Option<FunctionInstance> {
    match &function.origin {
        hir::FunctionOrigin::Free(hir::FreeFunctionOrigin::Plain) => None,
        hir::FunctionOrigin::Free(hir::FreeFunctionOrigin::Generic {
            origin, arguments, ..
        }) => Some(FunctionInstance::GenericFunction {
            origin: *origin,
            arguments: arguments.clone(),
        }),
        hir::FunctionOrigin::Method(method) => match &method.specialization {
            hir::MethodSpecialization::Plain => None,
            hir::MethodSpecialization::OwnerParameterized { origin, .. } => {
                Some(FunctionInstance::ParameterizedMethod {
                    origin: *origin,
                    owner: method.owner,
                    owner_arguments: hir::NonEmptyVec::from_vec(
                        method_owner_type_arguments(module, method.owner).to_vec(),
                    )
                    .expect("an owner-parameterized method has owner arguments"),
                })
            }
            hir::MethodSpecialization::Generic {
                origin,
                method_arguments,
                ..
            } => Some(FunctionInstance::GenericMethod {
                origin: *origin,
                owner: method.owner,
                owner_arguments: method_owner_type_arguments(module, method.owner).to_vec(),
                method_arguments: method_arguments.clone(),
            }),
        },
    }
}

impl Lowerer {
    fn lower_instance_arguments(
        &mut self,
        module: &hir::Module,
        arguments: &[hir::TypeId],
    ) -> Vec<mir::Type> {
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        arguments
            .iter()
            .map(|argument| {
                types.lower(
                    *argument,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                )
            })
            .collect()
    }

    pub(super) fn record_function_instance(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
        mir_id: mir::FunctionId,
    ) {
        let Some(instance) = function_instance(module, &module.functions[hir_id]) else {
            return;
        };
        let instance = match instance {
            FunctionInstance::GenericFunction {
                origin, arguments, ..
            } => LoweredFunctionInstance::GenericFunction {
                origin,
                arguments: non_empty_mir_arguments(
                    self.lower_instance_arguments(module, &arguments.to_vec()),
                ),
            },
            FunctionInstance::ParameterizedMethod {
                origin,
                owner,
                owner_arguments,
                ..
            } => LoweredFunctionInstance::ParameterizedMethod {
                origin,
                owner: self.lower_instance_method_owner(module, owner),
                owner_arguments: non_empty_mir_arguments(
                    self.lower_instance_arguments(module, &owner_arguments.to_vec()),
                ),
            },
            FunctionInstance::GenericMethod {
                origin,
                owner,
                owner_arguments,
                method_arguments,
                ..
            } => LoweredFunctionInstance::GenericMethod {
                origin,
                owner: self.lower_instance_method_owner(module, owner),
                owner_arguments: self.lower_instance_arguments(module, &owner_arguments),
                method_arguments: non_empty_mir_arguments(
                    self.lower_instance_arguments(module, &method_arguments.to_vec()),
                ),
            },
        };
        self.instances.record(
            hir_id,
            mir_id,
            self.functions[mir_id].symbol.clone(),
            self.functions[mir_id].name.clone(),
            instance,
        );
    }

    fn lower_instance_method_owner(
        &mut self,
        module: &hir::Module,
        owner: hir::MethodOwner,
    ) -> mir::MonomorphizedMethodOwner {
        match owner {
            hir::MethodOwner::Class(id) => {
                mir::MonomorphizedMethodOwner::Class(self.class_map[&id])
            }
            hir::MethodOwner::Struct(id) => {
                mir::MonomorphizedMethodOwner::Struct(self.struct_map[&id])
            }
            hir::MethodOwner::Enum(id) => {
                let types = Types {
                    module,
                    struct_map: &self.struct_map,
                    class_map: &self.class_map,
                };
                let id = self.enums.get_or_create(
                    &types,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                    id,
                );
                mir::MonomorphizedMethodOwner::Enum(id)
            }
            hir::MethodOwner::Interface(id) => {
                mir::MonomorphizedMethodOwner::Interface(self.interfaces.mir_id(id))
            }
            hir::MethodOwner::Object(id) => {
                mir::MonomorphizedMethodOwner::Object(mir::ObjectTypeId::from_raw(id.into_raw()))
            }
            hir::MethodOwner::Structural(ty) => {
                let ty = Types {
                    module,
                    struct_map: &self.struct_map,
                    class_map: &self.class_map,
                }
                .lower(
                    ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                );
                mir::MonomorphizedMethodOwner::Structural(ty)
            }
        }
    }
}

/// Metadata for functions that HIR has already fully instantiated. MIR never
/// owns an instantiation worklist; it only records the concrete source mapping.
#[derive(Default)]
pub(super) struct InstanceRegistry {
    by_function: HashMap<hir::FunctionId, mir::MonomorphizedFunctionId>,
    generic_function_by_origin: HashMap<hir::GenericFunctionOriginId, mir::GenericFunctionSourceId>,
    parameterized_method_by_origin:
        HashMap<hir::OwnerParameterizedMethodOriginId, mir::ParameterizedMethodSourceId>,
    generic_method_by_origin: HashMap<hir::GenericMethodOriginId, mir::GenericMethodSourceId>,
    pub(super) generic_function_sources: Arena<mir::GenericFunctionSource>,
    pub(super) parameterized_method_sources: Arena<mir::ParameterizedMethodSource>,
    pub(super) generic_method_sources: Arena<mir::GenericMethodSource>,
    pub(super) meta: Arena<mir::MonomorphizedFunction>,
}

impl InstanceRegistry {
    pub(super) fn record(
        &mut self,
        hir_function: hir::FunctionId,
        function: mir::FunctionId,
        symbol: String,
        name: String,
        instance: LoweredFunctionInstance,
    ) -> mir::MonomorphizedFunctionId {
        let source = match instance {
            LoweredFunctionInstance::GenericFunction { origin, arguments } => {
                let source = *self
                    .generic_function_by_origin
                    .entry(origin)
                    .or_insert_with(|| {
                        self.generic_function_sources
                            .alloc(mir::GenericFunctionSource {
                                display_name: name.clone(),
                            })
                    });
                mir::MonomorphizedSource::GenericFunction { source, arguments }
            }
            LoweredFunctionInstance::ParameterizedMethod {
                origin,
                owner,
                owner_arguments,
            } => {
                let source = *self
                    .parameterized_method_by_origin
                    .entry(origin)
                    .or_insert_with(|| {
                        self.parameterized_method_sources
                            .alloc(mir::ParameterizedMethodSource {
                                display_name: name.clone(),
                            })
                    });
                mir::MonomorphizedSource::ParameterizedMethod {
                    source,
                    owner,
                    owner_arguments,
                }
            }
            LoweredFunctionInstance::GenericMethod {
                origin,
                owner,
                owner_arguments,
                method_arguments,
            } => {
                let source = *self
                    .generic_method_by_origin
                    .entry(origin)
                    .or_insert_with(|| {
                        self.generic_method_sources.alloc(mir::GenericMethodSource {
                            display_name: name.clone(),
                        })
                    });
                mir::MonomorphizedSource::GenericMethod {
                    source,
                    owner,
                    owner_arguments,
                    method_arguments,
                }
            }
        };
        let id = self.meta.alloc(mir::MonomorphizedFunction {
            function,
            symbol,
            source,
        });
        assert!(self.by_function.insert(hir_function, id).is_none());
        id
    }

    pub(super) fn get(&self, source: hir::FunctionId) -> Option<mir::MonomorphizedFunctionId> {
        self.by_function.get(&source).copied()
    }
}
