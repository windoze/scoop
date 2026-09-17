use std::fmt;

use scoop_identity::SignatureTypeKey;
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use super::{NominalInterfaceShapeAuthority, SignatureBinderScopeV1, SignatureTypeSemanticError};

impl SignatureBinderScopeV1 {
    /// Validates one signature tree with the artifact's shared resource meter.
    ///
    /// The explicit stack keeps untrusted signature depth off the Rust call
    /// stack. A root has semantic depth one; every visited node and traversed
    /// edge is charged before its semantic work is performed.
    pub fn validate_signature_semantics_metered<A, E>(
        &self,
        signature: &SignatureTypeKey,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), MeteredSignatureTypeSemanticError<E>>
    where
        A: NominalInterfaceShapeAuthority<E>,
    {
        let mut pending = Vec::new();
        meter
            .try_reserve_collection_slots(&mut pending, 1, path)
            .map_err(MeteredSignatureTypeSemanticError::Resource)?;
        pending.push((signature, 1_u64));

        while let Some((signature, depth)) = pending.pop() {
            meter
                .check_semantic_depth(depth, path)
                .map_err(MeteredSignatureTypeSemanticError::Resource)?;
            meter
                .charge_nodes(1, path)
                .map_err(MeteredSignatureTypeSemanticError::Resource)?;
            meter
                .charge_work(1, path)
                .map_err(MeteredSignatureTypeSemanticError::Resource)?;

            match signature {
                SignatureTypeKey::Nominal(declaration) => {
                    let shape = authority
                        .concrete_nominal_shape(*declaration)
                        .map_err(SignatureTypeSemanticError::Reference)
                        .map_err(MeteredSignatureTypeSemanticError::Semantic)?;
                    if shape.type_parameter_arity() != 0 {
                        return Err(MeteredSignatureTypeSemanticError::Semantic(
                            SignatureTypeSemanticError::ConcreteNominalArity {
                                declaration: *declaration,
                                actual: shape.type_parameter_arity(),
                            },
                        ));
                    }
                }
                SignatureTypeKey::NominalApplication { origin, arguments } => {
                    let shape = authority
                        .generic_nominal_shape(*origin)
                        .map_err(SignatureTypeSemanticError::Reference)
                        .map_err(MeteredSignatureTypeSemanticError::Semantic)?;
                    if usize::try_from(shape.type_parameter_arity()).ok()
                        != Some(arguments.as_slice().len())
                    {
                        return Err(MeteredSignatureTypeSemanticError::Semantic(
                            SignatureTypeSemanticError::GenericNominalArity {
                                declaration: *origin,
                                expected: shape.type_parameter_arity(),
                                actual: arguments.as_slice().len(),
                            },
                        ));
                    }
                    push_sequence(&mut pending, arguments.as_slice(), depth, meter, path)?;
                }
                SignatureTypeKey::Tuple(elements) => {
                    push_sequence(&mut pending, elements.as_slice(), depth, meter, path)?;
                }
                SignatureTypeKey::Function {
                    parameters, result, ..
                }
                | SignatureTypeKey::NativeFunctionPointer {
                    parameters, result, ..
                } => {
                    let child_count = parameters.len().checked_add(1).ok_or_else(|| {
                        MeteredSignatureTypeSemanticError::Resource(integer_out_of_range(path))
                    })?;
                    let child_depth =
                        reserve_children(&mut pending, child_count, depth, meter, path)?;
                    pending.push((result.as_ref(), child_depth));
                    for parameter in parameters.iter().rev() {
                        pending.push((parameter, child_depth));
                    }
                }
                SignatureTypeKey::RawPointer(pointee) => {
                    let child_depth = reserve_children(&mut pending, 1, depth, meter, path)?;
                    pending.push((pointee.as_ref(), child_depth));
                }
                SignatureTypeKey::Binder { .. } => self
                    .validate(signature)
                    .map_err(SignatureTypeSemanticError::BinderScope)
                    .map_err(MeteredSignatureTypeSemanticError::Semantic)?,
            }
        }
        Ok(())
    }
}

fn push_sequence<'a, E>(
    pending: &mut Vec<(&'a SignatureTypeKey, u64)>,
    signatures: &'a [SignatureTypeKey],
    parent_depth: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), MeteredSignatureTypeSemanticError<E>> {
    if signatures.is_empty() {
        return Ok(());
    }
    let child_depth = reserve_children(pending, signatures.len(), parent_depth, meter, path)?;
    for signature in signatures.iter().rev() {
        pending.push((signature, child_depth));
    }
    Ok(())
}

fn reserve_children<E>(
    pending: &mut Vec<(&SignatureTypeKey, u64)>,
    count: usize,
    parent_depth: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<u64, MeteredSignatureTypeSemanticError<E>> {
    let child_depth = parent_depth
        .checked_add(1)
        .ok_or_else(|| MeteredSignatureTypeSemanticError::Resource(integer_out_of_range(path)))?;
    meter
        .check_semantic_depth(child_depth, path)
        .map_err(MeteredSignatureTypeSemanticError::Resource)?;
    let edge_count = u64::try_from(count)
        .map_err(|_| MeteredSignatureTypeSemanticError::Resource(integer_out_of_range(path)))?;
    meter
        .charge_edges(edge_count, path)
        .map_err(MeteredSignatureTypeSemanticError::Resource)?;
    meter
        .charge_work(edge_count, path)
        .map_err(MeteredSignatureTypeSemanticError::Resource)?;
    meter
        .try_reserve_collection_slots(pending, count, path)
        .map_err(MeteredSignatureTypeSemanticError::Resource)?;
    Ok(child_depth)
}

fn integer_out_of_range(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}

#[derive(Debug, Eq, PartialEq)]
pub enum MeteredSignatureTypeSemanticError<E> {
    Semantic(SignatureTypeSemanticError<E>),
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for MeteredSignatureTypeSemanticError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Semantic(error) => error.fmt(formatter),
            Self::Resource(error) => {
                write!(formatter, "signature validation resource failure: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for MeteredSignatureTypeSemanticError<E> {}

#[cfg(test)]
mod tests;
