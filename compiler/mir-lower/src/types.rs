//! Concrete HIR-to-MIR type transposition and synthesized type registries.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_hir::concrete as hir;
use scoop_mir as mir;

fn remap_idx<S, T>(id: la_arena::Idx<S>) -> la_arena::Idx<T> {
    la_arena::Idx::from_raw(id.into_raw())
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
            methods: Vec::new(),
        });
        shell.interfaces.alloc(mir::InterfaceDef {
            name: name.clone(),
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
            hir::TypeKind::Int => mir::Type::Int,
            hir::TypeKind::UInt => mir::Type::UInt,
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
            name: name.clone(),
            gc_free: decl.gc_free,
            variants: Vec::new(),
        });
        shell.enums.alloc(mir::EnumDef {
            name: name.clone(),
            gc_free: decl.gc_free,
            variants: Vec::new(),
        });
        self.by_hir.insert(hir_id, id);
        self.hir_ids.insert(id, hir_id);
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
        | mir::Type::Int
        | mir::Type::UInt
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
        let name = format!("box${}", mir::encode_type(shell, payload));
        let id = classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: name.clone(),
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
            | mir::Type::Int
            | mir::Type::UInt
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
