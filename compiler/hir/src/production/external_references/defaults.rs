use scoop_identity::{CallableTemplateOrigin, PropertyOwner};
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    ExternalHirReferenceProductionError, ExternalHirReferenceProductionInput,
    accumulator::ExternalReferenceAccumulator,
};
use crate::{
    DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1, DefaultCallableDeclarationV1,
    DefaultClassConstructorIdV1, DefaultConstructorRefV1, DefaultFieldRefV1,
    ExportDefaultCallableTargetV1, ExternalHirReferenceRoleV1,
    ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1,
};

pub(super) fn collect<A, E>(
    input: ExternalHirReferenceProductionInput<'_>,
    accumulator: &mut ExternalReferenceAccumulator<'_, A>,
    meter: &mut BudgetMeter,
) -> Result<(), ExternalHirReferenceProductionError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    for (template_index, template) in (0_u64..).zip(input.default_templates.records()) {
        let references = template.references();
        let path = WirePath::root().field(7).index(template_index).field(11);
        for reference in references.callables() {
            if let Some(target) = callable_target(reference.target()) {
                accumulator.observe(target, ExternalHirReferenceRoleV1::DefaultDependency)?;
            }
        }
        for reference in references.constructors() {
            accumulator.observe(
                constructor_target(reference.target()),
                ExternalHirReferenceRoleV1::DefaultDependency,
            )?;
        }
        for (reference_index, reference) in (0_u64..).zip(references.types()) {
            accumulator.observe_signature(
                reference.target(),
                ExternalHirReferenceRoleV1::DefaultDependency,
                meter,
                &path.clone().field(3).index(reference_index).field(1),
            )?;
        }
        for reference in references.globals() {
            accumulator.observe(
                ExternalHirTargetV1::Property(PropertyOwner::Property(*reference.target())),
                ExternalHirReferenceRoleV1::DefaultDependency,
            )?;
        }
        for reference in references.singleton_values() {
            accumulator.observe(
                ExternalHirTargetV1::ObjectValue(*reference.target()),
                ExternalHirReferenceRoleV1::DefaultDependency,
            )?;
        }
        for reference in references.fields() {
            if let Some(target) = field_target(reference.target()) {
                accumulator.observe(target, ExternalHirReferenceRoleV1::DefaultDependency)?;
            }
        }
    }
    Ok(())
}

fn callable_target(target: &ExportDefaultCallableTargetV1) -> Option<ExternalHirTargetV1> {
    match target {
        ExportDefaultCallableTargetV1::Callable(callable) => {
            Some(callable_declaration_target(callable.declaration()))
        }
        ExportDefaultCallableTargetV1::Bound(callable) => Some(bound_callable_target(callable)),
        ExportDefaultCallableTargetV1::DerivedEquality { .. } => None,
        ExportDefaultCallableTargetV1::LocalFunction { declaration } => {
            Some(ExternalHirTargetV1::Callable(*declaration))
        }
        ExportDefaultCallableTargetV1::Lambda { body }
        | ExportDefaultCallableTargetV1::AnonymousFunction { body } => {
            Some(ExternalHirTargetV1::GeneratedCallable(*body))
        }
        ExportDefaultCallableTargetV1::CallableReference { invoke } => {
            Some(ExternalHirTargetV1::GeneratedCallable(*invoke))
        }
        ExportDefaultCallableTargetV1::FunctionAddress { declaration } => {
            Some(callable_declaration_target(*declaration))
        }
    }
}

fn bound_callable_target(callable: &DefaultBoundCallableRefV1) -> ExternalHirTargetV1 {
    match callable.source() {
        DefaultBoundCallableSourceV1::Class { callable, .. } => {
            callable_declaration_target(callable.declaration())
        }
        DefaultBoundCallableSourceV1::Interface { member, .. } => {
            ExternalHirTargetV1::Callable(*member)
        }
    }
}

const fn callable_declaration_target(
    declaration: DefaultCallableDeclarationV1,
) -> ExternalHirTargetV1 {
    match declaration {
        DefaultCallableDeclarationV1::Function(declaration) => {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(declaration))
        }
        DefaultCallableDeclarationV1::GenericFunction(declaration) => {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::GenericFunction(declaration))
        }
        DefaultCallableDeclarationV1::PropertyAccessor(declaration) => {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Accessor(declaration))
        }
        DefaultCallableDeclarationV1::Generated(declaration) => {
            ExternalHirTargetV1::GeneratedCallable(declaration)
        }
    }
}

const fn constructor_target(constructor: &DefaultConstructorRefV1) -> ExternalHirTargetV1 {
    match constructor {
        DefaultConstructorRefV1::Struct { declaration, .. } => {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(*declaration))
        }
        DefaultConstructorRefV1::Class { declaration, .. } => match declaration {
            DefaultClassConstructorIdV1::Source(declaration) => {
                ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(*declaration))
            }
            DefaultClassConstructorIdV1::Generated(declaration) => {
                ExternalHirTargetV1::GeneratedCallable(*declaration)
            }
        },
        DefaultConstructorRefV1::Variant { declaration, .. } => {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::VariantConstructor(*declaration))
        }
    }
}

const fn field_target(field: &DefaultFieldRefV1) -> Option<ExternalHirTargetV1> {
    match field {
        DefaultFieldRefV1::Struct { declaration, .. }
        | DefaultFieldRefV1::Class { declaration, .. } => {
            Some(ExternalHirTargetV1::Field(*declaration))
        }
        DefaultFieldRefV1::Tuple { .. } => None,
    }
}
