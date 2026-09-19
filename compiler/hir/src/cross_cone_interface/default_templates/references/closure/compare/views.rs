use super::*;

impl<'a> From<&'a ExportDefaultCallableTargetV1> for CallableTargetView<'a> {
    fn from(target: &'a ExportDefaultCallableTargetV1) -> Self {
        match target {
            ExportDefaultCallableTargetV1::Callable(value) => Self::Callable(value),
            ExportDefaultCallableTargetV1::Bound(value) => Self::Bound(value),
            ExportDefaultCallableTargetV1::DerivedEquality { owner_type } => {
                Self::DerivedEquality(owner_type)
            }
            ExportDefaultCallableTargetV1::LocalFunction { declaration } => {
                Self::LocalFunction(*declaration)
            }
            ExportDefaultCallableTargetV1::Lambda { body } => Self::Lambda(*body),
            ExportDefaultCallableTargetV1::AnonymousFunction { body } => {
                Self::AnonymousFunction(*body)
            }
            ExportDefaultCallableTargetV1::CallableReference { invoke } => {
                Self::CallableReference(*invoke)
            }
            ExportDefaultCallableTargetV1::FunctionAddress { declaration } => {
                Self::FunctionAddress(*declaration)
            }
        }
    }
}

impl CallableTargetView<'_> {
    /// Compares this actual target to the declared target using the shared budget.
    pub fn compare_to(
        self,
        declared: &ExportDefaultCallableTargetV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Ordering, WireError> {
        meter.charge_work(1, path)?;
        callable_target(declared, self, meter, path).map(Ordering::reverse)
    }
}

impl<'a> From<&'a DefaultConstructorRefV1> for ConstructorTargetView<'a> {
    fn from(target: &'a DefaultConstructorRefV1) -> Self {
        Self::Constructor(target)
    }
}

impl ConstructorTargetView<'_> {
    /// Compares this actual target to the declared target using the shared budget.
    pub fn compare_to(
        self,
        declared: &DefaultConstructorRefV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Ordering, WireError> {
        meter.charge_work(1, path)?;
        constructor_target(declared, self, meter, path).map(Ordering::reverse)
    }
}

impl<'a> From<&'a DefaultFieldRefV1> for FieldTargetView<'a> {
    fn from(target: &'a DefaultFieldRefV1) -> Self {
        Self::Field(target)
    }
}

impl FieldTargetView<'_> {
    /// Compares this actual target to the declared target using the shared budget.
    pub fn compare_to(
        self,
        declared: &DefaultFieldRefV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Ordering, WireError> {
        meter.charge_work(1, path)?;
        field_target(declared, self, meter, path).map(Ordering::reverse)
    }
}

/// Compares the left and right signature targets in canonical target order.
pub fn compare_default_signature_reference_targets(
    left: &SignatureTypeKey,
    right: &SignatureTypeKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Ordering, WireError> {
    signature_type(left, right, meter, path)
}
