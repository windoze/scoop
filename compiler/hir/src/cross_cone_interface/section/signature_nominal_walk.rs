use scoop_identity::{NominalDeclarationOwner, SignatureTypeKey};
use scoop_wire::{WireError, WireErrorKind, WirePath};

/// Iteratively visits every nominal leaf in one signature tree.
pub struct SignatureNominalWalker<'signature> {
    pending: Vec<&'signature SignatureTypeKey>,
}

impl<'signature> SignatureNominalWalker<'signature> {
    pub fn new(
        signature: &'signature SignatureTypeKey,

        path: &WirePath,
    ) -> Result<Self, WireError> {
        let mut pending = Vec::new();
        scoop_wire::allocation::try_reserve(&mut pending, 1, path)?;
        pending.push(signature);
        Ok(Self { pending })
    }

    pub fn next(&mut self, path: &WirePath) -> Result<Option<NominalDeclarationOwner>, WireError> {
        while let Some(signature) = self.pending.pop() {
            match signature {
                SignatureTypeKey::Nominal(declaration) => {
                    return Ok(Some(NominalDeclarationOwner::Concrete(*declaration)));
                }
                SignatureTypeKey::NominalApplication { origin, arguments } => {
                    push_children(&mut self.pending, arguments.as_slice(), path)?;
                    return Ok(Some(NominalDeclarationOwner::GenericTemplate(*origin)));
                }
                SignatureTypeKey::Tuple(elements) => {
                    push_children(&mut self.pending, elements.as_slice(), path)?
                }
                SignatureTypeKey::Function {
                    parameters, result, ..
                }
                | SignatureTypeKey::NativeFunctionPointer {
                    parameters, result, ..
                } => {
                    let child_count = parameters
                        .len()
                        .checked_add(1)
                        .ok_or_else(|| integer_out_of_range(path))?;
                    scoop_wire::allocation::try_reserve(&mut self.pending, child_count, path)?;
                    self.pending.push(result.as_ref());
                    for parameter in parameters.iter().rev() {
                        self.pending.push(parameter);
                    }
                }
                SignatureTypeKey::RawPointer(pointee) => {
                    scoop_wire::allocation::try_reserve(&mut self.pending, 1, path)?;
                    self.pending.push(pointee.as_ref());
                }
                SignatureTypeKey::Binder { .. } => {}
            }
        }
        Ok(None)
    }
}

fn push_children<'signature>(
    pending: &mut Vec<&'signature SignatureTypeKey>,
    children: &'signature [SignatureTypeKey],
    path: &WirePath,
) -> Result<(), WireError> {
    if children.is_empty() {
        return Ok(());
    }
    scoop_wire::allocation::try_reserve(pending, children.len(), path)?;
    for child in children.iter().rev() {
        pending.push(child);
    }
    Ok(())
}

fn integer_out_of_range(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
