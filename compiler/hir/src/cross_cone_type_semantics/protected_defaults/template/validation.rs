use scoop_identity::CallableTemplateOrigin;

use super::super::{ProtectedDefaultReferenceKindV1, ProtectedDefaultReferenceV1};
use super::*;

impl ProtectedDefaultTemplateV1 {
    pub(super) fn finish(self) -> Result<Self, ProtectedDefaultTemplateBuildError> {
        if self.body.value().result_type() != &self.result {
            return Err(self.result_error());
        }
        self.validate_locals_and_owners()?;
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
