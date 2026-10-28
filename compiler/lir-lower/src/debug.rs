use super::*;
use lir::{DebugEncoding, DebugField, DebugType, DebugTypeKind, DebugTypeRef};
use scoop_identity::PersistentExactTypeId;
use std::collections::{BTreeMap, HashSet};

mod enumerations;
mod objects;

pub(super) fn types(
    context: &LoweringContext,
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    layouts: &Arena<lir::Layout>,
    arrays: &Arena<lir::ArrayType>,
) -> StorageResult<lir::DebugTypes> {
    let mut builder = Types {
        context,
        module,
        structs,
        enums,
        layouts: layouts
            .values()
            .map(|layout| (layout.identity.layout_record().key().exact_type(), layout))
            .collect(),
        arrays: arrays
            .values()
            .map(|array| (array.identity.layout_record().key().exact_type(), array))
            .collect(),
        visited: HashSet::new(),
        types: BTreeMap::new(),
    };
    for function in module
        .functions
        .values()
        .chain(module.release_hooks.values().map(|hook| &hook.code))
    {
        if matches!(function.debug_origin, mir::DebugOrigin::Source(_))
            || function
                .body
                .locals
                .values()
                .any(|local| matches!(local.debug_variable, mir::DebugVariable::Source { .. }))
        {
            for parameter in &function.params {
                builder.source(&parameter.ty)?;
            }
            if function.return_ty != mir::Type::Unit {
                builder.source(&function.return_ty)?;
            }
        }
        for local in function.body.locals.values() {
            if matches!(local.debug_variable, mir::DebugVariable::Source { .. }) {
                builder.source(&local.ty)?;
            }
        }
    }
    for global in module.globals.values() {
        if matches!(global.debug_origin, mir::DebugOrigin::Source(_)) {
            builder.source(&global.ty)?;
        }
    }
    Ok(builder.types)
}

struct Types<'a> {
    context: &'a LoweringContext,
    module: &'a mir::Module,
    structs: &'a lir::StructDefs,
    enums: &'a lir::EnumDefs,
    layouts: HashMap<PersistentExactTypeId, &'a lir::Layout>,
    arrays: HashMap<PersistentExactTypeId, &'a lir::ArrayType>,
    visited: HashSet<PersistentExactTypeId>,
    types: lir::DebugTypes,
}

