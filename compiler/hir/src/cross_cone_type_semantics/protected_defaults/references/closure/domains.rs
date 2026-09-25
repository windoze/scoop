use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{WireError, WirePath};

use super::super::{ProtectedDefaultReferenceKindV1 as Kind, ProtectedDefaultReferenceSetV1};
use super::{
    ProtectedDefaultBodyClosureError, ProtectedDefaultReferenceBodySemanticAuthority,
    collect::Collected, receiver, records::Domain,
};
use crate::{
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1 as Target,
    DefaultConstructorRefV1, DefaultFieldRefV1, ExportDefaultCallableTargetV1,
    OptionalTemplateReceiverV1, ProtectedDefaultTemplateKeyV1,
    compare_default_signature_reference_targets,
};

pub(super) struct Domains<'a> {
    callables: Domain<'a, ExportDefaultCallableTargetV1>,
    constructors: Domain<'a, DefaultConstructorRefV1>,
    types: Domain<'a, SignatureTypeKey>,
    globals: Domain<'a, PersistentPropertyId>,
    singletons: Domain<'a, PersistentObjectValueId>,
    fields: Domain<'a, DefaultFieldRefV1>,
}
impl<'a> Domains<'a> {
    pub fn new(
        references: &'a ProtectedDefaultReferenceSetV1,

        path: &WirePath,
    ) -> Result<Self, WireError> {
        Ok(Self {
            callables: Domain::new(references.callables(), Kind::Callable, path)?,
            constructors: Domain::new(references.constructors(), Kind::Constructor, path)?,
            types: Domain::new(references.types(), Kind::Type, path)?,
            globals: Domain::new(references.globals(), Kind::Global, path)?,
            singletons: Domain::new(references.singleton_values(), Kind::Singleton, path)?,
            fields: Domain::new(references.fields(), Kind::Field, path)?,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn observe<A: ProtectedDefaultReferenceBodySemanticAuthority<E>, E>(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        occurrence: DefaultBodyReferenceOccurrenceV1<'_>,
        template_receiver: &OptionalTemplateReceiverV1,
        collected: &Collected<'_>,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), ProtectedDefaultBodyClosureError<E>> {
        if matches!(
            occurrence.target,
            Target::Type(SignatureTypeKey::Binder { .. })
        ) {
            return Ok(());
        }
        let context = receiver::project_default_reference_context(
            occurrence,
            template_receiver,
            &collected.expressions,
        )
        .map_err(|error| match error {
            receiver::DefaultReferenceReceiverError::Resource(error) => {
                ProtectedDefaultBodyClosureError::Resource(error)
            }
            receiver::DefaultReferenceReceiverError::ReceiverOutsideBody => {
                ProtectedDefaultBodyClosureError::ReceiverOutsideBody
            }
        })?;
        let expected_use = context.expression_use();
        let receiver = context.receiver();
        macro_rules! observe {
            ($domain:ident, $compare:expr) => {
                self.$domain.observe(
                    key,
                    occurrence,
                    expected_use,
                    receiver,
                    $compare,
                    authority,
                    path,
                )
            };
        }
        match occurrence.target {
            Target::Callable(target) => {
                observe!(callables, |record, path| target.compare_to(record, path))
            }
            Target::Constructor(target) => {
                observe!(constructors, |record, path| target.compare_to(record, path))
            }
            Target::Field(target) => {
                observe!(fields, |record, path| target.compare_to(record, path))
            }
            Target::Type(target) => observe!(types, |record, path| {
                compare_default_signature_reference_targets(target, record, path)
            }),
            Target::Global(target) => observe!(globals, |record, _path| Ok(target.cmp(record))),
            Target::Singleton(target) => {
                observe!(singletons, |record, _path| Ok(target.cmp(record)))
            }
        }
    }
    pub fn finish<E>(&self) -> Result<(), ProtectedDefaultBodyClosureError<E>> {
        self.callables.finish()?;
        self.constructors.finish()?;
        self.types.finish()?;
        self.globals.finish()?;
        self.singletons.finish()?;
        self.fields.finish()
    }
}
