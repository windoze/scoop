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
                        let value = &self.source.class_fields[*field];
                        ResolvedField {
                            identity: self.source.field_identities[*field].id(),
                            name: &self.source.properties[value.property].name,
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
        source: &'a export::ImportedClassType,
        arguments: &[concrete::TypeId],
    ) -> Self {
        let declaration = &source.declaration;
        let modifier = match declaration.interface.declaration_details().modality() {
            export::NominalInheritanceModalityV1::Final => export::ClassModifier::Final,
            export::NominalInheritanceModalityV1::Open => export::ClassModifier::Open,
            export::NominalInheritanceModalityV1::Abstract => export::ClassModifier::Abstract,
            export::NominalInheritanceModalityV1::Interface => {
                unreachable!("a class retains class modality")
            }
        };
        let representation = match declaration.interface.source_shape() {
            export::NominalSourceShapeV1::Intrinsic(source) => {
                let [element] = arguments else {
                    unreachable!("intrinsic array arity was checked in HIR")
                };
                let application = match source.family() {
                    export::IntrinsicTypeKind::Array => {
                        concrete::IntrinsicTypeRepresentation::Array { element: *element }
                    }
                    export::IntrinsicTypeKind::MutableArray => {
                        concrete::IntrinsicTypeRepresentation::MutableArray { element: *element }
                    }
                    _ => unreachable!(
                        "resolved intrinsic class applications retain their array family"
                    ),
                };
                ResolvedClassRepresentation::Intrinsic {
                    declaration: source.family(),
                    application,
                }
            }
            _ => ResolvedClassRepresentation::Declared {
                fields: source
                    .fields
                    .iter()
                    .map(|field| ResolvedField {
                        identity: field.identity,
                        name: &field.name,
                        ty: field.ty,
                    })
                    .collect(),
                base_class: source.base_class,
            },
        };
        Self {
            origin: export::HirNominalIdentity::Source(declaration.identity.clone()),
            name: declaration.name().to_owned(),
            owner: None,
            modifier,
            representation,
            interfaces: &source.interfaces,
            interface_implementations: &source.interface_implementations,
            methods: &[],
            virtual_methods: &source.virtual_methods,
            constructors: &[],
            span: scoop_ast::Span::new(0, 0),
        }
    }
}
