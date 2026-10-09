use super::*;

impl Replay<'_> {
    pub(super) fn value_constituent(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<lir::ValueLayoutConstituentV1> {
        if let Some(value) = self.values.get(&exact) {
            return Ok(value.clone());
        }
        let target = self.target;
        let key = self.identities.canonical_record::<_, ExactTypeKey>(exact)?;
        let storage = match key.key() {
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
                let value = self.value_dependency(exact)?.value().clone();
                self.values.insert(exact, value.clone());
                return Ok(value);
            }
            ExactTypeKey::Tuple(elements) => {
                let mut values = self.reserve(elements.as_slice().len())?;
                for element in elements.as_slice() {
                    values.push(self.value_constituent(*element)?);
                }
                let mut inputs = self.reserve(values.len())?;
                inputs.extend(values.iter());
                lir::TupleStorageLayoutV1::replay(target, &key, &inputs)
                    .map_err(lir::ExactLayoutReplayError::from)?
                    .storage()
                    .clone()
            }
            ExactTypeKey::Function { .. } => pointer_storage(
                target.managed_pointer_layout(),
                lir::RefScan::References(vec![0]),
            )?,
            ExactTypeKey::RawPointer(_) => {
                pointer_storage(target.data_pointer().layout(), lir::RefScan::None)?
            }
            ExactTypeKey::NativeFunctionPointer { .. } => {
                pointer_storage(target.code_pointer().layout(), lir::RefScan::None)?
            }
        };
        let value = lir::ValueLayoutConstituentV1::new(
            target,
            LayoutKey::new(exact, target.wire_id(), RepresentationRole::ManagedValue),
            storage,
        )
        .map_err(lir::ExactLayoutReplayError::from)?;
        self.values.insert(exact, value.clone());
        Ok(value)
    }
    pub(super) fn niche_pointer_kind(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<Option<lir::NullNicheKind>> {
        Ok(
            match self
                .identities
                .canonical_key::<_, ExactTypeKey>(exact)?
                .as_ref()
            {
                ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
                    match self.value_dependency(exact)?.representation().kind() {
                        lir::ExactRepresentationKindV1::QualifiedPointer(kind) => Some(kind),
                        lir::ExactRepresentationKindV1::Interface => {
                            Some(lir::NullNicheKind::Interface)
                        }
                        _ => None,
                    }
                }
                ExactTypeKey::Function { .. } => Some(lir::NullNicheKind::Managed),
                ExactTypeKey::RawPointer(_) => Some(lir::NullNicheKind::Raw),
                ExactTypeKey::NativeFunctionPointer { .. } => Some(lir::NullNicheKind::Code),
                ExactTypeKey::Tuple(_) => None,
            },
        )
    }
}

fn pointer_storage(
    layout: lir::ScalarLayout,
    scan: lir::RefScan,
) -> Result<lir::ValueStorageLayoutV1> {
    Ok(
        lir::ValueStorageLayoutV1::inline(layout.size_bytes(), layout.alignment_bytes(), scan)
            .map_err(lir::ExactLayoutReplayError::from)?,
    )
}
