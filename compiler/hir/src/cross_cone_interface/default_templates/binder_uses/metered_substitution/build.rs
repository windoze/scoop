use super::*;
use scoop_identity::NonEmptyVec;

impl Substitution<'_, '_> {
    pub(super) fn finish(
        &mut self,
        source: &SignatureTypeKey,
    ) -> Result<(), MeteredDefaultTemplateTypeSubstitutionError> {
        self.meter.charge_work(1, self.path)?;
        let value = match source {
            SignatureTypeKey::NominalApplication { origin, arguments } => {
                SignatureTypeKey::NominalApplication {
                    origin: *origin,
                    arguments: self.non_empty(arguments.as_slice().len())?,
                }
            }
            SignatureTypeKey::Tuple(elements) => {
                SignatureTypeKey::Tuple(self.non_empty(elements.as_slice().len())?)
            }
            SignatureTypeKey::Function {
                effect, parameters, ..
            } => {
                let result = self.boxed()?;
                SignatureTypeKey::Function {
                    effect: *effect,
                    parameters: self.take(parameters.len())?,
                    result,
                }
            }
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                ..
            } => {
                let result = self.boxed()?;
                SignatureTypeKey::NativeFunctionPointer {
                    calling_convention: *calling_convention,
                    parameters: self.take(parameters.len())?,
                    result,
                }
            }
            SignatureTypeKey::RawPointer(_) => SignatureTypeKey::RawPointer(self.boxed()?),
            SignatureTypeKey::Nominal(_) | SignatureTypeKey::Binder { .. } => {
                return Err(invalid_length(1, 0, self.path).into());
            }
        };
        self.values.push(value);
        Ok(())
    }
    fn pop(&mut self) -> Result<SignatureTypeKey, WireError> {
        self.values
            .pop()
            .ok_or_else(|| invalid_length(1, 0, self.path))
    }
    fn boxed(&mut self) -> Result<Box<SignatureTypeKey>, WireError> {
        self.meter
            .charge_owned_bytes(std::mem::size_of::<SignatureTypeKey>() as u64, self.path)?;
        Ok(Box::new(self.pop()?))
    }
    fn take(&mut self, count: usize) -> Result<Vec<SignatureTypeKey>, WireError> {
        let start = self
            .values
            .len()
            .checked_sub(count)
            .ok_or_else(|| invalid_length(count, self.values.len(), self.path))?;
        let mut values = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut values, count, self.path)?;
        values.extend(self.values.drain(start..));
        Ok(values)
    }
    fn non_empty(&mut self, count: usize) -> Result<NonEmptyVec<SignatureTypeKey>, WireError> {
        NonEmptyVec::new(self.take(count)?).map_err(|_| invalid_length(1, 0, self.path))
    }
}
