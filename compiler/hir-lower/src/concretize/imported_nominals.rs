//! Dependency value representations retain their defining Cone and field ids.

use super::*;

mod classes;
mod interfaces;

impl Concretizer<'_> {
    pub(super) fn lower_imported_struct(
        &mut self,
        source: &export::ImportedStructType,
    ) -> concrete::TypeId {
        let declaration = &source.declaration;
        let identity = declaration.identity.id();
        if let Some(id) = self.imported_structs.get(&identity) {
            return self.struct_type[id];
        }
        let export::NominalSourceShapeV1::Struct(shape) = declaration.interface.source_shape()
        else {
            unreachable!("an imported struct retains a struct declaration")
        };
        let fields = source
            .fields
            .iter()
            .map(|field| concrete::DeclaredStructField {
                identity: field.identity,
                name: field.name.clone(),
                ty: self.lower_type(field.ty, &[]),
            })
            .collect();
        let id = concrete::StructId::from_raw(
            u32::try_from(self.structs.len())
                .expect("concrete struct ids fit in u32")
                .into(),
        );
        let ty = self.intern_type(concrete::TypeKind::Struct(id), source.gc_free);
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
            origin: export::HirNominalIdentity::Source(export::HirSourceNominalIdentity::Concrete(
                declaration.identity.clone(),
            )),
            canonical_type: ty,
            name: declaration.name().to_owned(),
            owner: None,
            type_arguments: Vec::new(),
            gc_free: source.gc_free,
            representation: concrete::StructRepresentation::Declared {
                attributes: export::StructAttributes {
                    no_gc: false,
                    c_layout,
                    interior_mutable: shape.interior_mutable(),
                },
                c_abi,
                fields,
            },
            // Dependency dispatch and callable definitions remain in the
            // provider. This arena contains the consumer's value representation.
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: scoop_ast::Span::new(0, 0),
        });
        assert_eq!(allocated, id);
        self.imported_structs.insert(identity, id);
        self.struct_type.insert(id, ty);
        ty
    }

    pub(super) fn lower_imported_enum(
        &mut self,
        source: &export::ImportedEnumType,
    ) -> concrete::TypeId {
        let declaration = &source.declaration;
        let identity = declaration.identity.id();
        if let Some(id) = self.imported_enums.get(&identity) {
            return self.enum_type[id];
        }
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
                        ty: self.lower_type(field.ty, &[]),
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
        let id = concrete::EnumId::from_raw(
            u32::try_from(self.enums.len())
                .expect("concrete enum ids fit in u32")
                .into(),
        );
        let ty = self.intern_type(concrete::TypeKind::Enum(id), source.gc_free);
        let allocated = self.enums.alloc(concrete::EnumDef {
            origin: export::HirNominalIdentity::Source(export::HirSourceNominalIdentity::Concrete(
                declaration.identity.clone(),
            )),
            canonical_type: ty,
            name: declaration.name().to_owned(),
            owner: None,
            type_arguments: Vec::new(),
            gc_free: source.gc_free,
            variants,
            // Callable and dispatch definitions remain in the provider.
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: scoop_ast::Span::new(0, 0),
        });
        assert_eq!(allocated, id);
        self.imported_enums.insert(identity, id);
        self.enum_type.insert(id, ty);
        ty
    }
}
