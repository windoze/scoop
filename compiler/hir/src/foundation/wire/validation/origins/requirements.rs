//! Source anchors for declarations and generated callable materializations.

use std::collections::{HashMap, HashSet};

use scoop_identity::{
    CallableMaterialization, CallableTemplateOwner, DefinitionOriginSubject, GeneratedCallableKey,
    LocalValueSelector, PersistentGeneratedCallableId,
};
use scoop_wire::{WireError, WireErrorKind, WirePath};

use super::{
    DefinitionOriginValidationError, GeneratedCallableRecord, HirFoundationValidationError,
    LocalValueRecord, OriginExpectation,
};

pub(super) struct OriginRequirements<'a> {
    pub(super) required: HashMap<DefinitionOriginSubject, OriginExpectation<'a>>,
    pub(super) optional: HashMap<DefinitionOriginSubject, OriginExpectation<'a>>,
    generated: HashMap<PersistentGeneratedCallableId, &'a GeneratedCallableKey>,
    exact_types: &'a [crate::foundation::ExactTypeRecord],
    dependencies: &'a [&'a crate::CanonicalHirFoundation],
    visiting: HashSet<PersistentGeneratedCallableId>,
}

impl<'a> OriginRequirements<'a> {
    pub(super) fn new(
        generated_records: &'a [GeneratedCallableRecord],
        exact_types: &'a [crate::foundation::ExactTypeRecord],
        dependencies: &'a [&'a crate::CanonicalHirFoundation],
        required_count: usize,

