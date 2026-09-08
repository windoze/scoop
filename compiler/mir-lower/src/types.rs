//! Concrete HIR-to-MIR type transposition and synthesized type registries.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_hir::concrete as hir;
use scoop_mir as mir;

pub(super) fn remap_idx<S, T>(id: la_arena::Idx<S>) -> la_arena::Idx<T> {
    la_arena::Idx::from_raw(id.into_raw())
}

pub(super) fn lower_nominal_link_stem(stem: &hir::NominalLinkStem) -> mir::NominalLinkStem {
    mir::NominalLinkStem::from_session_local_encoding(stem.as_str().to_string())
}

/// Closed roles for MIR-only nominal entities. Callers provide typed source
/// identities or MIR types; generated link stems never derive from display
/// names or arena ids masquerading as names.
pub(super) enum GeneratedNominalLinkRole<'a> {
    Boxed(&'a mir::Type),
    CoroutineStep(&'a mir::Type),
    CoroutineSlot(&'a mir::Type),
    CoroutineFrame {
        source_symbol: &'a str,
    },
    CoroutineAdapter {
        source_symbol: &'a str,
        state: mir::CoroutineSuspendStateId,
    },
    LambdaClosure {
        invoke_symbol: &'a str,
    },
    AnonymousClosure {
        invoke_symbol: &'a str,
    },
    CallableReferenceClosure(hir::CallableReferenceId),
    FunctionAdapterClosure {
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
    },
    DynamicFunctionAdapterClosure {
        target: mir::FunctionTypeId,
    },
}

pub(super) fn generated_nominal_link_stem(
    module: &mir::Module,
    role: GeneratedNominalLinkRole<'_>,
) -> mir::NominalLinkStem {
    fn field(output: &mut String, tag: char, value: &str) {
        use std::fmt::Write;
        write!(output, "{tag}{}:{value}", value.len()).expect("writing a String is infallible");
    }

    fn encoded_type(module: &mir::Module, ty: &mir::Type) -> String {
        mir::encode_type(module, ty).expect("generated nominals use source-level MIR types")
    }

    let mut encoding = String::from("$generated$");
    match role {
        GeneratedNominalLinkRole::Boxed(payload) => {
            field(&mut encoding, 'r', "box");
            field(&mut encoding, 't', &encoded_type(module, payload));
        }
        GeneratedNominalLinkRole::CoroutineStep(result) => {
            field(&mut encoding, 'r', "coroutine-step");
            field(&mut encoding, 't', &encoded_type(module, result));
        }
        GeneratedNominalLinkRole::CoroutineSlot(value) => {
            field(&mut encoding, 'r', "coroutine-slot");
            field(&mut encoding, 't', &encoded_type(module, value));
        }
        GeneratedNominalLinkRole::CoroutineFrame { source_symbol } => {
            field(&mut encoding, 'r', "coroutine-frame");
            field(&mut encoding, 'c', source_symbol);
        }
        GeneratedNominalLinkRole::CoroutineAdapter {
            source_symbol,
            state,
        } => {
            field(&mut encoding, 'r', "coroutine-adapter");
            field(&mut encoding, 'c', source_symbol);
            field(&mut encoding, 's', &state.get().to_string());
        }
        GeneratedNominalLinkRole::LambdaClosure { invoke_symbol } => {
            field(&mut encoding, 'r', "lambda-closure");
            field(&mut encoding, 'c', invoke_symbol);
        }
        GeneratedNominalLinkRole::AnonymousClosure { invoke_symbol } => {
            field(&mut encoding, 'r', "anonymous-closure");
            field(&mut encoding, 'c', invoke_symbol);
        }
        GeneratedNominalLinkRole::CallableReferenceClosure(reference) => {
            field(&mut encoding, 'r', "callable-reference-closure");
            field(
                &mut encoding,
                'i',
                &reference.into_raw().into_u32().to_string(),
            );
        }
        GeneratedNominalLinkRole::FunctionAdapterClosure { source, target } => {
            field(&mut encoding, 'r', "function-adapter-closure");
            field(
                &mut encoding,
                's',
                &encoded_type(module, &mir::Type::Function(source)),
            );
            field(
                &mut encoding,
                't',
                &encoded_type(module, &mir::Type::Function(target)),
            );
        }
        GeneratedNominalLinkRole::DynamicFunctionAdapterClosure { target } => {
            field(&mut encoding, 'r', "dynamic-function-adapter-closure");
            field(
                &mut encoding,
                't',
                &encoded_type(module, &mir::Type::Function(target)),
            );
        }
    }
    mir::NominalLinkStem::from_session_local_encoding(encoding)
}

pub(super) const fn lower_integer_kind(kind: hir::IntegerKind) -> mir::IntegerKind {
    let signedness = match kind.signedness() {
        hir::IntegerSignedness::Signed => mir::IntegerSignedness::Signed,
        hir::IntegerSignedness::Unsigned => mir::IntegerSignedness::Unsigned,
    };
    let width = match kind.width() {
        hir::IntegerWidth::W8 => mir::IntegerWidth::W8,
        hir::IntegerWidth::W16 => mir::IntegerWidth::W16,
        hir::IntegerWidth::W32 => mir::IntegerWidth::W32,
        hir::IntegerWidth::W64 => mir::IntegerWidth::W64,
    };
    mir::IntegerKind::new(signedness, width)
}

pub(super) const fn raise_integer_kind(kind: mir::IntegerKind) -> hir::IntegerKind {
    let signedness = match kind.signedness() {
        mir::IntegerSignedness::Signed => hir::IntegerSignedness::Signed,
        mir::IntegerSignedness::Unsigned => hir::IntegerSignedness::Unsigned,
    };
    let width = match kind.width() {
        mir::IntegerWidth::W8 => hir::IntegerWidth::W8,
        mir::IntegerWidth::W16 => hir::IntegerWidth::W16,
        mir::IntegerWidth::W32 => hir::IntegerWidth::W32,
        mir::IntegerWidth::W64 => hir::IntegerWidth::W64,
    };
    hir::IntegerKind::new(signedness, width)
}

pub(super) const fn lower_integer_constant(
    value: hir::HirIntegerConstant,
) -> mir::MirIntegerConstant {
    match value {
        hir::HirIntegerConstant::Signed8(bits) => mir::MirIntegerConstant::Signed8(bits),
        hir::HirIntegerConstant::Signed16(bits) => mir::MirIntegerConstant::Signed16(bits),
        hir::HirIntegerConstant::Signed32(bits) => mir::MirIntegerConstant::Signed32(bits),
        hir::HirIntegerConstant::Signed64(bits) => mir::MirIntegerConstant::Signed64(bits),
        hir::HirIntegerConstant::Unsigned8(bits) => mir::MirIntegerConstant::Unsigned8(bits),
        hir::HirIntegerConstant::Unsigned16(bits) => mir::MirIntegerConstant::Unsigned16(bits),
        hir::HirIntegerConstant::Unsigned32(bits) => mir::MirIntegerConstant::Unsigned32(bits),
        hir::HirIntegerConstant::Unsigned64(bits) => mir::MirIntegerConstant::Unsigned64(bits),
    }
}

/// Concrete interface applications. The MIR identity includes every type
/// argument because it is also the runtime TypeDescriptor / itable lookup key.
#[derive(Default)]
pub(super) struct InterfaceRegistry {
    pub(super) defs: Arena<mir::InterfaceDef>,
    instances: HashMap<mir::InterfaceId, (hir::InterfaceId, Vec<mir::Type>)>,
    by_hir: HashMap<hir::InterfaceId, mir::InterfaceId>,
}

impl InterfaceRegistry {
    pub(super) fn get_or_create(
        &mut self,
        module: &hir::Module,
        shell: &mut mir::Module,
        hir_id: hir::InterfaceId,
        args: Vec<mir::Type>,
    ) -> mir::InterfaceId {
        if let Some(&id) = self.by_hir.get(&hir_id) {
            return id;
        }
        let decl = &module.interfaces[hir_id];
        let name = decl.name.clone();
        let id = self.defs.alloc(mir::InterfaceDef {
            link_stem: lower_nominal_link_stem(&decl.link_stem),
            name: name.clone(),
            type_arguments: args.clone(),
            methods: Vec::new(),
        });
        shell.interfaces.alloc(mir::InterfaceDef {
            link_stem: lower_nominal_link_stem(&decl.link_stem),
            name: name.clone(),
            type_arguments: args.clone(),
            methods: Vec::new(),
        });
        self.instances.insert(id, (hir_id, args));
        self.by_hir.insert(hir_id, id);
        id
    }

    pub(super) fn source(&self, id: mir::InterfaceId) -> (hir::InterfaceId, &[mir::Type]) {
        let (hir, args) = &self.instances[&id];
        (*hir, args)
    }

    pub(super) fn mir_id(&self, id: hir::InterfaceId) -> mir::InterfaceId {
        self.by_hir[&id]
    }
}

/// Shared type-lowering context: the HIR type arena, the struct /
/// class / interface maps. Local-concrete HIR has no type parameters and no
/// substitution state.
#[derive(Clone, Copy)]
pub(super) struct Types<'a> {
    pub(super) module: &'a hir::Module,
    pub(super) struct_map: &'a HashMap<hir::StructId, mir::StructId>,
    pub(super) class_map: &'a HashMap<hir::ClassId, mir::ClassId>,
}

impl Types<'_> {
    /// Map a HIR type onto its MIR type. Aggregate shapes are
    /// preserved: structs keep their remapped concrete id, tuples keep their
    /// mapped element types, and concrete enum definitions are transposed on
    /// first reference. Reference types map onto their remapped ids.
    pub(super) fn lower(
        &self,
        ty: hir::TypeId,
        enums: &mut EnumRegistry,
        structs: &mut StructRegistry,
        interfaces: &mut InterfaceRegistry,
        shell: &mut mir::Module,
    ) -> mir::Type {
        match &self.module.types[ty].kind {
            hir::TypeKind::Unit => mir::Type::Unit,
            hir::TypeKind::Integer(kind) => mir::Type::Integer(lower_integer_kind(*kind)),
            hir::TypeKind::Boolean => mir::Type::Boolean,
            hir::TypeKind::String => mir::Type::String,
            hir::TypeKind::Struct(id) => mir::Type::Struct(self.struct_map[id]),
            hir::TypeKind::Class(id) => mir::Type::Class(self.class_map[id]),
            hir::TypeKind::Interface(id) => {
                let args = self.module.interfaces[*id]
                    .type_arguments
                    .iter()
                    .map(|&arg| self.lower(arg, enums, structs, interfaces, shell))
                    .collect();
                mir::Type::Interface(interfaces.get_or_create(self.module, shell, *id, args))
            }
            hir::TypeKind::Any => mir::Type::Any,
            hir::TypeKind::Tuple(elements) => mir::Type::Tuple(
                elements
                    .iter()
                    .map(|&element| self.lower(element, enums, structs, interfaces, shell))
                    .collect(),
            ),
            hir::TypeKind::Function(id) => mir::Type::Function(remap_idx(*id)),
            hir::TypeKind::Ptr(pointee) => mir::Type::Ptr(Box::new(
                self.lower(*pointee, enums, structs, interfaces, shell),
            )),
            hir::TypeKind::FunPtr(id) => mir::Type::FunPtr(remap_idx(*id)),
            hir::TypeKind::Enum(id) => {
                let args = self.module.enums[*id]
                    .type_arguments
                    .iter()
                    .map(|argument| self.lower(*argument, enums, structs, interfaces, shell))
                    .collect::<Vec<_>>();
                let enum_id = enums.get_or_create(self, structs, interfaces, shell, *id);
                mir::Type::Enum(enum_id, args)
            }
        }
    }
}

