//! Source visibility replay from explicit, artifact-bound declaration providers.
use crate::*;
use scoop_identity::{
    ConeIdentity, DefinitionOriginSubject as Subject, PersistentGenericTypeId, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationKind,
};
use scoop_wire::{WireError, WirePath};
mod binding;
pub use binding::*;
mod callables;
pub use callables::*;
mod complete;
pub use complete::*;
mod errors;
mod keys;
mod merge;
mod providers;
mod replay;
mod values;
pub use errors::*;
pub use values::*;
type Error = DefaultSourceDomainError;
type Declarations<'s, 'a, 'f> = BoundDefaultSourceAccessDeclarationsV1<'s, 'a, 'f>;

/// A provider registry for raw source domains. It grants no checked lookup,
/// receiver, default profile, or executable capability.
#[derive(Debug)]
pub struct DefaultSourceDomainsV1<'b, 's, 'a, 'f> {
    current: &'b Declarations<'s, 'a, 'f>,
    dependencies: &'b [&'b Declarations<'s, 'a, 'f>],
    core: &'b ImportedCoreFundamentalTypeProtocol,
    any: PersistentTypeId,
}
impl<'b, 's, 'a, 'f> DefaultSourceDomainsV1<'b, 's, 'a, 'f> {
    pub fn new(
        current: &'b Declarations<'s, 'a, 'f>,
        dependencies: &'b [&'b Declarations<'s, 'a, 'f>],
        core: &'b ImportedCoreFundamentalTypeProtocol,
    ) -> Result<Self, Error> {
        providers::validate(current, dependencies)?;

        let any = scoop_identity::CoreBuiltinNominal::Any
            .identity_record()
            .id();
        Ok(Self {
            current,
            dependencies,
            core,
            any,
        })
    }

    /// `scope` must come from the original declaration's checked provider frame
    /// in the complete defaults transaction. Here only its membership is checked.
    pub fn type_source_domain(
        &self,
        ty: &SignatureTypeKey,
        scope: &SignatureBinderScopeV1,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        self.type_source_domain_at(ty, scope, &WirePath::root())
    }

    fn type_source_domain_at(
        &self,
        ty: &SignatureTypeKey,
        scope: &SignatureBinderScopeV1,

        path: &WirePath,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        let mut result = DefaultSourceAccessDomainV1::universal();
        visit_default_source_type_access_demands(ty, path, &mut |demand, path| {
            let part = self.demand_domain(demand, scope)?;
            result = merge::intersect(&result, &part, path)?;
            Ok::<_, Error>(())
        })?;
        Ok(result)
    }
}
fn subject(owner: SourceNominalId) -> Subject {
    match owner {
        SourceNominalId::Concrete(id) => Subject::Type(id),
        SourceNominalId::GenericTemplate(id) => Subject::GenericType(id),
    }
}
