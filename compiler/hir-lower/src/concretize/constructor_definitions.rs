//! Storage adaptation for the shared constructor lowering algorithms.

use super::*;

pub(super) struct ConstructorBodyView<'a, K> {
    pub(super) parameters: &'a [export::ConstructorParameter],
    pub(super) kind: &'a K,
    pub(super) safety: concrete::Safety,
    pub(super) origin: export::DefinitionOrigin,
    pub(super) span: scoop_ast::Span,
    pub(super) evaluation_context: export::SourceContextId,
    pub(super) discriminator: u32,
}

impl<'a> Concretizer<'a> {
    pub(super) fn class_constructor_definition(
        &self,
        definition: export::ClassConstructorDefinition,
    ) -> ConstructorBodyView<'a, export::ClassConstructorKind> {
        match definition {
            export::ClassConstructorDefinition::Local(id) => {
                let source = &self.source.class_constructors[id];
                ConstructorBodyView {
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
                ConstructorBodyView {
                    parameters: &source.parameters,
                    kind,
                    safety: constructor_safety(source.effects),
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
                    parameters: &source.parameters,
                    kind,
                    safety: constructor_safety(source.effects),
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
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::ConstructorParameter> {
        parameters
            .iter()
            .map(|parameter| concrete::ConstructorParameter {
                id: concrete::ConstructorParamId::from_raw(parameter.id.into_raw()),
                binding: concrete::BindingId::from_raw(parameter.binding.into_raw()),
                definition: parameter.definition,
                name: parameter.name.clone(),
                ty: self.lower_type(parameter.ty, substitution),
            })
            .collect()
    }
}

fn constructor_safety(effects: export::CallableSourceEffectsV1) -> concrete::Safety {
    match effects.safety() {
        export::CallableSafetyV1::Safe => concrete::Safety::Safe,
        export::CallableSafetyV1::Unsafe => concrete::Safety::Unsafe,
    }
}
