//! Complete dependency interface signatures in provider declaration order.

use super::*;
use hir::ImportedCallableSource;
use std::sync::Arc;

impl Lowerer {
    pub(super) fn imported_interface_type(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        let hir::NominalDispatchOrderV1::Interface { parents, members } =
            declaration.interface.declaration_details().dispatch_order()
        else {
            return Err(ImportedSignatureTypeError::Structural);
        };
        let mut interface = hir::ImportedInterfaceType {
            declaration: Arc::clone(&declaration),
            parents: Vec::new(),
            methods: Vec::new(),
        };
        // Method signatures may refer back to this interface. Finish the record
        // before returning the complete HIR product.
        let ty = self.intern_type(hir::Type::ImportedInterface(Arc::new(interface.clone())));
        for parent in parents {
            let parent_ty = self.imported_signature_type(parent)?;
            let hir::Type::ImportedInterface(parent) = &self.types[parent_ty] else {
                return Err(ImportedSignatureTypeError::Structural);
            };
            for method in &parent.methods {
                if !interface
                    .methods
                    .iter()
                    .any(|existing| existing.slot.id() == method.slot.id())
                {
                    interface.methods.push(method.clone());
                }
            }
            interface.parents.push(parent_ty);
        }
        for member in members {
            interface
                .methods
                .retain(|method| !member.overrides().values().contains(&method.slot.id()));
            let candidate = self
                .dependencies
                .as_ref()
                .and_then(|dependencies| {
                    dependencies
                        .callable_for_slot(
                            hir::SourceNominalId::Concrete(declaration.identity.id()),
                            member.slot(),
                        )
                        .ok()
                        .flatten()
                })
                .ok_or(ImportedSignatureTypeError::Structural)?;
            let callable = candidate.interface();
            let source = candidate
                .source_interface()
                .ok_or(ImportedSignatureTypeError::Structural)?;
            let parameters = callable
                .parameters()
                .parameters()
                .iter()
                .zip(source.parameters().parameters())
                .map(|(parameter, source)| {
                    Ok((
                        source.name().as_str().to_owned(),
                        self.imported_signature_type(parameter.value_type())?,
                    ))
                })
                .collect::<Result<Vec<_>, ImportedSignatureTypeError>>()?;
            let return_type = self.imported_signature_type(callable.result())?;
            interface.methods.push(hir::ImportedInterfaceMethod {
                slot: declaration
                    .interface_slots
                    .iter()
                    .find(|slot| slot.id() == member.slot())
                    .ok_or(ImportedSignatureTypeError::Structural)?
                    .clone(),
                overrides: member.overrides().values().to_vec(),
                declaration: callable.clone(),
                name: candidate.name().to_owned(),
                parameters,
                return_type,
            });
        }
        let suppressed = interface
            .methods
            .iter()
            .flat_map(|method| &method.overrides)
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        interface
            .methods
            .retain(|method| !suppressed.contains(&method.slot.id()));
        self.types[ty] = hir::Type::ImportedInterface(Arc::new(interface));
        Ok(ty)
    }
}
