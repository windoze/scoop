use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, CanonicalScoopStorage, ExactTypeKey, LayoutKey,
    ScoopAbiValueShape, ValidatedIdentityGraph,
};
use scoop_wire::WirePath;

use super::*;

impl Layouts<'_> {
    pub(in crate::layout_compile_closure) fn check_signature(
        &mut self,
        signature: &CanonicalScoopAbiFunctionSignature,
        identities: &ValidatedIdentityGraph,
    ) -> Result<(), Error> {
        for argument in signature.arguments() {
            self.check_storage(argument.storage(), identities)?;
            if let scoop_identity::ScoopAbiArgument::DirectParts(storage, _) = argument {
                let (value, _) = self.storage(storage.exact_type(), identities)?;
                if storage.shape() == ScoopAbiValueShape::Aggregate && has_gc(value.storage()) {
                    return Err(Error::StorageMismatch(storage.exact_type()));
                }
            }
        }
        let exact = signature.signature().result();
        self.storage(exact, identities)?;
        let is_unit = self
            .value_if_present(exact)?
            .and_then(lir::ExactLayoutExportV1::value_handle)
            .is_some_and(|value| {
                matches!(
                    value.representation().kind(),
                    lir::ExactRepresentationKindV1::IntrinsicValue(
                        lir::IntrinsicValueFamilyV1::Unit
                    )
                )
            });
        if is_unit != matches!(signature.result(), scoop_identity::ScoopAbiReturn::UnitVoid) {
            return Err(Error::StorageMismatch(exact));
        }
        if let Some(storage) = signature.result().storage() {
            self.check_storage(storage, identities)?;
            if matches!(
                signature.result(),
                scoop_identity::ScoopAbiReturn::DirectParts(_, _)
            ) && storage.shape() == ScoopAbiValueShape::Aggregate
                && has_gc(self.storage(exact, identities)?.0.storage())
            {
                return Err(Error::StorageMismatch(exact));
            }
        }
        Ok(())
    }

    fn check_storage(
        &mut self,
        actual: CanonicalScoopStorage,
        identities: &ValidatedIdentityGraph,
    ) -> Result<(), Error> {
        let (value, shape) = self.storage(actual.exact_type(), identities)?;
        if canonical(&value, shape) != actual {
            return Err(Error::StorageMismatch(actual.exact_type()));
        }
        Ok(())
    }

    fn storage(
        &mut self,
        exact: PersistentExactTypeId,
        identities: &ValidatedIdentityGraph,
    ) -> Result<(lir::ValueLayoutConstituentV1, ScoopAbiValueShape), Error> {
        if let Some(value) = self.values.get(&exact) {
            return Ok(value.clone());
        }
        let target = self.local.target();
        let key = identities.canonical_record::<_, ExactTypeKey>(exact)?;
        let (storage, shape) = match key.key() {
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
                let record = self.value(exact)?;
                let value = record
                    .value_handle()
                    .ok_or(lir::ExactCallableAbiError::LayoutRole)?;
                let result = (value.value().clone(), value.canonical_storage().shape());
                self.values.insert(exact, result.clone());
                return Ok(result);
            }
            ExactTypeKey::Tuple(elements) => {
                let mut values = Vec::new();
                let mut inputs = Vec::new();
                scoop_wire::allocation::try_reserve(
                    &mut values,
                    elements.as_slice().len(),
                    &WirePath::root(),
                )?;
                scoop_wire::allocation::try_reserve(
                    &mut inputs,
                    elements.as_slice().len(),
                    &WirePath::root(),
                )?;
                for element in elements.as_slice() {
                    values.push(self.storage(*element, identities)?.0);
                }
                inputs.extend(values.iter());
                let tuple = lir::TupleStorageLayoutV1::replay(target, &key, &inputs)?;
                (tuple.storage().clone(), ScoopAbiValueShape::Aggregate)
            }
            ExactTypeKey::Function { .. } => (
                pointer_storage(
                    target.managed_pointer_layout(),
                    lir::RefScan::References(vec![0]),
                )?,
                ScoopAbiValueShape::Scalar,
            ),
            ExactTypeKey::RawPointer(_) => (
                pointer_storage(target.data_pointer().layout(), lir::RefScan::None)?,
                ScoopAbiValueShape::Scalar,
            ),
            ExactTypeKey::NativeFunctionPointer { .. } => (
                pointer_storage(target.code_pointer().layout(), lir::RefScan::None)?,
                ScoopAbiValueShape::Scalar,
            ),
        };
        let value = lir::ValueLayoutConstituentV1::new(
            target,
            LayoutKey::new(exact, target.wire_id(), RepresentationRole::ManagedValue),
            storage,
        )?;
        self.values.insert(exact, (value.clone(), shape));
        Ok((value, shape))
    }
}

fn canonical(
    value: &lir::ValueLayoutConstituentV1,
    shape: ScoopAbiValueShape,
) -> CanonicalScoopStorage {
    CanonicalScoopStorage::new(
        value.exact(),
        value.storage().byte_size(),
        value.storage().alignment().as_nonzero(),
        shape,
    )
}

fn pointer_storage(
    layout: lir::ScalarLayout,
    scan: lir::RefScan,
) -> Result<lir::ValueStorageLayoutV1, Error> {
    Ok(lir::ValueStorageLayoutV1::inline(
        layout.size_bytes(),
        layout.alignment_bytes(),
        scan,
    )?)
}

fn has_gc(storage: &lir::ValueStorageLayoutV1) -> bool {
    storage
        .nonzero()
        .is_some_and(|value| value.scan().as_ref_scan() != &lir::RefScan::None)
}
