//! Substitute the declared element condition using ordinary source inheritance.

use super::*;
use scoop_identity::{CoreBuiltinNominal, SignatureTypeKey};
use std::collections::BTreeSet;

mod tuples;

impl<'a> SharedTypeMetadataV1<'a> {
    pub(crate) fn applied_encoding_parent(
        self,
        application: &AppliedNominal<'a>,
        dependencies: &[Self],
    ) -> Result<Option<PersistentExactTypeId>, Error> {
        let Some(encoding) = application
            .declaration
            .declaration_details()
            .element_encoding()
        else {
            return Ok(None);
        };
        let SignatureTypeKey::Nominal(interface) = encoding.interface() else {
            return Err(Error::NonConcreteSignature);
        };
        let element = self.signature_exact_type_with_bindings(
            encoding.element(),
            &application.bindings(),
            self.identities,
        )?;
        if !self.element_has_encoding(
            element,
            SourceNominalId::Concrete(*interface),
            dependencies,
        )? {
            return Ok(None);
        }
        self.signature_exact_type(encoding.interface()).map(Some)
    }

    fn element_has_encoding(
        self,
        exact: PersistentExactTypeId,
        interface: SourceNominalId,
        dependencies: &[Self],
    ) -> Result<bool, Error> {
        let key = self.identities.canonical_key::<_, ExactTypeKey>(exact)?;
        match key.as_ref() {
            ExactTypeKey::Tuple(elements) => {
                for element in elements.as_slice() {
                    if !self.element_has_encoding(*element, interface, dependencies)? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            ExactTypeKey::Nominal(owner)
                if *owner == CoreBuiltinNominal::Any.identity_record().id() =>
            {
                return Ok(false);
            }
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {}
            _ => return Ok(false),
        }
        let application = self.applied_nominal(exact, dependencies.iter().copied())?;
        if self.source_inherits_encoding(
            application.declaration.declaration(),
            interface,
            dependencies,
        )? {
            return Ok(true);
        }
        let Some(encoding) = application
            .declaration
            .declaration_details()
            .element_encoding()
        else {
            return Ok(false);
        };
        if !matches!(encoding.interface(), SignatureTypeKey::Nominal(owner)
            if SourceNominalId::Concrete(*owner) == interface)
        {
            return Ok(false);
        }
        let element = self.signature_exact_type_with_bindings(
            encoding.element(),
            &application.bindings(),
            self.identities,
        )?;
        self.element_has_encoding(element, interface, dependencies)
    }

    fn source_inherits_encoding(
        self,
        source: SourceNominalId,
        interface: SourceNominalId,
        dependencies: &[Self],
    ) -> Result<bool, Error> {
        let mut pending = vec![source];
        let mut seen = BTreeSet::new();
        while let Some(owner) = pending.pop() {
            if owner == interface {
                return Ok(true);
            }
            if !seen.insert(owner) {
                continue;
            }
            let declaration = std::iter::once(self)
                .chain(dependencies.iter().copied())
                .find_map(|metadata| metadata.public.nominal_interfaces().declaration(owner))
                .ok_or(Error::InheritanceSource(owner))?;
            for parent in declaration.exact_supertypes().values() {
                pending.push(match parent {
                    SignatureTypeKey::Nominal(owner) => SourceNominalId::Concrete(*owner),
                    SignatureTypeKey::NominalApplication { origin, .. } => {
                        SourceNominalId::GenericTemplate(*origin)
                    }
                    _ => return Err(Error::NonConcreteSignature),
                });
            }
        }
        Ok(false)
    }
}

impl MetadataTypes<'_, '_> {
    pub(crate) fn encoding_parent(
        self,
        owner: PersistentExactTypeId,
    ) -> Result<Option<PersistentExactTypeId>, Error> {
        let application = self.applied_nominal(owner)?;
        SharedTypeMetadataV1 {
            identities: self.identity_graph(owner),
            ..self.current
        }
        .applied_encoding_parent(
            &application,
            &self
                .dependencies
                .iter()
                .map(|value| value.metadata)
                .collect::<Vec<_>>(),
        )
    }
}
