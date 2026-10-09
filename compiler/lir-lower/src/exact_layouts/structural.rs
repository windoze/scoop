use super::*;

impl Projection<'_> {
    pub(super) fn structural_value(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
    ) -> Result<lir::ExactValueLayoutV1> {
        let foundation = self.output.foundation();
        if identity.layout_key().representation() != RepresentationRole::ManagedValue {
            return Err(ExactLayoutLoweringError::Role(identity.layout()));
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
                    identity, &inputs, foundation,
                )?);
            }
            ExactTypeKey::Function { .. } => lir::NullNicheKind::Managed,
            ExactTypeKey::RawPointer(_) => lir::NullNicheKind::Raw,
            ExactTypeKey::NativeFunctionPointer { .. } => lir::NullNicheKind::Code,
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
                return Err(ExactLayoutLoweringError::SourceRepresentation(
                    identity.exact(),
                ));
            }
        };
        Ok(lir::ExactValueLayoutV1::qualified_pointer(
            identity, pointer, foundation,
        )?)
    }
}
