//! Resolve complete query types through existing declarations and type rules.

use super::*;
use scoop_identity::{DecodedSignatureTypeKey, Effect};

mod nominals;
mod references;
pub(super) use references::NominalReferences;

impl FrontendQueries<'_, '_> {
    pub(super) fn query_type_reference(
        &mut self,
        context: hir::SemanticTypeContext<'_>,
        reference: DecodedSignatureTypeKey,
        span: Span,
    ) -> Result<hir::TypeId, Vec<Diagnostic>> {
        let mut references = self
            .nominal_references
            .get_or_init(|| NominalReferences::new(self.lowerer));
        let signature = reference.resolve(&mut references);
        let binders = self.signature_binders(context);
        self.type_query(context, span, |lowerer| {
            let signature = signature
                .map_err(|message| lowerer.error(span, message))
                .ok()?;
            lowerer.resolve_query_signature_type(&signature, &binders, span)
        })
    }
}

impl Lowerer {
    fn resolve_query_signature_type(
        &mut self,
        signature: &SignatureTypeKey,
        binders: &[hir::HirSignatureBinder],
        span: Span,
    ) -> Option<hir::TypeId> {
        use SignatureTypeKey as Key;
        let ty = match signature {
            Key::Nominal(id) => {
                let owner = hir::SourceNominalId::Concrete(*id);
                let result = if self.nominal_owners.contains_key(&owner) {
                    self.apply_nominal_type(owner, Vec::new())
                } else {
                    self.imported_signature_type(signature)
                };
                return result
                    .map_err(|error| {
                        self.error(span, error.diagnostic("queried nominal type"));
                    })
                    .ok();
            }
            Key::NominalApplication { origin, arguments } => {
                let arguments =
                    self.resolve_query_type_list(arguments.as_slice(), binders, span)?;
                return self.apply_query_nominal_type(
                    hir::SourceNominalId::GenericTemplate(*origin),
                    arguments,
                    span,
                );
            }
            Key::Tuple(elements) => {
                let elements = self.resolve_query_type_list(elements.as_slice(), binders, span)?;
                self.intern_type(hir::Type::Tuple(elements))
            }
            Key::Function {
                effect,
                parameters,
                result,
                receiver,
            } => {
                let parameters = self.resolve_query_type_list(parameters, binders, span)?;
                let result = self.resolve_query_signature_type(result, binders, span)?;
                let receiver = match receiver {
                    Some(receiver) => {
                        Some(self.resolve_query_signature_type(receiver, binders, span)?)
                    }
                    None => None,
                };
                let function = self.intern_receiver_function_type(
                    *effect == Effect::Suspend,
                    receiver,
                    parameters,
                    result,
                );
                self.function_types[function].canonical_type
            }
            Key::RawPointer(pointee) => {
                let pointee = self.resolve_query_signature_type(pointee, binders, span)?;
                let declaration = self.query_pointer_declaration(false);
                return self.apply_query_nominal_type(declaration, vec![pointee], span);
            }
            Key::NativeFunctionPointer {
                parameters, result, ..
            } => {
                let parameters = self.resolve_query_type_list(parameters, binders, span)?;
                let result = self.resolve_query_signature_type(result, binders, span)?;
                let function = self.intern_function_type(false, parameters, result);
                let signature = self.function_types[function].canonical_type;
                let declaration = self.query_pointer_declaration(true);
                return self.apply_query_nominal_type(declaration, vec![signature], span);
            }
            Key::Binder { depth, index } => {
                let parameter = binders
                    .iter()
                    .find(|binder| binder.depth == *depth && binder.index == *index);
                let Some(parameter) = parameter else {
                    self.error(
                        span,
                        format!("type binder {depth}:{index} does not exist in the query context"),
                    );
                    return None;
                };
                self.intern_type(hir::Type::Param(parameter.parameter))
            }
            Key::Companion(host) => {
                let host = self.resolve_query_signature_type(host, binders, span)?;
                return self
                    .projected_companion_type(host)
                    .map_err(|message| self.error(span, message))
                    .ok();
            }
        };
        Some(ty)
    }

    fn resolve_query_type_list(
        &mut self,
        types: &[SignatureTypeKey],
        binders: &[hir::HirSignatureBinder],
        span: Span,
    ) -> Option<Vec<hir::TypeId>> {
        types
            .iter()
            .map(|ty| self.resolve_query_signature_type(ty, binders, span))
            .collect()
    }
}
