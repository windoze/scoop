use scoop_identity::{DecodedPersistentId, PersistentId};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::*;
use crate::{
    CanonicalExactDescriptorExportsV1, ExactDescriptorAncestryV1, ExactDescriptorDispatchV1,
    ExactDescriptorExportV1, ExactDescriptorTableError, StrongTypeDescriptorRefV2,
};

impl DecodedExactDescriptorExportV1 {
    pub fn validate_against(
        self,
        expected: &ExactDescriptorExportV1,
        meter: &mut BudgetMeter,
    ) -> Result<ExactDescriptorExportV1, ExactDescriptorWireError> {
        meter.charge_work(10, &WirePath::root())?;
        verify(self.exact, expected.exact())?;
        verify(
            self.value_layout,
            expected.value_layout().identity().layout(),
        )?;
        verify(
            self.instance_layout,
            expected.instance_layout().identity().layout(),
        )?;
        self.shape.validate_against(expected.shape(), meter)?;
        let object_scan = self.object_scan.validate_metered(meter)?;
        if object_scan.as_ref_scan() != expected.object_scan() {
            return Err(ExactDescriptorWireError::ObjectScan);
        }
        self.ancestry.validate_against(expected.ancestry(), meter)?;
        self.dispatch.validate_against(expected.dispatch(), meter)?;
        if self.diagnostic_name != expected.diagnostic_name().as_str() {
            return Err(ExactDescriptorWireError::DiagnosticName);
        }
        if !self
            .definition
            .matches_definition(expected.definition(), meter)?
        {
            return Err(ExactDescriptorWireError::Definition);
        }
        if !self
            .registration
            .matches_registration(expected.registration(), meter)?
        {
            return Err(ExactDescriptorWireError::Registration);
        }
        Ok(expected.clone())
    }
}

impl DecodedAncestry {
    fn validate_against(
        self,
        expected: &ExactDescriptorAncestryV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), ExactDescriptorWireError> {
        meter.charge_work(
            (self.interfaces.len() as u64).saturating_add(1),
            &WirePath::root(),
        )?;
        if !optional_ref_matches(self.parent, expected.parent())
            || self.interfaces.len() != expected.interfaces().len()
            || !self
                .interfaces
                .into_iter()
                .zip(expected.interfaces())
                .all(|(actual, expected)| ref_matches(actual, *expected))
        {
            return Err(ExactDescriptorWireError::Ancestry);
        }
        Ok(())
    }
}

impl DecodedDispatch {
    fn validate_against(
        self,
        expected: &ExactDescriptorDispatchV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), ExactDescriptorWireError> {
        meter.charge_work(
            (self.itables.len() as u64).saturating_add(1),
            &WirePath::root(),
        )?;
        verify(self.vtable, expected.vtable())?;
        if self.itables.len() != expected.itables().len() {
            return Err(ExactDescriptorWireError::Dispatch);
        }
        for (actual, expected) in self.itables.into_iter().zip(expected.itables()) {
            if !ref_matches(actual.interface, expected.interface()) {
                return Err(ExactDescriptorWireError::Dispatch);
            }
            verify(actual.table, expected.table())?;
        }
        Ok(())
    }
}

impl DecodedCanonicalExactDescriptorExportsV1 {
    pub fn validate_against(
        self,
        expected: &CanonicalExactDescriptorExportsV1,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalExactDescriptorExportsV1, ExactDescriptorTableError> {
        meter.charge_work(self.records.len() as u64, &WirePath::root())?;
        if self.records.len() != expected.records().len() {
            return Err(ExactDescriptorTableError::Count);
        }
        for (index, (actual, expected)) in
            self.records.into_iter().zip(expected.records()).enumerate()
        {
            actual
                .validate_against(expected, meter)
                .map_err(|source| ExactDescriptorTableError::Record { index, source })?;
        }
        Ok(expected.clone())
    }
}

fn optional_ref_matches(
    actual: DecodedOptionalStrongTypeDescriptorRefV2,
    expected: Option<StrongTypeDescriptorRefV2>,
) -> bool {
    match (actual, expected) {
        (DecodedOptionalStrongTypeDescriptorRefV2::Absent, None) => true,
        (
            DecodedOptionalStrongTypeDescriptorRefV2::Local(actual),
            Some(StrongTypeDescriptorRefV2::Local(expected)),
        )
        | (
            DecodedOptionalStrongTypeDescriptorRefV2::CoreExternal(actual),
            Some(StrongTypeDescriptorRefV2::CoreExternal(expected)),
        ) => actual.verify(expected).is_ok(),
        (
            DecodedOptionalStrongTypeDescriptorRefV2::DependencyExternal {
                provider: actual_provider,
                exact: actual_exact,
            },
            Some(StrongTypeDescriptorRefV2::DependencyExternal {
                provider: expected_provider,
                exact: expected_exact,
            }),
        ) => {
            actual_provider.verify(expected_provider).is_ok()
                && actual_exact.verify(expected_exact).is_ok()
        }
        _ => false,
    }
}

fn ref_matches(
    actual: DecodedStrongTypeDescriptorRefV2,
    expected: StrongTypeDescriptorRefV2,
) -> bool {
    match (actual, expected) {
        (
            DecodedStrongTypeDescriptorRefV2::Local(actual),
            StrongTypeDescriptorRefV2::Local(expected),
        )
        | (
            DecodedStrongTypeDescriptorRefV2::CoreExternal(actual),
            StrongTypeDescriptorRefV2::CoreExternal(expected),
        ) => actual.verify(expected).is_ok(),
        (
            DecodedStrongTypeDescriptorRefV2::DependencyExternal {
                provider: actual_provider,
                exact: actual_exact,
            },
            StrongTypeDescriptorRefV2::DependencyExternal {
                provider: expected_provider,
                exact: expected_exact,
            },
        ) => {
            actual_provider.verify(expected_provider).is_ok()
                && actual_exact.verify(expected_exact).is_ok()
        }
        _ => false,
    }
}

fn verify<I: PersistentId>(
    actual: DecodedPersistentId<I>,
    expected: I,
) -> Result<(), ExactDescriptorWireError> {
    actual
        .verify(expected)
        .map(|_| ())
        .map_err(|_| ExactDescriptorWireError::Identity)
}

#[derive(Debug)]
pub enum ExactDescriptorWireError {
    Identity,
    ObjectScan,
    Ancestry,
    Dispatch,
    DiagnosticName,
    Definition,
    Registration,
    Shape(crate::TypeInstanceShapeWireError),
    Scan(crate::MeteredScanValidationError),
    Resource(WireError),
}

impl From<crate::TypeInstanceShapeWireError> for ExactDescriptorWireError {
    fn from(error: crate::TypeInstanceShapeWireError) -> Self {
        Self::Shape(error)
    }
}

impl From<crate::MeteredScanValidationError> for ExactDescriptorWireError {
    fn from(error: crate::MeteredScanValidationError) -> Self {
        Self::Scan(error)
    }
}

impl From<WireError> for ExactDescriptorWireError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for ExactDescriptorWireError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "descriptor wire differs from checked replay: {self:?}"
        )
    }
}

impl std::error::Error for ExactDescriptorWireError {}
