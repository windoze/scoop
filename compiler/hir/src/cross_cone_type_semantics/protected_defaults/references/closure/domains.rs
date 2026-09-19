use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};

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
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, WireError> {
        Ok(Self {
            callables: Domain::new(references.callables(), Kind::Callable, meter, path)?,
            constructors: Domain::new(references.constructors(), Kind::Constructor, meter, path)?,
            types: Domain::new(references.types(), Kind::Type, meter, path)?,
            globals: Domain::new(references.globals(), Kind::Global, meter, path)?,
            singletons: Domain::new(references.singleton_values(), Kind::Singleton, meter, path)?,
            fields: Domain::new(references.fields(), Kind::Field, meter, path)?,
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
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ProtectedDefaultBodyClosureError<E>> {
        if matches!(
            occurrence.target,
            Target::Type(SignatureTypeKey::Binder { .. })
        ) {
            return Ok(());
        }
        let (expected_use, receiver) =
            receiver::project(occurrence, template_receiver, collected, meter, path)?;
        macro_rules! observe {
            ($domain:ident, $compare:expr) => {
                self.$domain.observe(
                    key,
                    occurrence,
                    expected_use,
                    receiver,
                    $compare,
                    authority,
                    meter,
                    path,
                )
            };
        }
        match occurrence.target {
            Target::Callable(target) => observe!(callables, |record, meter, path| target
                .compare_to(record, meter, path)),
            Target::Constructor(target) => observe!(constructors, |record, meter, path| target
                .compare_to(record, meter, path)),
            Target::Field(target) => observe!(fields, |record, meter, path| target
                .compare_to(record, meter, path)),
            Target::Type(target) => observe!(types, |record, meter, path| {
                compare_default_signature_reference_targets(target, record, meter, path)
            }),
            Target::Global(target) => observe!(globals, |record, meter, path| {
                meter.charge_work(1, path)?;
                Ok(target.cmp(record))
            }),
            Target::Singleton(target) => observe!(singletons, |record, meter, path| {
                meter.charge_work(1, path)?;
                Ok(target.cmp(record))
            }),
        }
    }
    pub fn finish<E>(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ProtectedDefaultBodyClosureError<E>> {
        self.callables.finish(meter, path)?;
        self.constructors.finish(meter, path)?;
        self.types.finish(meter, path)?;
        self.globals.finish(meter, path)?;
        self.singletons.finish(meter, path)?;
        self.fields.finish(meter, path)
    }
}
