//! Concrete HIR-to-MIR type transposition and synthesized type registries.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_hir::concrete as hir;
use scoop_mir as mir;

mod source_origin;

pub(super) use source_origin::{SourceExactTypeRegistry, owned_builtin_types};

pub(super) fn remap_idx<S, T>(id: la_arena::Idx<S>) -> la_arena::Idx<T> {
    la_arena::Idx::from_raw(id.into_raw())
}

pub(super) fn exact_function_identity(
    module: &hir::Module,
    function: mir::FunctionTypeId,
) -> (hir::ExactCallableSignature, hir::PersistentExactTypeId) {
    let source_id = remap_idx(function);
    let source = &module.function_types[source_id];
    let effect = if source.is_suspend {
        hir::Effect::Suspend
    } else {
        hir::Effect::Ordinary
    };
    let signature = hir::ExactCallableSignature::new(
        effect,
        None,
        source
            .parameter_types
            .iter()
            .map(|parameter| module.exact_type_identities[*parameter].id())
            .collect(),
        module.exact_type_identities[source.return_type].id(),
    );
    let exact_type = module.exact_type_identities[source.canonical_type].id();
    (signature, exact_type)
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
            name: name.clone(),
            type_arguments: args.clone(),
            methods: Vec::new(),
        });
        shell.interfaces.alloc(mir::InterfaceDef {
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
        exact_types: &mut SourceExactTypeRegistry,
        enums: &mut EnumRegistry,
        structs: &mut StructRegistry,
        interfaces: &mut InterfaceRegistry,
        shell: &mut mir::Module,
    ) -> mir::Type {
        let lowered = match &self.module.types[ty].kind {
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
                    .map(|&arg| self.lower(arg, exact_types, enums, structs, interfaces, shell))
                    .collect();
                mir::Type::Interface(interfaces.get_or_create(self.module, shell, *id, args))
            }
            hir::TypeKind::Any => mir::Type::Any,
            hir::TypeKind::Tuple(elements) => mir::Type::Tuple(
                elements
                    .iter()
                    .map(|&element| {
                        self.lower(element, exact_types, enums, structs, interfaces, shell)
                    })
                    .collect(),
            ),
            hir::TypeKind::Function(id) => mir::Type::Function(remap_idx(*id)),
            hir::TypeKind::Ptr(pointee) => mir::Type::Ptr(Box::new(self.lower(
                *pointee,
                exact_types,
                enums,
                structs,
                interfaces,
                shell,
            ))),
            hir::TypeKind::FunPtr(id) => mir::Type::FunPtr(remap_idx(*id)),
            hir::TypeKind::Enum(id) => {
                let args = self.module.enums[*id]
                    .type_arguments
                    .iter()
                    .map(|argument| {
                        self.lower(*argument, exact_types, enums, structs, interfaces, shell)
                    })
                    .collect::<Vec<_>>();
                let enum_id =
                    enums.get_or_create(self, exact_types, structs, interfaces, shell, *id);
                mir::Type::Enum(enum_id, args)
            }
        };
        exact_types.record(self.module, ty, lowered.clone());
        lowered
    }
}

/// Concrete enum definitions, transposed once from distinct local-concrete
/// HIR identities. Generic applications remain distinct through their typed
/// arguments; the declaration name is display-only.
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
        exact_types: &mut SourceExactTypeRegistry,
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
            name: name.clone(),
            type_arguments: Vec::new(),
            gc_free: decl.gc_free,
            variants: Vec::new(),
        });
        shell.enums.alloc(mir::EnumDef {
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
            .map(|argument| types.lower(*argument, exact_types, self, structs, interfaces, shell))
            .collect::<Vec<_>>();
        self.defs[id].type_arguments.clone_from(&type_arguments);
        shell.enums[id].type_arguments = type_arguments;
        let variants = decl
            .variants
            .iter()
            .map(|variant| mir::VariantDef {
                identity: variant.identity,
                name: variant.name.clone(),
                gc_free: variant.gc_free,
                fields: variant
                    .fields
                    .iter()
                    .map(|field| mir::VariantField {
                        identity: field.identity,
                        name: field.name.clone(),
                        ty: types.lower(field.ty, exact_types, self, structs, interfaces, shell),
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
    pub(super) entries: Vec<BoxedEntry>,
    pub(super) order: Vec<mir::ClassId>,
}

pub(super) struct BoxedEntry {
    pub(super) payload: mir::Type,
    pub(super) class: mir::ClassId,
    pub(super) payload_identity: hir::PersistentExactTypeId,
}

impl BoxedRegistry {
    pub(super) fn get_or_create(
        &mut self,
        classes: &mut Arena<mir::ClassDef>,
        shell: &mut mir::Module,
        payload: &mir::Type,
        payload_identity: hir::PersistentExactTypeId,
    ) -> mir::ClassId {
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.payload_identity == payload_identity)
        {
            assert_eq!(
                &entry.payload, payload,
                "one exact type identity must lower to one MIR type"
            );
            return entry.class;
        }
        assert!(
            self.entries.iter().all(|entry| entry.payload != *payload),
            "one MIR source type must retain one exact type identity"
        );
        let name = format!("box<{}>", mir::type_name(shell, payload));
        let id = classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
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
        self.entries.push(BoxedEntry {
            payload: payload.clone(),
            class: id,
            payload_identity,
        });
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
