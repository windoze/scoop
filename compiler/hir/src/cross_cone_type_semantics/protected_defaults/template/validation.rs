use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WirePath};

use super::super::{ProtectedDefaultReferenceKindV1, ProtectedDefaultReferenceV1};
use super::*;
use crate::compare_default_signature_reference_targets;

impl ProtectedDefaultTemplateV1 {
    pub(super) fn finish(self) -> Result<Self, ProtectedDefaultTemplateBuildError> {
        if self.body.value().result_type() != &self.result {
            return Err(self.result_error());
        }
        self.validate_locals_and_owners()?;
        Ok(self)
    }

    pub(super) fn finish_metered<E>(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ProtectedDefaultTemplateResolutionError<E>> {
        use ProtectedDefaultTemplateResolutionError as Error;
        let path = WirePath::root();
        if !compare_default_signature_reference_targets(
            self.body.value().result_type(),
            &self.result,
            meter,
            &path,
        )
        .map_err(Error::Resource)?
        .is_eq()
        {
            return Err(Error::Record(self.result_error()));
        }
        let references = &self.references;
        for count in [
            references.callables().len(),
            references.constructors().len(),
            references.types().len(),
            references.globals().len(),
            references.singleton_values().len(),
            references.fields().len(),
        ] {
            meter
                .charge_work(count as u64, &path)
                .map_err(Error::Resource)?;
        }
        self.validate_locals_and_owners().map_err(Error::Record)?;
        Ok(self)
    }

    fn result_error(self) -> ProtectedDefaultTemplateBuildError {
        ProtectedDefaultTemplateBuildError::ResultType {
            declared: self.result,
            body: self.body.value().result_type().clone(),
        }
    }

    fn validate_locals_and_owners(&self) -> Result<(), ProtectedDefaultTemplateBuildError> {
        self.index_locals()
            .map_err(ProtectedDefaultTemplateBuildError::LocalIndices)?;
        let expected = self.key.owner();
        let references = &self.references;
        use ProtectedDefaultReferenceKindV1 as Kind;
        owners(references.callables(), Kind::Callable, expected)?;
        owners(references.constructors(), Kind::Constructor, expected)?;
        owners(references.types(), Kind::Type, expected)?;
        owners(references.globals(), Kind::Global, expected)?;
        owners(references.singleton_values(), Kind::Singleton, expected)?;
        owners(references.fields(), Kind::Field, expected)
    }
}
fn owners<T>(
    records: &[ProtectedDefaultReferenceV1<T>],
    kind: ProtectedDefaultReferenceKindV1,
    expected: CallableTemplateOrigin,
) -> Result<(), ProtectedDefaultTemplateBuildError> {
    for (index, record) in records.iter().enumerate() {
        let actual = record.witness().owner();
        if actual != expected {
            return Err(ProtectedDefaultTemplateBuildError::ReferenceOwner {
                kind,
                index,
                expected,
                actual,
            });
        }
    }
    Ok(())
}
