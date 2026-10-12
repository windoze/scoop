//! Storage adaptation for the shared constructor lowering algorithms.

use super::*;

pub(super) struct ConstructorBodyView<'a, K> {
    pub(super) enclosing: Option<(
        &'a export::ConstructorContext,
        export::InitializingClassFieldRef,
    )>,
    pub(super) parameters: &'a [export::ConstructorParameter],
    pub(super) kind: &'a K,
    pub(super) safety: concrete::Safety,
    pub(super) origin: export::DefinitionOrigin,
    pub(super) span: scoop_ast::Span,
    pub(super) evaluation_context: export::SourceContextId,
    pub(super) discriminator: u32,
}

impl<'a> Concretizer<'a> {
    pub(super) fn class_constructor_origin(
        &self,
        definition: export::ClassConstructorDefinition,
    ) -> (
        export::DefaultClassConstructorIdV1,
        scoop_identity::PersistentConstructorId,
    ) {
        use export::DefaultClassConstructorIdV1 as Origin;
        match definition {
            export::ClassConstructorDefinition::Local(id) => {
                match &self.source.constructor_identities[id] {
                    export::HirClassConstructorIdentity::Source(record) => {
                        (Origin::Source(record.id()), record.id())
                    }
                    export::HirClassConstructorIdentity::ZeroArgumentAdapter { source, record } => {
                        let source = self.source.constructor_identities[*source]
                            .source_record()
                            .expect("a zero-argument adapter retains its source constructor")
                            .id();
                        (Origin::Generated(record.id()), source)
                    }
                }
            }
            export::ClassConstructorDefinition::Template(id) => {
                let source = self.source.imported_constructor_templates[id].declaration;
                (Origin::Source(source), source)
            }
        }
    }

    pub(super) fn struct_constructor_origin(
        &self,
        definition: export::StructConstructorDefinition,
    ) -> scoop_identity::PersistentConstructorId {
        match definition {
            export::StructConstructorDefinition::Local(id) => {
                self.source.constructor_identities[id].id()
            }
            export::StructConstructorDefinition::Template(id) => {
                self.source.imported_constructor_templates[id].declaration
            }
        }
    }

    pub(super) fn class_constructor_definition(
        &self,
        definition: export::ClassConstructorDefinition,
    ) -> ConstructorBodyView<'a, export::ClassConstructorKind> {
        match definition {
            export::ClassConstructorDefinition::Local(id) => {
                let source = &self.source.class_constructors[id];
                let enclosing = source.context_parameter.as_ref().map(|parameter| {
                    let owner = &self.source.classes[source.owner];
                    let field = owner
                        .fields
                        .iter()
                        .copied()
                        .find(|field| {
                            self.source.class_fields[*field].source == parameter.field_source()
                        })
                        .expect("an inner constructor has its enclosing field");
                    (
                        parameter,
                        export::InitializingClassFieldRef {
                            owner: self.source.class_applications[owner.self_application]
                                .canonical_type,
                            field: self.source.field_identities[field].id(),
                        },
                    )
                });
                ConstructorBodyView {
                    enclosing,
                    parameters: &source.parameters,
                    kind: &source.kind,
                    safety: source.safety,
                    origin: source.origin,
                    span: source.span,
                    evaluation_context: source.evaluation_context,
                    discriminator: id.into_raw().into_u32(),
                }
            }
            export::ClassConstructorDefinition::Template(id) => {
                let source = &self.source.imported_constructor_templates[id];
                let export::ConstructorKind::Class(kind) = &source.kind else {
                    unreachable!("a class constructor retains its class definition")
                };
                let enclosing = source.context_parameter.as_ref().map(|parameter| {
                    let export::Type::Class(application) = self.source.types[source.owner] else {
                        unreachable!("an imported class constructor has a class owner")
                    };
                    let template = self.source.class_applications[application].template;
                    let class = &self.source.loaded_class_definitions[&template];
                    let index = class
                        .declaration
                        .field_sources
                        .iter()
                        .position(|field| {
                            field.storage
                                == match parameter {
                                    export::ConstructorContext::EnclosingInstance(_) => {
                                        export::NominalFieldStorage::EnclosingInstance
                                    }
                                    export::ConstructorContext::CapturedEnvironment(_) => {
                                        export::NominalFieldStorage::CapturedEnvironment
                                    }
                                }
                        })
                        .expect("an imported inner constructor has its enclosing field");
                    (
                        parameter,
                        export::InitializingClassFieldRef {
                            owner: source.owner,
                            field: class.field_identity(index),
                        },
                    )
                });
                ConstructorBodyView {
                    enclosing,
                    parameters: &source.parameters,
                    kind,
                    safety: constructor_safety(&source.effects),
                    origin: source.origin,
                    span: source.origin.span,
                    evaluation_context: source.evaluation_context,
                    discriminator: id.into_raw().into_u32(),
                }
            }
        }
    }

    pub(super) fn struct_constructor_definition(
        &self,
        definition: export::StructConstructorDefinition,
    ) -> ConstructorBodyView<'a, export::StructConstructorKind> {
        match definition {
            export::StructConstructorDefinition::Local(id) => {
                let source = &self.source.struct_constructors[id];
                ConstructorBodyView {
                    enclosing: None,
                    parameters: &source.parameters,
                    kind: &source.kind,
                    safety: source.safety,
                    origin: source.origin,
                    span: source.span,
                    evaluation_context: source.origin.context,
                    discriminator: id.into_raw().into_u32(),
                }
            }
            export::StructConstructorDefinition::Template(id) => {
                let source = &self.source.imported_constructor_templates[id];
                let export::ConstructorKind::Struct(kind) = &source.kind else {
                    unreachable!("a struct constructor retains its struct definition")
                };
                ConstructorBodyView {
                    enclosing: None,
                    parameters: &source.parameters,
                    kind,
                    safety: constructor_safety(&source.effects),
                    origin: source.origin,
                    span: source.origin.span,
                    evaluation_context: source.evaluation_context,
                    discriminator: id.into_raw().into_u32(),
                }
            }
        }
    }

    pub(super) fn lower_constructor_parameters(
        &mut self,
        parameters: &[export::ConstructorParameter],
        enclosing: Option<&export::ConstructorContext>,
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::ConstructorParameter> {
        enclosing
            .into_iter()
            .map(|context| (context.selector(), context.parameter()))
            .chain(parameters.iter().enumerate().map(|(index, parameter)| {
                (
                    scoop_identity::LocalValueSelector::Parameter {
                        declaration_index: index as u32,
                    },
                    parameter,
                )
            }))
            .map(|(selector, parameter)| concrete::ConstructorParameter {
                selector,
                id: concrete::ConstructorParamId::from_raw(parameter.id.into_raw()),
                binding: concrete::BindingId::from_raw(parameter.binding.into_raw()),
                definition: parameter.definition,
                name: parameter.name.clone(),
                ty: self.lower_type(parameter.ty, substitution),
            })
            .collect()
    }
}

fn constructor_safety(effects: &export::CallableSourceEffectsV1) -> concrete::Safety {
    match effects.safety() {
        export::CallableSafetyV1::Safe => concrete::Safety::Safe,
        export::CallableSafetyV1::Unsafe => concrete::Safety::Unsafe,
    }
}