impl Types<'_> {
    fn source(&mut self, ty: &mir::Type) -> StorageResult<DebugTypeRef> {
        let id = exact_type_record(self.module, ty).id();
        if self.visited.insert(id) {
            let mut shape = self.describe(ty)?;
            shape.name = mir::type_name(self.module, ty);
            if let DebugTypeKind::Pointer(DebugTypeRef::Inline(object)) = &mut shape.kind
                && matches!(object.kind, DebugTypeKind::Record(_))
            {
                object.name.clone_from(&shape.name);
            }
            self.types.insert(id, shape);
        }
        Ok(DebugTypeRef::Source(id))
    }

    fn shape(&self, ty: &mir::Type) -> StorageResult<(u64, u64)> {
        size_align(
            self.context,
            self.module,
            &|id| Ok(repr_shape(self.context, &self.enums[enum_def_id(id)].repr)),
            ty,
        )
    }

    fn describe(&mut self, ty: &mir::Type) -> StorageResult<DebugType> {
        let (size, align) = self.shape(ty)?;
        let (name, kind) = match ty {
            mir::Type::Unit => ("Unit".into(), DebugTypeKind::Record(Vec::new())),
            mir::Type::Integer(kind) => (
                integer_name(*kind),
                DebugTypeKind::Basic(integer_encoding(*kind)),
            ),
            mir::Type::Boolean => (
                "Boolean".into(),
                DebugTypeKind::Basic(DebugEncoding::Boolean),
            ),
            mir::Type::MachineScalar(_) => (
                "__scoop_machine".into(),
                DebugTypeKind::Basic(DebugEncoding::Unsigned),
            ),
            mir::Type::Context(storage) => {
                let kind = match storage.role {
                    mir::ContextStorageRole::Mark | mir::ContextStorageRole::SwitchGuard => {
                        let pointer = self.context.pointer_layout(lir::PointerKind::Managed);
                        let fields = (0..size / pointer.size)
                            .map(|index| {
                                field(
                                    &format!("__scoop_{index}"),
                                    index * pointer.size,
                                    self.address(),
                                )
                            })
                            .collect();
                        DebugTypeKind::Record(fields)
                    }
                    mir::ContextStorageRole::Task
                    | mir::ContextStorageRole::Node
                    | mir::ContextStorageRole::Binding => self.opaque_pointer(),
                };
                ("__scoop_context".into(), kind)
            }
            mir::Type::String => (
                "String".into(),
                DebugTypeKind::Pointer(inline(self.string_object())),
            ),
            mir::Type::Any => (
                "Any".into(),
                DebugTypeKind::Pointer(inline(self.header_object("Any"))),
            ),
            mir::Type::Class(id) => {
                let definition = &self.module.classes[*id];
                let object = self.class_object(*id)?;
                (
                    definition.name.clone(),
                    DebugTypeKind::Pointer(inline(object)),
                )
            }
            mir::Type::Interface(id) => {
                let pointer = self.context.pointer_layout(lir::PointerKind::Managed);
                (
                    self.module.interfaces[*id].name.clone(),
                    DebugTypeKind::Record(vec![
                        field(
                            "__scoop_object",
                            0,
                            inline(self.pointer("Any", inline(self.header_object("Any")))),
                        ),
                        field("__scoop_itable", pointer.size, self.address()),
                    ]),
                )
            }
            mir::Type::Ptr(pointee) => {
                ("Ptr".into(), DebugTypeKind::Pointer(self.source(pointee)?))
            }
            mir::Type::FunPtr(signature) => (
                "FunPtr".into(),
                DebugTypeKind::Pointer(inline(self.signature(*signature)?)),
            ),
            mir::Type::Function(signature) => {
                let (offset, layout) = self.context.closure_prefix();
                let mut fields = self.header_fields();
                let signature = self.signature(*signature)?;
                fields.push(field(
                    "__scoop_invoke",
                    offset,
                    inline(self.pointer("", inline(signature))),
                ));
                (
                    "Function".into(),
                    DebugTypeKind::Pointer(inline(DebugType {
                        name: "Function".into(),
                        size: layout.size,
                        align: layout.align,
                        kind: DebugTypeKind::Record(fields),
                    })),
                )
            }
            mir::Type::Tuple(elements) => {
                let (offsets, _, _) = aggregate_shape(
                    self.context,
                    self.module,
                    &|id| Ok(repr_shape(self.context, &self.enums[enum_def_id(id)].repr)),
                    elements,
                )?;
                let mut fields = Vec::new();
                for ((index, element), position) in elements.iter().enumerate().zip(offsets) {
                    fields.push(field(
                        &format!("_{}", index + 1),
                        position,
                        self.source(element)?,
                    ));
                }
                ("Tuple".into(), DebugTypeKind::Record(fields))
            }
            mir::Type::Struct(id) => {
                let definition = &self.module.structs[*id];
                let kind = match &definition.representation {
                    mir::StructRepresentation::Declared(declared) => {
                        let layout = &self.structs[struct_def_id(*id)];
                        let mut fields = Vec::new();
                        for (index, member) in declared.fields.iter().enumerate() {
                            let offset = layout
                                .field_layout(index)
                                .expect("the actual struct layout contains every source field")
                                .offset;
                            fields.push(field(&member.name, offset, self.source(&member.ty)?));
                        }
                        DebugTypeKind::Record(fields)
                    }
                    mir::StructRepresentation::Intrinsic(representation) => match representation {
                        mir::IntrinsicStructRepresentation::Char => {
                            DebugTypeKind::Basic(DebugEncoding::Unicode)
                        }
                        mir::IntrinsicStructRepresentation::Float(_) => {
                            DebugTypeKind::Basic(DebugEncoding::Float)
                        }
                        mir::IntrinsicStructRepresentation::Integer(kind) => {
                            DebugTypeKind::Basic(integer_encoding(*kind))
                        }
                        mir::IntrinsicStructRepresentation::Boolean => {
                            DebugTypeKind::Basic(DebugEncoding::Boolean)
                        }
                        mir::IntrinsicStructRepresentation::Unit => {
                            DebugTypeKind::Record(Vec::new())
                        }
                        mir::IntrinsicStructRepresentation::MaybeUninit { value } => {
                            DebugTypeKind::Record(vec![field("value", 0, self.source(value)?)])
                        }
                        mir::IntrinsicStructRepresentation::Ptr { pointee } => {
                            DebugTypeKind::Pointer(self.source(pointee)?)
                        }
                        mir::IntrinsicStructRepresentation::FunPtr { signature } => {
                            DebugTypeKind::Pointer(inline(self.signature(*signature)?))
                        }
                    },
                };
                (definition.name.clone(), kind)
            }
            mir::Type::Enum(id, _) => (self.module.enums[*id].name.clone(), self.enumeration(*id)?),
        };
        Ok(DebugType {
            name,
            size,
            align,
            kind,
        })
    }

    fn signature(&mut self, id: mir::FunctionTypeId) -> StorageResult<DebugType> {
        let signature = &self.module.function_types[id];
        let parameters = signature
            .parameters()
            .map(|ty| self.source(ty))
            .collect::<StorageResult<Vec<_>>>()?;
        let result = if signature.return_type == mir::Type::Unit {
            None
        } else {
            Some(self.source(&signature.return_type)?)
        };
        Ok(DebugType {
            name: String::new(),
            size: 0,
            align: 1,
            kind: DebugTypeKind::Function { parameters, result },
        })
    }

    fn address(&self) -> DebugTypeRef {
        let layout = self.context.pointer_layout(lir::PointerKind::Raw);
        basic(
            "uintptr_t",
            layout.size,
            layout.align,
            DebugEncoding::Address,
        )
    }

    fn pointer(&self, name: &str, pointee: DebugTypeRef) -> DebugType {
        let layout = self.context.pointer_layout(lir::PointerKind::Raw);
        DebugType {
            name: name.into(),
            size: layout.size,
            align: layout.align,
            kind: DebugTypeKind::Pointer(pointee),
        }
    }

    fn opaque_pointer(&self) -> DebugTypeKind {
        DebugTypeKind::Pointer(inline(DebugType {
            name: "void".into(),
            size: 0,
            align: 1,
            kind: DebugTypeKind::Record(Vec::new()),
        }))
    }
}

