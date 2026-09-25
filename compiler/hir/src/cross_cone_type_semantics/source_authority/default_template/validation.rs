use super::*;
use crate::{
    DefaultSourceReferenceV1, ExportDefaultReferenceKindV1,
    compare_default_signature_reference_targets,
};
use DefaultSourceTemplateBuildError as Error;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::WirePath;
impl DefaultSourceTemplateV1 {
    pub(super) fn validate_shape(&self) -> Result<(), Error> {
        let path = WirePath::root();
        if !compare_default_signature_reference_targets(
            self.body.value().result_type(),
            &self.result,
            &path,
        )
        .map_err(Error::Resource)?
        .is_eq()
        {
            return Err(Error::ResultType);
        }
        let provider = self.definition_root.declaration();
        use ExportDefaultReferenceKindV1 as Kind;
        owners(self.references.callables(), Kind::Callable, provider)?;
        owners(self.references.constructors(), Kind::Constructor, provider)?;
        owners(self.references.types(), Kind::Type, provider)?;
        owners(self.references.globals(), Kind::Global, provider)?;
        owners(
            self.references.singleton_values(),
            Kind::Singleton,
            provider,
        )?;
        owners(self.references.fields(), Kind::Field, provider)
    }
}
fn owners<T>(
    records: &[DefaultSourceReferenceV1<T>],
    kind: ExportDefaultReferenceKindV1,
    expected: CallableTemplateOrigin,
) -> Result<(), Error> {
    for (index, record) in records.iter().enumerate() {
        let actual = record.witness().owner();
        if actual != expected {
            return Err(Error::ReferenceProvider {
                kind,
                index,
                expected,
                actual,
            });
        }
    }
    Ok(())
}
