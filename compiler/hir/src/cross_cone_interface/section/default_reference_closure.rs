use std::fmt;

use scoop_identity::{CallableTemplateOrigin, PropertyOwner, SignatureTypeKey};
use scoop_wire::{WireError, WirePath};

use super::{CrossConeHirInterfaceSectionV1, signature_nominal_walk::SignatureNominalWalker};
use crate::{
    CanonicalExternalHirReferencesV1, DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1,
    DefaultCallableDeclarationV1, DefaultClassConstructorIdV1, DefaultConstructorRefV1,
    DefaultFieldRefV1, ExportDefaultCallableTargetV1, ExternalHirReferenceRoleV1,
    ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1,
};

impl CrossConeHirInterfaceSectionV1 {
    pub(super) fn validate_generic_body_reference_closure<A, E>(
        &self,
        authority: &mut A,
        path: &WirePath,
    ) -> Result<(), ExternalHirDefaultClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut validator = DefaultReferenceClosureValidator::new(
            self.external_references(),
            authority,
            &path.clone().field(10),
        )?;
        validator.role = ExternalHirReferenceRoleV1::TemplateDependency;
        for (body_index, body) in self.generic_callable_bodies().records().iter().enumerate() {
            body.visit_declaration_targets(
                &mut |target| {
                    validator.observe(
                        target,
                        ExternalHirDefaultUseSiteV1::GenericBody { body_index },
                    )
                },
                &path.clone().field(11).index(body_index as u64),
            )?;
        }
        for (body_index, initialization) in
            self.generic_initializations().records().iter().enumerate()
        {
            initialization.visit_declaration_targets(
                &mut |target| {
                    validator.observe(
                        target,
                        ExternalHirDefaultUseSiteV1::GenericInitialization { body_index },
                    )
                },
                &path.clone().field(12).index(body_index as u64),
            )?;
        }
        for (body_index, delegate) in self.generic_delegates().records().iter().enumerate() {
            delegate.visit_declaration_targets(
                &mut |target| {
                    validator.observe(
                        target,
                        ExternalHirDefaultUseSiteV1::GenericDelegate { body_index },
                    )
                },
                &path.clone().field(13).index(body_index as u64),
            )?;
        }
        validator.finish()
    }

    /// Validates that `DefaultDependency` is exactly the set of foreign
    /// declaration and nominal targets in field 7 reference sets.
    pub fn validate_default_reference_closure<A, E>(
        &self,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), ExternalHirDefaultClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let references_path = path.clone().field(10);
        let mut validator = DefaultReferenceClosureValidator::new(
            self.external_references(),
            authority,
            &references_path,
        )?;

        for (template_wire_index, (template_index, template)) in
            (0_u64..).zip(self.default_templates().records().iter().enumerate())
        {
            let set = template.references();
            let set_path = path.clone().field(7).index(template_wire_index).field(11);

            for (reference_index, reference) in set.callables().iter().enumerate() {
                if let Some(target) = callable_target(reference.target()) {
                    validator.observe(
                        target,
                        ExternalHirDefaultUseSiteV1::Callable {
                            template_index,
                            reference_index,
                        },
                    )?;
                }
            }

            for (reference_index, reference) in set.constructors().iter().enumerate() {
                validator.observe(
                    constructor_target(reference.target()),
                    ExternalHirDefaultUseSiteV1::Constructor {
                        template_index,
                        reference_index,
                    },
                )?;
            }

            for (wire_index, (reference_index, reference)) in
                (0_u64..).zip(set.types().iter().enumerate())
            {
                validator.visit_signature(
                    reference.target(),
                    ExternalHirDefaultUseSiteV1::Type {
                        template_index,
                        reference_index,
                    },
                    &set_path.clone().field(3).index(wire_index).field(1),
                )?;
            }

            for (reference_index, reference) in set.globals().iter().enumerate() {
                validator.observe(
                    ExternalHirTargetV1::Property(PropertyOwner::Property(*reference.target())),
                    ExternalHirDefaultUseSiteV1::Global {
                        template_index,
                        reference_index,
                    },
                )?;
            }

            for (reference_index, reference) in set.singleton_values().iter().enumerate() {
                validator.observe(
                    ExternalHirTargetV1::ObjectValue(*reference.target()),
                    ExternalHirDefaultUseSiteV1::Singleton {
                        template_index,
                        reference_index,
                    },
                )?;
            }

            for (reference_index, reference) in set.fields().iter().enumerate() {
                if let Some(target) = field_target(reference.target()) {
                    validator.observe(
                        target,
                        ExternalHirDefaultUseSiteV1::Field {
                            template_index,
                            reference_index,
                        },
                    )?;
                }
            }
        }

        validator.finish()
    }
}

pub(super) struct DefaultReferenceClosureValidator<'references, 'validation, A> {
    pub(super) role: ExternalHirReferenceRoleV1,
    references: &'references CanonicalExternalHirReferencesV1,
    seen: Vec<bool>,
    current: scoop_identity::ConeIdentity,
    authority: &'validation mut A,
}