/// Concrete enum definitions, transposed once from distinct local-concrete
/// HIR identities (`Option$I`, or a plain non-generic name).
#[derive(Default)]
pub(super) struct EnumRegistry {
    pub(super) defs: Arena<mir::EnumDef>,
    by_hir: HashMap<hir::EnumId, mir::EnumId>,
    pub(super) hir_ids: HashMap<mir::EnumId, hir::EnumId>,
}

impl EnumRegistry {
    /// Build one semantic variant identity only after checking it against the
    /// fully transposed MIR enum definition.  Body lowering uses this store
    /// boundary instead of pairing enum ids and raw indices at expression
    /// sites.
    pub(super) fn variant_ref(&self, enum_id: mir::EnumId, variant: u32) -> mir::MirVariantRef {
        mir::MirVariantRef::new(&self.defs, enum_id, variant)
            .expect("local-concrete HIR variant identities are valid in the MIR enum store")
    }

    pub(super) fn lower_variant_ref(&self, source: hir::EnumVariantRef) -> mir::MirVariantRef {
        let enum_id = self.by_hir[&source.enumeration()];
        self.variant_ref(enum_id, source.variant().into_raw())
    }

    /// Bind a payload field to an already checked semantic variant.
    pub(super) fn variant_field_ref(
        &self,
        variant: mir::MirVariantRef,
        field: u32,
    ) -> mir::MirVariantFieldRef {
        mir::MirVariantFieldRef::new(&self.defs, variant, field)
            .expect("local-concrete HIR payload identities are valid in the MIR enum store")
    }

