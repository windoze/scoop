use super::*;
use crate::concretize::nominals::ResolvedField;

pub(super) struct ResolvedStructDefinition<'a> {
    pub origin: export::HirNominalIdentity,
    pub name: String,
    pub owner: Option<concrete::NominalOwner>,
    pub representation: ResolvedStructRepresentation<'a>,
    pub interfaces: &'a [export::TypeId],
    pub interface_implementations: &'a [export::InterfaceImplementation],
    pub methods: &'a [export::FunctionId],
    pub constructors: &'a [export::StructConstructorId],
    pub span: scoop_ast::Span,
}

pub(super) enum ResolvedStructRepresentation<'a> {
    Declared {
        attributes: export::StructAttributes,
        c_abi: concrete::StructCAbi,
        fields: Vec<ResolvedField<'a>>,
    },
    Intrinsic {
        declaration: export::IntrinsicTypeKind,
        application: concrete::IntrinsicTypeRepresentation,
    },
}

impl<'input> Concretizer<'input> {
    pub(super) fn source_struct_definition(
        &self,
        id: export::StructId,
        application: ConcreteApplicationRepresentation,
    ) -> ResolvedStructDefinition<'input> {
        let declaration = &self.source.structs[id];
        let representation = match (&declaration.representation, application) {
            (
                export::StructRepresentation::Declared(fields),
                ConcreteApplicationRepresentation::Declared,
            ) => {
                let fields = fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        let reference = export::StructFieldRef::checked(
                            &self.source.structs,
                            id,
                            u32::try_from(index).expect("source field indices fit u32"),
                        )
                        .expect("a declared struct retains each field");
                        ResolvedField {
                            identity: self.source.field_identities[reference].id(),
                            name: &field.name,
                            ty: field.ty,
                        }
                    })
                    .collect();
                ResolvedStructRepresentation::Declared {
                    attributes: declaration.attributes,
                    c_abi: self.struct_c_abi(id),
                    fields,
                }
            }
            (
                export::StructRepresentation::Intrinsic(declaration),
                ConcreteApplicationRepresentation::Intrinsic(application),
            ) => ResolvedStructRepresentation::Intrinsic {
                declaration: *declaration,
                application,
            },
            _ => unreachable!("declaration and application representations agree"),
        };
        ResolvedStructDefinition {
            origin: self.source.nominal_identities[id].clone(),
            name: self.source_nominal_name(&declaration.name, declaration.owner),
            owner: self.lower_nominal_owner(declaration.owner),
            representation,
            interfaces: &declaration.interfaces,
            interface_implementations: &declaration.interface_implementations,
            methods: &declaration.methods,
            constructors: if declaration.type_params.is_empty() {
                &declaration.constructors
            } else {
                &[]
            },
            span: declaration.span,
        }
    }
}

impl<'a> ResolvedStructDefinition<'a> {
    pub(super) fn from_dependency(
        source: &'a export::LoadedStructDefinition,
        application: ConcreteApplicationRepresentation,
    ) -> Self {
        if let ConcreteApplicationRepresentation::Intrinsic(application) = application {
            let export::StructRepresentation::Intrinsic(declaration) =
                source.definition.representation
            else {
                unreachable!("declaration and application representations agree")
            };
            return Self::dependency(
                &source.declaration,
                ResolvedStructRepresentation::Intrinsic {
                    declaration,
                    application,
                },
                &source.definition.interfaces,
                &source.definition.interface_implementations,
            );
        }
        let c_abi = match source.declaration.c_abi {
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
        let fields = source
            .definition
            .semantic_fields()
            .iter()
            .enumerate()
            .map(|(index, field)| ResolvedField {
                identity: source.field_identity(index),
                name: &field.name,
                ty: field.ty,
            })
            .collect();
        Self::dependency(
            &source.declaration,
            ResolvedStructRepresentation::Declared {
                attributes: source.definition.attributes,
                c_abi,
                fields,
            },
            &source.definition.interfaces,
            &source.definition.interface_implementations,
        )
    }

    pub(super) fn from_intrinsic(
        source: &'a export::ImportedIntrinsicType,
        declaration: export::IntrinsicTypeKind,
        application: concrete::IntrinsicTypeRepresentation,
    ) -> Self {
        Self::dependency(
            &source.declaration,
            ResolvedStructRepresentation::Intrinsic {
                declaration,
                application,
            },
            &source.interfaces,
            // Ordinary primitive implementations and boxing adapters remain external.
            &[],
        )
    }

    fn dependency(
        declaration: &export::ImportedNominalDeclaration,
        representation: ResolvedStructRepresentation<'a>,
        interfaces: &'a [export::TypeId],
        interface_implementations: &'a [export::InterfaceImplementation],
    ) -> Self {
        Self {
            origin: export::HirNominalIdentity::Source(declaration.identity.clone()),
            name: declaration.name().to_owned(),
            owner: None,
            representation,
            interfaces,
            interface_implementations,
            methods: &[],
            constructors: &[],
            span: scoop_ast::Span::new(0, 0),
        }
    }
}