        path: &WirePath,
    ) -> Result<Self, HirFoundationValidationError> {
        let mut required = HashMap::new();
        scoop_wire::allocation::try_reserve_map(&mut required, required_count, path)
            .map_err(HirFoundationValidationError::Resource)?;
        let mut optional = HashMap::new();
        scoop_wire::allocation::try_reserve_map(&mut optional, generated_records.len(), path)
            .map_err(HirFoundationValidationError::Resource)?;
        let mut generated = HashMap::new();
        scoop_wire::allocation::try_reserve_map(&mut generated, generated_records.len(), path)
            .map_err(HirFoundationValidationError::Resource)?;
        generated.extend(
            generated_records
                .iter()
                .map(|record| (record.id(), record.key())),
        );
        let mut visiting = HashSet::new();
        scoop_wire::allocation::try_reserve_set(&mut visiting, generated_records.len(), path)
            .map_err(HirFoundationValidationError::Resource)?;
        Ok(Self {
            required,
            optional,
            generated,
            exact_types,
            dependencies,
            visiting,
        })
    }

    fn generated_key(&self, id: PersistentGeneratedCallableId) -> Option<&GeneratedCallableKey> {
        self.generated.get(&id).copied().or_else(|| {
            self.dependencies.iter().find_map(|foundation| {
                foundation
                    .generated_callable_by_bytes(id.as_array())
                    .map(|(_, key)| key)
            })
        })
    }

    pub(super) fn require(
        &mut self,
        subject: DefinitionOriginSubject,
        expectation: OriginExpectation<'a>,
    ) -> Result<(), DefinitionOriginValidationError> {
        if self.required.insert(subject, expectation).is_some() {
            Err(DefinitionOriginValidationError::DuplicateRequirement { subject })
        } else {
            Ok(())
        }
    }

    pub(super) fn allow(
        &mut self,
        subject: DefinitionOriginSubject,
        expectation: OriginExpectation<'a>,
    ) -> Result<(), DefinitionOriginValidationError> {
        if self.optional.insert(subject, expectation).is_some() {
            Err(DefinitionOriginValidationError::DuplicateRequirement { subject })
        } else {
            Ok(())
        }
    }

    pub(super) fn local_value(
        &mut self,
        record: &LocalValueRecord,

        path: &WirePath,
    ) -> Result<(), HirFoundationValidationError> {
        let selector = record.key().selector();
        if !matches!(
            selector,
            LocalValueSelector::This
                | LocalValueSelector::Parameter { .. }
                | LocalValueSelector::LocalDeclaration { .. }
                | LocalValueSelector::BoundReceiver { .. }
        ) {
            return Ok(());
        }
        if matches!(
            selector,
            LocalValueSelector::This
                | LocalValueSelector::Parameter {
                    declaration_index: 0
                }
        ) && let CallableTemplateOwner::Generated(id) = record.key().owner().template()
            && matches!(
                self.generated_key(id),
                Some(GeneratedCallableKey::DerivedEquality { .. })
            )
        {
            // Type-owned method parameters are synthetic, despite their ABI selectors.
            // Keeping them outside both origin sets also rejects fabricated origins.
            return Ok(());
        }
        let subject = DefinitionOriginSubject::LocalValue(record.id());
        let anchor = self
            .materialization_subject(record.key().owner(), path)?
            .ok_or(DefinitionOriginValidationError::MissingSourceAnchor { subject })?;
        self.require(subject, OriginExpectation::SameSource(anchor))?;
        Ok(())
    }

    fn materialization_subject(
        &mut self,
        materialization: CallableMaterialization,

        path: &WirePath,
    ) -> Result<Option<DefinitionOriginSubject>, HirFoundationValidationError> {
        self.callable_subject(materialization.template(), path)
    }

    pub(super) fn callable_subject(
        &mut self,
        owner: CallableTemplateOwner,

        path: &WirePath,
    ) -> Result<Option<DefinitionOriginSubject>, HirFoundationValidationError> {
        self.visiting.clear();
        self.resolve_callable_subject(owner, path)
    }

    pub(super) fn generated_subject(
        &mut self,
        id: PersistentGeneratedCallableId,

        path: &WirePath,
    ) -> Result<Option<DefinitionOriginSubject>, HirFoundationValidationError> {
        self.visiting.clear();
        self.resolve_callable_subject(CallableTemplateOwner::Generated(id), path)
    }

    fn resolve_callable_subject(
        &mut self,
        mut owner: CallableTemplateOwner,

        path: &WirePath,
    ) -> Result<Option<DefinitionOriginSubject>, HirFoundationValidationError> {
        let mut depth = 1_u64;
        loop {
            match owner {
                CallableTemplateOwner::Function(id) => {
                    return Ok(Some(DefinitionOriginSubject::Function(id)));
                }
                CallableTemplateOwner::GenericFunction(id) => {
                    return Ok(Some(DefinitionOriginSubject::GenericFunction(id)));
                }
                CallableTemplateOwner::Constructor(id) => {
                    return Ok(Some(DefinitionOriginSubject::Constructor(id)));
                }
                CallableTemplateOwner::Accessor(id) => {
                    return Ok(Some(DefinitionOriginSubject::PropertyAccessor(id)));
                }
                CallableTemplateOwner::VariantConstructor(id) => {
                    return Ok(Some(DefinitionOriginSubject::EnumVariant(id)));
                }
                CallableTemplateOwner::ReleaseHook(exact) => {
                    let owner = self
                        .exact_types
                        .iter()
                        .find(|record| record.id() == exact)
                        .and_then(|record| crate::foundation::release::nominal_owner(record.key()))
                        .or_else(|| {
                            self.dependencies
                                .iter()
                                .find_map(|foundation| foundation.release_hook_owner(exact))
                        });
                    return Ok(owner.map(super::nominal_subject));
                }
                CallableTemplateOwner::Generated(id) => {
                    if !self.visiting.insert(id) {
                        return Ok(None);
                    }

                    let Some(key) = self.generated_key(id) else {
                        return Ok(None);
                    };
                    match key {
                        GeneratedCallableKey::Lexical { parent, .. }
                        | GeneratedCallableKey::CallableReferenceInvoke { parent, .. } => {
                            owner = parent.template();
                        }
                        GeneratedCallableKey::Initialization { unit, .. } => {
                            return Ok(Some(DefinitionOriginSubject::InitializationUnit(*unit)));
                        }
                        GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
                            source, ..
                        }
                        | GeneratedCallableKey::CoroutineDriver {
                            source_callable: source,
                        }
                        | GeneratedCallableKey::CoroutineAdapter {
                            source_callable: source,
                            ..
                        }
                        | GeneratedCallableKey::DispatchAdjust { target: source, .. } => {
                            owner = source.template();
                        }
                        GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                            return Ok(Some(DefinitionOriginSubject::Constructor(*constructor)));
                        }
                        GeneratedCallableKey::DerivedEquality { .. }
                        | GeneratedCallableKey::FunctionAdapter { .. }
                        | GeneratedCallableKey::DynamicFunctionAdapter { .. }
                        | GeneratedCallableKey::ForeignCallbackManagedAdapter { .. }
                        | GeneratedCallableKey::CoroutineStart { .. }
                        | GeneratedCallableKey::FunctionBridge { .. }
                        | GeneratedCallableKey::BoxingAdjust { .. } => return Ok(None),
                    }
                    depth = depth.checked_add(1).ok_or_else(|| {
                        HirFoundationValidationError::Resource(WireError::new(
                            WireErrorKind::IntegerOutOfRange,
                            path.clone(),
                            None,
                        ))
                    })?;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