    pub(super) fn lower_variant_field_ref(
        &self,
        source: hir::EnumVariantFieldRef,
    ) -> mir::MirVariantFieldRef {
        let variant = self.lower_variant_ref(source.variant());
        self.variant_field_ref(variant, source.local_index())
    }

    pub(super) fn get_or_create(
        &mut self,
        types: &Types,
        structs: &mut StructRegistry,
        interfaces: &mut InterfaceRegistry,
        shell: &mut mir::Module,
        hir_id: hir::EnumId,
    ) -> mir::EnumId {
        if let Some(&id) = self.by_hir.get(&hir_id) {
            return id;
        }
        let decl = &types.module.enums[hir_id];
        let name = decl.name.clone();
        let id = self.defs.alloc(mir::EnumDef {
            link_stem: lower_nominal_link_stem(&decl.link_stem),
            name: name.clone(),
            type_arguments: Vec::new(),
            gc_free: decl.gc_free,
            variants: Vec::new(),
        });
        shell.enums.alloc(mir::EnumDef {
            link_stem: lower_nominal_link_stem(&decl.link_stem),
            name: name.clone(),
            type_arguments: Vec::new(),
            gc_free: decl.gc_free,
            variants: Vec::new(),
        });
        self.by_hir.insert(hir_id, id);
        self.hir_ids.insert(id, hir_id);
        let type_arguments = decl
            .type_arguments
            .iter()
            .map(|argument| types.lower(*argument, self, structs, interfaces, shell))
            .collect::<Vec<_>>();
        self.defs[id].type_arguments.clone_from(&type_arguments);
        shell.enums[id].type_arguments = type_arguments;
        let variants = decl
            .variants
            .iter()
            .map(|variant| mir::VariantDef {
                name: variant.name.clone(),
                gc_free: variant.gc_free,
                fields: variant
                    .fields
                    .iter()
                    .map(|field| mir::Field {
                        name: field.name.clone(),
                        ty: types.lower(field.ty, self, structs, interfaces, shell),
                    })
                    .collect(),
            })
            .collect();
        self.defs[id].variants = variants;
        id
    }

