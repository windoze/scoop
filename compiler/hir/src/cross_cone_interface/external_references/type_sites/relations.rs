//! Reconstruct the complete foreign nominal fanout at each actual HIR site.

use super::{
    HirDependencyTypePositionV1, HirDependencyTypeSiteV1, HirExpressionTypeRoleV1,
    HirExpressionTypeSiteV1,
};
use crate::concrete::ExecutableExpressionPosition;
use scoop_identity::{ConeIdentity, NominalDeclarationOwner, ValidatedIdentityGraph};
use scoop_wire::WirePath;
use std::collections::BTreeMap;

use crate::CanonicalExternalHirReferencesV1;

mod errors;
mod nominals;
pub use errors::HirDependencyTypeRelationError;
type Error = HirDependencyTypeRelationError;
type Position = HirDependencyTypePositionV1;
type Nominal = (ConeIdentity, NominalDeclarationOwner);

struct Site<'a> {
    source: &'a HirDependencyTypeSiteV1,
    nominals: Vec<Nominal>,
}

struct TypeSiteRelations<'a> {
    current: ConeIdentity,
    identities: &'a ValidatedIdentityGraph,
    dependencies: &'a [ConeIdentity],
}

impl CanonicalExternalHirReferencesV1 {
    pub fn validate_type_site_relations(
        &self,
        current: ConeIdentity,
        identities: &ValidatedIdentityGraph,
        dependencies: &[ConeIdentity],
    ) -> Result<(), Error> {
        let input = TypeSiteRelations {
            current,
            identities,
            dependencies,
        };
        let path = WirePath::root().field(10);
        let mut sites: BTreeMap<Position, Site<'_>> = BTreeMap::new();
        for reference in self.records() {
            if reference.type_sites().is_empty() {
                continue;
            }
            let crate::ExternalHirTargetV1::Nominal(owner) = reference.target() else {
                return Err(Error::Target(reference.target()));
            };
            for source in reference.type_sites().records() {
                let site = sites.entry(source.position()).or_insert_with(|| Site {
                    source,
                    nominals: Vec::new(),
                });

                if site.source != source {
                    return Err(Error::ConflictingPosition(source.position()));
                }

                scoop_wire::allocation::try_reserve(&mut site.nominals, 1, &path)?;
                site.nominals.push((reference.origin(), owner));
            }
        }
        let mut last: Option<&HirExpressionTypeSiteV1> = None;
        for (position, site) in &mut sites {
            if let Some(previous) = last
                && let Some(current) = site.source.as_expression()
                && previous.position() == current.position()
                && (previous.origin() != current.origin()
                    || previous.role() != HirExpressionTypeRoleV1::Value)
            {
                return Err(Error::ConflictingPosition(*position));
            }
            last = site.source.as_expression();
            let expected = input.type_site_nominals(site.source.exact())?;

            site.nominals.sort_unstable();
            if site.nominals != expected {
                return Err(Error::NominalClosure(*position));
            }
        }
        for reference in self.records() {
            for call in reference.call_sites().records() {
                let (roles, exact): (&[_], _) = match call.reason() {
                    crate::HirDependencyCallReasonV1::SourceBinding(_)
                    | crate::HirDependencyCallReasonV1::SourceDeclaration => {
                        (&[HirExpressionTypeRoleV1::Value], call.result())
                    }
                    crate::HirDependencyCallReasonV1::CastFailure { checked_type } => (
                        &[
                            HirExpressionTypeRoleV1::Value,
                            HirExpressionTypeRoleV1::TypeTest,
                        ],
                        *checked_type,
                    ),
                };
                for role in roles {
                    let key = Position::Expression(call.position(), *role);

                    if let Some(site) = sites.get(&key) {
                        if site.source.exact() != exact
                            || site.source.as_expression().map(|source| source.origin())
                                != Some(call.origin())
                        {
                            return Err(Error::CallResult(call.position()));
                        }
                    } else if !input.type_site_nominals(exact)?.is_empty() {
                        return Err(Error::CallResult(call.position()));
                    }
                }
            }
        }
        Ok(())
    }
}
