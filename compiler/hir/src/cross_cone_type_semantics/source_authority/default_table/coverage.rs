use super::*;
use crate::{
    CanonicalNominalSourceParameterProtocolsV1, ProtectedParameterCallingKindV1,
    ProtectedSourceBuildError,
};
use scoop_wire::WireError;
use std::fmt;

impl CanonicalDefaultSourceTemplatesV1 {
    /// Exact source-to-source coverage only, without default or execution authority.
    pub fn validate_parameter_coverage(
        &self,
        parameters: &CanonicalNominalSourceParameterProtocolsV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), DefaultSourceTemplateCoverageError> {
        use DefaultSourceTemplateCoverageError as Error;
        let path = WirePath::root();
        meter
            .check_semantic_depth(1, &path)
            .map_err(Error::Resource)?;
        meter
            .check_table_entries(parameters.records().len() as u64, &path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        meter
            .check_table_entries(self.records.len() as u64, &path)
            .map_err(Error::Resource)?;
        let mut actual = self.records.iter();
        for protocol in parameters.records() {
            meter.charge_work(1, &path).map_err(Error::Resource)?;
            meter
                .check_table_entries(protocol.parameters().len() as u64, &path)
                .map_err(Error::Resource)?;
            for (position, parameter) in protocol.parameters().iter().enumerate() {
                meter.charge_work(1, &path).map_err(Error::Resource)?;
                if matches!(
                    parameter.calling_kind(),
                    ProtectedParameterCallingKindV1::Required
                        | ProtectedParameterCallingKindV1::VarargEmpty
                ) {
                    continue;
                }
                let position = u32::try_from(position).map_err(|_| Error::PositionOverflow)?;
                let expected = ProtectedDefaultTemplateKeyV1::try_new(protocol.owner(), position)
                    .map_err(Error::Key)?;
                meter.charge_work(64, &path).map_err(Error::Resource)?;
                let Some(record) = actual.next() else {
                    return Err(Error::Missing(expected));
                };
                match record.key().cmp(&expected) {
                    std::cmp::Ordering::Less => return Err(Error::Extra(record.key())),
                    std::cmp::Ordering::Greater => return Err(Error::Missing(expected)),
                    std::cmp::Ordering::Equal => {}
                }
            }
        }
        if let Some(record) = actual.next() {
            return Err(Error::Extra(record.key()));
        }
        Ok(())
    }
}
#[derive(Debug)]
pub enum DefaultSourceTemplateCoverageError {
    Resource(WireError),
    Key(ProtectedSourceBuildError),
    PositionOverflow,
    Missing(ProtectedDefaultTemplateKeyV1),
    Extra(ProtectedDefaultTemplateKeyV1),
}
impl fmt::Display for DefaultSourceTemplateCoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Key(e) => e.fmt(f),
            Self::PositionOverflow => f.write_str("source parameter position exceeds u32"),
            Self::Missing(key) => write!(f, "source parameter has no default body at {key:?}"),
            Self::Extra(key) => {
                write!(f, "source default body has no default parameter at {key:?}")
            }
        }
    }
}
impl std::error::Error for DefaultSourceTemplateCoverageError {}
