use super::*;

impl Graph<'_> {
    pub(super) fn close(&mut self, meter: &mut BudgetMeter) -> Result<(), Error> {
        let path = WirePath::root().field(8);
        while let Some(owner) = self.pending.pop() {
            meter.charge_nodes(1, &path)?;
            meter.charge_work(1 + u64::from(self.requirements.len().max(1).ilog2()), &path)?;
            // Each owner is expanded once. The original declaration references
            // retain every semantic edge, including repeated type positions.
            for requirement in self.requirements.remove(&owner).unwrap_or_default() {
                use NominalMaterializationRequirementV1 as Requirement;
                match requirement {
                    Requirement::Field { field, .. } => {
                        self.signature(field.value_type(), Kind::Representation, 1, meter)?;
                    }
                    Requirement::EnumVariantField { field, .. } => {
                        self.signature(field.value_type(), Kind::Representation, 1, meter)?;
                    }
                    Requirement::Inheritance { owner, parent } => {
                        self.inheritance(owner, parent, meter)?;
                    }
                    Requirement::Constructor { callable, .. }
                    | Requirement::Slot { callable, .. } => {
                        for parameter in callable.parameters().parameters() {
                            self.signature(parameter.value_type(), Kind::Signature, 1, meter)?;
                        }
                        self.signature(callable.result(), Kind::Signature, 1, meter)?;
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
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let path = WirePath::root().field(8);
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        let SignatureTypeKey::Nominal(parent) = parent else {
            return Err(Error::NonConcreteSignature);
        };
        let (provider, derived) = self.resolve_nominal(owner, meter)?;
        let kind = match self.nominal(*parent, meter)?.kind() {
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
            meter,
        )
    }

    pub(super) fn signature(
        &mut self,
        signature: &SignatureTypeKey,
        kind: Kind,
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let path = WirePath::root().field(8);
        meter.check_semantic_depth(depth, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        match signature {
            SignatureTypeKey::Nominal(owner) => self.select(*owner, kind, meter)?,
            SignatureTypeKey::NominalApplication { .. } | SignatureTypeKey::Binder { .. } => {
                return Err(Error::NonConcreteSignature);
            }
            SignatureTypeKey::Tuple(elements) => {
                for element in elements.as_slice() {
                    self.signature(element, kind, depth + 1, meter)?;
                }
            }
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                for parameter in parameters {
                    self.signature(parameter, kind, depth + 1, meter)?;
                }
                self.signature(result, kind, depth + 1, meter)?;
            }
            SignatureTypeKey::RawPointer(pointee) => {
                self.signature(pointee, kind, depth + 1, meter)?
            }
        }
        Ok(())
    }
}
