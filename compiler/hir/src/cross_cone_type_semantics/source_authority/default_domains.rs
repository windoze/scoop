//! Source visibility replay from explicit, artifact-bound declaration providers.
use crate::*;
use scoop_identity::{
    ConeIdentity, DefinitionOriginSubject as Subject, PersistentGenericTypeId, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationKind,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};
mod binding;
pub use binding::*;
mod errors;
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
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        providers::validate(current, dependencies, meter)?;
        meter.charge_owned_bytes(128, &path)?;
        meter.charge_sha256(256, &path)?;
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
        meter: &mut BudgetMeter,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        self.type_source_domain_at(ty, scope, meter, &WirePath::root())
    }

    fn type_source_domain_at(
        &self,
        ty: &SignatureTypeKey,
        scope: &SignatureBinderScopeV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        let mut result = DefaultSourceAccessDomainV1::universal();
        visit_default_source_type_access_demands(ty, meter, path, &mut |demand, meter, path| {
            let part = self.demand_domain(demand, scope, meter, path)?;
            result = merge::intersect(&result, &part, meter, path)?;
            Ok::<_, Error>(())
        })
        .map_err(|error| match error {
            DefaultSourceTypeAccessVisitError::Resource(error) => Error::Resource(error),
            DefaultSourceTypeAccessVisitError::Visitor(error) => error,
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
