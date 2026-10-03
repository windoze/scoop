use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, CanonicalScoopStorage, ExactCallableSignature,
    ExactTypeKey, GcEffect, LayoutKey, ScoopAbiValueShape, ValidatedIdentityGraph,
};
use scoop_wire::WirePath;

use super::*;

impl Layouts<'_> {
    pub(in crate::layout_compile_closure) fn signature(
        &mut self,
        signature: &ExactCallableSignature,
        effect: GcEffect,
        identities: &ValidatedIdentityGraph,
    ) -> Result<CanonicalScoopAbiFunctionSignature, Error> {
        let target = self.local.target();
        let mut arguments = Vec::new();
        scoop_wire::allocation::try_reserve(
            &mut arguments,
            signature.parameters().len() + usize::from(signature.receiver().is_present()),
            &WirePath::root(),
        )?;
        for exact in signature
            .receiver()
            .into_option()
            .into_iter()
            .chain(signature.parameters().iter().copied())
        {
            let (value, shape) = self.storage(exact, identities)?;
            arguments.push(lir::canonical_scoop_abi_argument(
                target,
                canonical(&value, shape),
            )?);
        }
        let result = if let Some(value) = self.value_if_present(signature.result())? {
            value
                .value_handle()
                .ok_or(lir::ExactCallableAbiError::LayoutRole)?
                .scoop_abi_return(target)?
        } else {
            let (value, shape) = self.storage(signature.result(), identities)?;
            lir::canonical_scoop_abi_value_return(target, canonical(&value, shape))?
        };
        Ok(CanonicalScoopAbiFunctionSignature::new(
            signature.clone(),
            arguments,
            result,
            effect,
        )?)
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