    pub(super) fn option_core(
        &self,
        module: &hir::Module,
        enum_id: mir::EnumId,
    ) -> Option<mir::OptionCore> {
        let &hir_id = self.hir_ids.get(&enum_id)?;
        let option = module.option_core(hir_id)?;
        let some_payload = self.lower_variant_field_ref(option.some_payload());
        let none = self.lower_variant_ref(option.none());
        mir::OptionCore::checked(&self.defs, some_payload, none)
    }

    pub(super) fn all_option_core(&self, module: &hir::Module) -> Vec<mir::OptionCore> {
        self.defs
            .iter()
            .filter_map(|(enum_id, _)| self.option_core(module, enum_id))
            .collect()
    }
}

/// Concrete struct definitions and their mandatory local-concrete HIR
/// provenance. Structs are transposed eagerly, so every MIR id has exactly one
/// source id before bodies can request boxing or interface lookup.
#[derive(Default)]
pub(super) struct StructRegistry {
    pub(super) defs: Arena<mir::StructDef>,
    pub(super) hir_ids: HashMap<mir::StructId, hir::StructId>,
}

/// Classify a MIR type while constructing compiler-synthesized concrete
/// aggregates. Source aggregates copy this mandatory bit from concrete HIR;
/// synthesized aggregates must derive it atomically with their definition.
pub(super) fn mir_type_gc_free(
    ty: &mir::Type,
    structs: &StructRegistry,
    enums: &EnumRegistry,
) -> bool {
    match ty {
        mir::Type::Unit
        | mir::Type::Integer(_)
        | mir::Type::MachineScalar(_)
        | mir::Type::Boolean
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_) => true,
        mir::Type::Struct(id) => structs.defs[*id].gc_free,
        mir::Type::Enum(id, _) => enums.defs[*id].gc_free,
        mir::Type::Tuple(elements) => elements
            .iter()
            .all(|element| mir_type_gc_free(element, structs, enums)),
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Any
        | mir::Type::Function(_) => false,
    }
}

