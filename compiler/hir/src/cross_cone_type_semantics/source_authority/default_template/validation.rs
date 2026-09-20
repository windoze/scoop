use super::*;
use crate::{
    DefaultSourceReferenceV1, ExportDefaultReferenceKindV1,
    compare_default_signature_reference_targets,
};
use DefaultSourceTemplateBuildError as Error;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WirePath};
impl DefaultSourceTemplateV1 {
    pub(super) fn validate_shape(&self, meter: &mut BudgetMeter) -> Result<(), Error> {
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
            return Err(Error::ResultType);
        }
        let provider = self.definition_root.declaration();
        use ExportDefaultReferenceKindV1 as Kind;
        owners(self.references.callables(), Kind::Callable, provider, meter)?;
        owners(
            self.references.constructors(),
            Kind::Constructor,
            provider,
            meter,
        )?;
        owners(self.references.types(), Kind::Type, provider, meter)?;
        owners(self.references.globals(), Kind::Global, provider, meter)?;
        owners(
            self.references.singleton_values(),
            Kind::Singleton,
            provider,
            meter,
        )?;
        owners(self.references.fields(), Kind::Field, provider, meter)
    }
}
fn owners<T>(
    records: &[DefaultSourceReferenceV1<T>],
    kind: ExportDefaultReferenceKindV1,
    expected: CallableTemplateOrigin,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    meter
        .check_table_entries(records.len() as u64, &WirePath::root())
        .map_err(Error::Resource)?;
    meter
        .charge_work(records.len() as u64, &WirePath::root())
        .map_err(Error::Resource)?;
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