impl<'references, 'validation, A> DefaultReferenceClosureValidator<'references, 'validation, A> {
    pub(super) fn new<E>(
        references: &'references CanonicalExternalHirReferencesV1,
        authority: &'validation mut A,

        path: &WirePath,
    ) -> Result<Self, ExternalHirDefaultClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut seen = Vec::new();
        scoop_wire::allocation::try_reserve(&mut seen, references.records().len(), path)
            .map_err(ExternalHirDefaultClosureValidationError::Resource)?;
        seen.resize(references.records().len(), false);
        Ok(Self {
            role: ExternalHirReferenceRoleV1::DefaultDependency,
            references,
            seen,
            current: authority.current_cone(),
            authority,
        })
    }

    fn visit_signature<E>(
        &mut self,
        signature: &SignatureTypeKey,
        site: ExternalHirDefaultUseSiteV1,
        path: &WirePath,
    ) -> Result<(), ExternalHirDefaultClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut walker = SignatureNominalWalker::new(signature, path)
            .map_err(ExternalHirDefaultClosureValidationError::Resource)?;
        while let Some(declaration) = walker
            .next(path)
            .map_err(ExternalHirDefaultClosureValidationError::Resource)?
        {
            self.observe(ExternalHirTargetV1::from(declaration), site)?;
        }
        Ok(())
    }

    pub(super) fn observe<E>(
        &mut self,
        target: ExternalHirTargetV1,
        site: ExternalHirDefaultUseSiteV1,
    ) -> Result<(), ExternalHirDefaultClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let expected = self
            .authority
            .external_hir_target_origin(target)
            .map_err(
                |error| ExternalHirDefaultClosureValidationError::TargetOrigin {
                    site,
                    target,
                    error,
                },
            )?;
        if expected == self.current {
            return Ok(());
        }

        let record_index = self
            .references
            .find_index(target)
            .ok_or(ExternalHirDefaultClosureValidationError::MissingReference { site, target })?;
        let record = &self.references.records()[record_index];
        if record.origin() != expected {
            return Err(ExternalHirDefaultClosureValidationError::OriginMismatch(
                Box::new(ExternalHirDefaultOriginMismatch {
                    site,
                    record_index,
                    target,
                    expected,
                    actual: record.origin(),
                }),
            ));
        }
        if !record.roles().contains(self.role) {
            return Err(ExternalHirDefaultClosureValidationError::MissingRole {
                site,
                record_index,
                target,
            });
        }
        self.seen[record_index] = true;
        Ok(())
    }

    pub(super) fn finish<E>(self) -> Result<(), ExternalHirDefaultClosureValidationError<E>> {
        for (record_index, record) in self.references.records().iter().enumerate() {
            if record.roles().contains(self.role) && !self.seen[record_index] {
                return Err(ExternalHirDefaultClosureValidationError::ExtraRole {
                    record_index,
                    target: record.target(),
                });
            }
        }
        Ok(())
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalHirDefaultUseSiteV1 {
    Annotation {
        reference_index: usize,
    },
    GenericDelegate {
        body_index: usize,
    },
    GenericInitialization {
        body_index: usize,
    },
    GenericBody {
        body_index: usize,
    },
    Callable {
        template_index: usize,
        reference_index: usize,
    },
    Constructor {
        template_index: usize,
        reference_index: usize,
    },
    Type {
        template_index: usize,
        reference_index: usize,
    },
    Global {
        template_index: usize,
        reference_index: usize,
    },
    Singleton {
        template_index: usize,
        reference_index: usize,
    },
    Field {
        template_index: usize,
        reference_index: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExternalHirDefaultClosureValidationError<E> {
    TargetOrigin {
        site: ExternalHirDefaultUseSiteV1,
        target: ExternalHirTargetV1,
        error: E,
    },
    MissingReference {
        site: ExternalHirDefaultUseSiteV1,
        target: ExternalHirTargetV1,
    },
    OriginMismatch(Box<ExternalHirDefaultOriginMismatch>),
    MissingRole {
        site: ExternalHirDefaultUseSiteV1,
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    ExtraRole {
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    Resource(WireError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalHirDefaultOriginMismatch {
    pub site: ExternalHirDefaultUseSiteV1,
    pub record_index: usize,
    pub target: ExternalHirTargetV1,
    pub expected: scoop_identity::ConeIdentity,
    pub actual: scoop_identity::ConeIdentity,
}

impl<E: fmt::Display> fmt::Display for ExternalHirDefaultClosureValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetOrigin {
                site,
                target,
                error,
            } => write!(
                formatter,
                "default dependency target {target:?} used at {site:?} has no canonical origin: {error}"
            ),
            Self::MissingReference { site, target } => write!(
                formatter,
                "foreign default dependency target {target:?} used at {site:?} is absent from the external HIR reference table"
            ),
            Self::OriginMismatch(error) => write!(
                formatter,
                "default dependency target {:?} used at {:?} uses external record {} with origin {}, expected {}",
                error.target, error.site, error.record_index, error.actual, error.expected
            ),
            Self::MissingRole {
                site,
                record_index,
                target,
            } => write!(
                formatter,
                "template dependency target {target:?} used at {site:?} uses external record {record_index} without its required dependency role"
            ),
            Self::ExtraRole {
                record_index,
                target,
            } => write!(
                formatter,
                "external record {record_index} for target {target:?} has a template dependency role without a corresponding template reference"
            ),
            Self::Resource(error) => write!(
                formatter,
                "external default reference closure resource failure: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExternalHirDefaultClosureValidationError<E>
{
}

impl<E> From<WireError> for ExternalHirDefaultClosureValidationError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

#[cfg(test)]
mod tests;
