//! Dependency value representations retain their defining Cone and field ids.

use super::*;

mod classes;
mod interfaces;
mod intrinsics;

impl Concretizer<'_> {
    pub(super) fn lower_imported_struct(
        &mut self,
        source: &export::ImportedStructType,
        substitution: &[concrete::TypeId],
    ) -> concrete::TypeId {
        let declaration = &source.declaration;
        let arguments = source
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect::<Vec<_>>();
        let identity = (declaration.owner(), arguments.clone());
        if let Some(id) = self.imported_structs.get(&identity) {
            return self.struct_type[id];
        }
        let export::NominalSourceShapeV1::Struct(shape) = declaration.interface.source_shape()
        else {
            unreachable!("an imported struct retains a struct declaration")
        };
        let id = concrete::StructId::from_raw(
            u32::try_from(self.structs.len())
                .expect("concrete struct ids fit in u32")
                .into(),
        );
        let ty = self.intern_type(concrete::TypeKind::Struct(id), false);
        let c_layout = match shape.c_layout_policy() {
            export::NominalCLayoutPolicyV1::Ordinary => None,
            export::NominalCLayoutPolicyV1::CLayout { contract } => Some(contract),
        };
        let c_abi = match declaration.c_abi {
            export::NativeBoundaryCAbiV1::SourceRepresentation => {
                concrete::StructCAbi::SourceRepresentation
            }
            export::NativeBoundaryCAbiV1::UInt64Field { field } => {
                concrete::StructCAbi::UInt64Field { field }
            }
            export::NativeBoundaryCAbiV1::NullablePointer { .. } => {
                unreachable!("nullable C projections belong to enum declarations")
            }
        };
        let allocated = self.structs.alloc(concrete::StructDef {
            origin: export::HirNominalIdentity::Source(declaration.identity.clone()),
            canonical_type: ty,
            name: declaration.name().to_owned(),
            owner: None,
            type_arguments: arguments,
            gc_free: false,
            representation: concrete::StructRepresentation::Declared {
                attributes: export::StructAttributes {
                    no_gc: false,
                    c_layout,
                    interior_mutable: shape.interior_mutable(),
                },
                c_abi,
                fields: Vec::new(),
            },
            // Dependency dispatch and callable definitions remain in the
            // provider. This arena contains the consumer's value representation.
            direct_interfaces: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: scoop_ast::Span::new(0, 0),
        });
        assert_eq!(allocated, id);
        self.imported_structs.insert(identity, id);
        self.struct_type.insert(id, ty);
        let fields: Vec<_> = source
            .fields
            .iter()
            .map(|field| concrete::DeclaredStructField {
                identity: field.identity,
                name: field.name.clone(),
                ty: self.lower_type(field.ty, substitution),
            })
            .collect();
        let concrete::StructRepresentation::Declared {
            fields: concrete_fields,
            ..
        } = &mut self.structs[id].representation
        else {
            unreachable!("an imported source struct keeps its declared representation")
        };
        let gc_free = fields.iter().all(|field| self.types[field.ty].gc_free);
        *concrete_fields = fields;
        self.structs[id].gc_free = gc_free;
        self.types[ty].gc_free = gc_free;
        self.structs[id].direct_interfaces = source
            .interfaces
            .iter()
            .map(|interface| self.lower_type(*interface, substitution))
            .collect();
        self.structs[id].interfaces =
            self.lower_imported_value_interfaces(&source.interfaces, substitution);
        ty
    }

    pub(super) fn lower_imported_enum(
        &mut self,
        source: &export::ImportedEnumType,
        substitution: &[concrete::TypeId],
    ) -> concrete::TypeId {
        let declaration = &source.declaration;
        let arguments = source
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect::<Vec<_>>();
        let identity = (declaration.owner(), arguments.clone());
        if let Some(id) = self.imported_enums.get(&identity) {
            return self.enum_type[id];
        }
        let id = concrete::EnumId::from_raw(
            u32::try_from(self.enums.len())
                .expect("concrete enum ids fit in u32")
                .into(),
        );
        let ty = self.intern_type(concrete::TypeKind::Enum(id), false);
        let allocated = self.enums.alloc(concrete::EnumDef {
            origin: export::HirNominalIdentity::Source(declaration.identity.clone()),
            canonical_type: ty,
            name: declaration.name().to_owned(),
            owner: None,
            type_arguments: arguments,
            gc_free: false,
            variants: Vec::new(),
            // Callable and dispatch definitions remain in the provider.
            direct_interfaces: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: scoop_ast::Span::new(0, 0),
        });
        assert_eq!(allocated, id);
        self.imported_enums.insert(identity, id);
        self.enum_type.insert(id, ty);
        let variants = source
            .variants
            .iter()
            .map(|variant| {
                let fields: Vec<_> = variant
                    .fields
                    .iter()
                    .map(|field| concrete::VariantField {
                        identity: field.identity,
                        name: field.name.clone(),
                        ty: self.lower_type(field.ty, substitution),
                    })
                    .collect();
                concrete::Variant {
                    identity: variant.identity,
                    name: variant.name.clone(),
                    gc_free: fields.iter().all(|field| self.types[field.ty].gc_free),
                    fields,
                }
            })
            .collect();
        self.enums[id].variants = variants;
        let gc_free = self.enums[id]
            .variants
            .iter()
            .all(|variant| variant.gc_free);
        self.enums[id].gc_free = gc_free;
        self.types[ty].gc_free = gc_free;
        self.enums[id].direct_interfaces = source
            .interfaces
            .iter()
            .map(|interface| self.lower_type(*interface, substitution))
            .collect();
        self.enums[id].interfaces =
            self.lower_imported_value_interfaces(&source.interfaces, substitution);
        ty
    }

    fn lower_imported_value_interfaces(
        &mut self,
        roots: &[export::TypeId],
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::TypeId> {
        let mut pending: Vec<_> = roots.iter().rev().copied().collect();
        let mut visited = Vec::new();
        let mut interfaces = Vec::new();
        while let Some(ty) = pending.pop() {
            if visited.contains(&ty) {
                continue;
            }
            visited.push(ty);
            let export::Type::ImportedInterface(source) = &self.source.types[ty] else {
                unreachable!("dependency value supertypes are resolved interfaces")
            };
            pending.extend(source.parents.iter().rev().copied());
            interfaces.push(self.lower_type(ty, substitution));
        }
        interfaces
    }
}
