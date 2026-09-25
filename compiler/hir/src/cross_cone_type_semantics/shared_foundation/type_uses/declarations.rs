use super::*;

impl Graph<'_> {
    pub(super) fn close(&mut self) -> Result<(), Error> {
        while let Some(owner) = self.pending.pop() {
            // Each owner is expanded once. The original declaration references
            // retain every semantic edge, including repeated type positions.
            for requirement in self.requirements.remove(&owner).unwrap_or_default() {
                use NominalMaterializationRequirementV1 as Requirement;
                match requirement {
                    Requirement::Field { field, .. } => {
                        self.signature(field.value_type(), Kind::Representation)?;
                    }
                    Requirement::EnumVariantField { field, .. } => {
                        self.signature(field.value_type(), Kind::Representation)?;
                    }
                    Requirement::Inheritance { owner, parent } => {
                        self.inheritance(owner, parent)?;
                    }
                    Requirement::Constructor { callable, .. }
                    | Requirement::Slot { callable, .. } => {
                        for parameter in callable.parameters().parameters() {
                            self.signature(parameter.value_type(), Kind::Signature)?;
                        }
                        self.signature(callable.result(), Kind::Signature)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn inheritance(
        &mut self,
        owner: PersistentTypeId,
        parent: &SignatureTypeKey,
    ) -> Result<(), Error> {
        let SignatureTypeKey::Nominal(parent) = parent else {
            return Err(Error::NonConcreteSignature);
        };
        let (provider, derived) = self.resolve_nominal(owner)?;
        let kind = match self.nominal(*parent)?.kind() {
            crate::PublicNominalKindV1::Class => Kind::ClassBase(derived),
            crate::PublicNominalKindV1::Interface => Kind::Interface(derived),
            _ => return Err(Error::InheritanceEdges(derived)),
        };
        // A dependency's own parent contributes representation support, not
        // a direct inheritance operation committed by the current Cone.
        self.select(
            *parent,
            if provider == self.current.provider {
                kind
            } else {
                Kind::Representation
            },
        )
    }

    pub(super) fn signature(
        &mut self,
        signature: &SignatureTypeKey,
        kind: Kind,
    ) -> Result<(), Error> {
        match signature {
            SignatureTypeKey::Nominal(owner) => self.select(*owner, kind)?,
            SignatureTypeKey::NominalApplication { .. } | SignatureTypeKey::Binder { .. } => {
                return Err(Error::NonConcreteSignature);
            }
            SignatureTypeKey::Tuple(elements) => {
                for element in elements.as_slice() {
                    self.signature(element, kind)?;
                }
            }
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                for parameter in parameters {
                    self.signature(parameter, kind)?;
                }
                self.signature(result, kind)?;
            }
            SignatureTypeKey::RawPointer(pointee) => self.signature(pointee, kind)?,
        }
        Ok(())
    }
}
