use super::*;

impl Replay<'_> {
    pub(super) fn structural_value(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
    ) -> Result<lir::ExactValueLayoutV1> {
        if identity.layout_key().representation() != RepresentationRole::ManagedValue {
            return Err(Error::Role(identity.layout()));
        }
        let pointer = match identity.exact_key() {
            ExactTypeKey::Tuple(elements) => {
                let mut values = self.reserve(elements.as_slice().len())?;
                for element in elements.as_slice() {
                    values.push(self.value_constituent(*element)?);
                }
                let mut inputs = self.reserve(values.len())?;
                inputs.extend(values.iter());
                return Ok(lir::ExactValueLayoutV1::tuple(
                    identity,
                    &inputs,
                    self.foundation,
                )?);
            }
            ExactTypeKey::Function { .. } => lir::NichePointerKind::Managed,
            ExactTypeKey::RawPointer(_) => lir::NichePointerKind::Raw,
            ExactTypeKey::NativeFunctionPointer { .. } => lir::NichePointerKind::Code,
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
                return Err(Error::MissingMirShape(identity.exact()));
            }
        };
        Ok(lir::ExactValueLayoutV1::qualified_pointer(
            identity,
            pointer,
            self.foundation,
        )?)
    }
}
