use super::*;
use crate::concretize::nominals::ResolvedField;

pub(super) struct ResolvedClassDefinition<'a> {
    pub origin: export::HirNominalIdentity,
    pub name: String,
    pub owner: Option<concrete::NominalOwner>,
    pub modifier: export::ClassModifier,
    pub representation: ResolvedClassRepresentation<'a>,
    pub interfaces: &'a [export::TypeId],
    pub interface_implementations: &'a [export::InterfaceImplementation],
    pub element_encoding: Option<&'a export::ElementEncoding>,
    pub methods: &'a [export::FunctionId],
    pub virtual_methods: &'a [export::ImportedVirtualMethod],
    pub constructors: &'a [export::ClassConstructorId],
    pub span: scoop_ast::Span,
}

pub(super) enum ResolvedClassRepresentation<'a> {
    Declared {
        fields: Vec<ResolvedField<'a>>,
        base_class: Option<export::TypeId>,
    },
    Intrinsic {
        declaration: export::IntrinsicTypeKind,
        application: concrete::IntrinsicTypeRepresentation,
    },
}

impl<'input> Concretizer<'input> {
    pub(super) fn source_class_definition(
        &self,
        id: export::ClassId,
        application: ConcreteApplicationRepresentation,
    ) -> ResolvedClassDefinition<'input> {
        let declaration = &self.source.classes[id];
        let representation = match (&declaration.representation, application) {
            (
                export::ClassRepresentation::Declared,
                ConcreteApplicationRepresentation::Declared,
            ) => {
                let fields = declaration
                    .fields
                    .iter()
                    .map(|field| {
                        let value = self.source.class_field_definition(*field);
                        ResolvedField {
                            identity: self.source.field_identities[*field].id(),
                            name: &value.name,
                            ty: value.ty,
                        }
                    })
                    .collect();
                ResolvedClassRepresentation::Declared {
                    fields,
                    base_class: declaration.base_class,
                }
            }
            (
                export::ClassRepresentation::Intrinsic(declaration),
                ConcreteApplicationRepresentation::Intrinsic(application),
            ) => ResolvedClassRepresentation::Intrinsic {
                declaration: *declaration,
                application,
            },
            _ => unreachable!("declaration and application representations agree"),
        };
        ResolvedClassDefinition {
            origin: self.source.nominal_identities[id].clone(),
            name: self.source_nominal_name(&declaration.name, declaration.owner),
            owner: self.lower_nominal_owner(declaration.owner),
            modifier: declaration.modifier,
            representation,
            interfaces: &declaration.interfaces,
            interface_implementations: &declaration.interface_implementations,
            element_encoding: declaration.element_encoding.as_ref(),
            methods: &declaration.methods,
            virtual_methods: &[],
            constructors: if declaration.type_params.is_empty() {
                &declaration.constructors
            } else {
                &[]
            },
            span: declaration.span,
        }
    }
}

impl<'a> ResolvedClassDefinition<'a> {
    pub(super) fn from_dependency(
        source: &'a export::LoadedClassDefinition,
        application: ConcreteApplicationRepresentation,
    ) -> Self {
        let declaration = &source.declaration;
        let definition = &source.definition;
        let representation = match (&definition.representation, application) {
            (
                export::ClassRepresentation::Declared,
                ConcreteApplicationRepresentation::Declared,
            ) => ResolvedClassRepresentation::Declared {
                fields: definition
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| ResolvedField {
                        identity: source.field_identity(index),
                        name: &field.name,
                        ty: field.ty,
                    })
                    .collect(),
                base_class: definition.base_class,
            },
            (
                export::ClassRepresentation::Intrinsic(declaration),
                ConcreteApplicationRepresentation::Intrinsic(application),
            ) => ResolvedClassRepresentation::Intrinsic {
                declaration: *declaration,
                application,
            },
            _ => unreachable!("declaration and application representations agree"),
        };
        let span = declaration.origin.origin().span();
        Self {
            origin: export::HirNominalIdentity::Source(declaration.identity.clone()),
            name: declaration.name().to_owned(),
            owner: None,
            modifier: definition.modifier,
            representation,
            interfaces: &definition.interfaces,
            interface_implementations: &definition.interface_implementations,
            element_encoding: definition.element_encoding.as_ref(),
            methods: &[],
            virtual_methods: &source.virtual_methods,
            constructors: &[],
            span: scoop_ast::Span::new(
                u32::try_from(span.start_byte()).expect("decoded source spans fit HIR"),
                u32::try_from(span.end_byte()).expect("decoded source spans fit HIR"),
            ),
        }
    }
}