/// Boxed value types, deduplicated by typed payload identity. The vtable and
/// itables are finalized after body lowering discovers all boxing sites.
#[derive(Default)]
pub(super) struct BoxedRegistry {
    pub(super) by_type: Vec<(mir::Type, mir::ClassId)>,
    pub(super) order: Vec<mir::ClassId>,
}

impl BoxedRegistry {
    pub(super) fn get_or_create(
        &mut self,
        classes: &mut Arena<mir::ClassDef>,
        shell: &mut mir::Module,
        payload: &mir::Type,
    ) -> mir::ClassId {
        if let Some((_, id)) = self.by_type.iter().find(|(found, _)| found == payload) {
            return *id;
        }
        let encoded = mir::encode_type(shell, payload)
            .expect("box payloads always use source-level MIR types");
        let name = format!("box${encoded}");
        let link_stem =
            generated_nominal_link_stem(shell, GeneratedNominalLinkRole::Boxed(payload));
        let id = classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            link_stem: link_stem.clone(),
            name: name.clone(),
            type_arguments: Vec::new(),
            representation: mir::ClassRepresentation::Declared {
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: payload.clone(),
                }],
                base_class: None,
            },
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        shell.classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            link_stem,
            name,
            type_arguments: Vec::new(),
            representation: mir::ClassRepresentation::Declared {
                fields: Vec::new(),
                base_class: None,
            },
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        self.by_type.push((payload.clone(), id));
        self.order.push(id);
        id
    }
}

/// Whether values of the type are boxed when they reach `Any` or an interface.
pub(super) fn is_boxable(ty: &mir::Type) -> bool {
    matches!(
        ty,
        mir::Type::Struct(_)
            | mir::Type::Enum(..)
            | mir::Type::Tuple(_)
            | mir::Type::Integer(_)
            | mir::Type::Boolean
            | mir::Type::Unit
    )
}

pub(super) fn is_reference_mir(ty: &mir::Type) -> bool {
    matches!(
        ty,
        mir::Type::String
            | mir::Type::Class(_)
            | mir::Type::Interface(_)
            | mir::Type::Function(_)
            | mir::Type::Any
    )
}