fn integer_encoding(kind: mir::IntegerKind) -> DebugEncoding {
    match kind.signedness() {
        mir::IntegerSignedness::Signed => DebugEncoding::Signed,
        mir::IntegerSignedness::Unsigned => DebugEncoding::Unsigned,
    }
}

fn integer_name(kind: mir::IntegerKind) -> String {
    let name = match kind.width() {
        mir::IntegerWidth::W8 => "Byte",
        mir::IntegerWidth::W16 => "Short",
        mir::IntegerWidth::W32 => "Int",
        mir::IntegerWidth::W64 => "Long",
    };
    match kind.signedness() {
        mir::IntegerSignedness::Signed => name.into(),
        mir::IntegerSignedness::Unsigned => format!("U{name}"),
    }
}

fn inline(ty: DebugType) -> DebugTypeRef {
    DebugTypeRef::Inline(Box::new(ty))
}

fn basic(name: &str, size: u64, align: u64, encoding: DebugEncoding) -> DebugTypeRef {
    inline(DebugType {
        name: name.into(),
        size,
        align,
        kind: DebugTypeKind::Basic(encoding),
    })
}

fn field(name: &str, offset: u64, ty: DebugTypeRef) -> DebugField {
    DebugField {
        name: name.into(),
        offset,
        ty,
        artificial: name.starts_with("__scoop_"),
    }
}
