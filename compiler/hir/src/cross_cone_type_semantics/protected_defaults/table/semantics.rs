//! Complete default-template proofs, separate from consumer winner selection.
use super::CanonicalProtectedDefaultTemplatesV1;
use crate::{
    CheckedNominalInheritanceGraphV1, CheckedNominalInheritanceInterfacesV1,
    CheckedProtectedSourceInterfacesV1, NominalInterfaceShapeAuthority,
    NominalSupportCallableSemanticAuthority, ProtectedDefaultLocalDataFlowSemanticAuthority,
    ProtectedDefaultNestedCallableSemanticAuthority,
    ProtectedDefaultOperationTypingSemanticAuthority, ProtectedDefaultOriginSemanticAuthority,
    ProtectedDefaultReferenceAccessSemanticAuthority, ProtectedDefaultRootSemanticAuthority,
};
use scoop_wire::WirePath;

mod checked;
mod errors;
mod template;
pub use checked::*;
pub use errors::*;

pub trait ProtectedDefaultSemanticAuthority<E>:
    NominalInterfaceShapeAuthority<E>
    + crate::DefaultLocalFunctionSignatureAuthority<E>
    + ProtectedDefaultRootSemanticAuthority<E>
    + ProtectedDefaultOriginSemanticAuthority<E>
    + ProtectedDefaultReferenceAccessSemanticAuthority<E>
    + ProtectedDefaultLocalDataFlowSemanticAuthority<E>
    + ProtectedDefaultOperationTypingSemanticAuthority<E>
    + ProtectedDefaultNestedCallableSemanticAuthority<E>
{
}
impl<A, E> ProtectedDefaultSemanticAuthority<E> for A where
    A: NominalInterfaceShapeAuthority<E>
        + crate::DefaultLocalFunctionSignatureAuthority<E>
        + ProtectedDefaultRootSemanticAuthority<E>
        + ProtectedDefaultOriginSemanticAuthority<E>
        + ProtectedDefaultReferenceAccessSemanticAuthority<E>
        + ProtectedDefaultLocalDataFlowSemanticAuthority<E>
        + ProtectedDefaultOperationTypingSemanticAuthority<E>
        + ProtectedDefaultNestedCallableSemanticAuthority<E>
{
}

impl CanonicalProtectedDefaultTemplatesV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn validate_semantics<'a, S, A, E>(
        &'a self,
        sources: &CheckedProtectedSourceInterfacesV1<'a>,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        inheritance: CheckedNominalInheritanceInterfacesV1<'_>,
        source_authority: &mut S,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<CheckedProtectedDefaultTemplatesV1<'a>, ProtectedDefaultTableSemanticError<E>>
    where
        S: NominalSupportCallableSemanticAuthority<E>,
        A: ProtectedDefaultSemanticAuthority<E>,
    {
        use ProtectedDefaultTableSemanticError as Error;
        sources
            .table()
            .validate_default_closure(self.keys())
            .map_err(Error::SourceClosure)?;
        let mut records = Vec::new();

        scoop_wire::allocation::try_reserve(&mut records, self.records().len(), path)
            .map_err(Error::Resource)?;
        for (index, template) in self.records().iter().enumerate() {
            let key = template.key();

            let source = sources
                .get(key.owner())
                .ok_or(Error::MissingSource { index, key })?;
            let owner = source
                .validate_owner_source(graph, source_authority)
                .map_err(|error| Error::Source {
                    index,
                    key,
                    error: Box::new(error),
                })?;
            let profile = template::validate(
                template,
                owner,
                source.protocol(),
                graph,
                inheritance,
                authority,
                path,
            )
            .map_err(|error| Error::Template {
                index,
                key,
                error: Box::new(error),
            })?;
            records.push(checked::checked(template, source, profile));
        }
        Ok(CheckedProtectedDefaultTemplatesV1 {
            table: self,
            records,
        })
    }
}

#[cfg(test)]
mod tests;
