//! Reconstruct the complete foreign nominal fanout at each actual expression.

use super::{HirDependencyTypeSiteV1, HirExpressionTypeRoleV1};
use crate::concrete::ExecutableExpressionPosition;
use scoop_identity::{ConeIdentity, NominalDeclarationOwner, ValidatedIdentityGraph};
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeMap;

use crate::CanonicalExternalHirReferencesV1;

mod errors;
mod nominals;
pub use errors::HirDependencyTypeRelationError;
type Error = HirDependencyTypeRelationError;
type Position = (ExecutableExpressionPosition, HirExpressionTypeRoleV1);
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
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let input = TypeSiteRelations {
            current,
            identities,
            dependencies,
        };
        let path = WirePath::root().field(10);
        let mut sites: BTreeMap<Position, Site<'_>> = BTreeMap::new();
        for reference in self.records() {
            meter.charge_work(1, &path)?;
            if reference.type_sites().is_empty() {
                continue;
            }
            let crate::ExternalHirTargetV1::Nominal(owner) = reference.target() else {
                return Err(Error::Target(reference.target()));
            };
            for source in reference.type_sites().records() {
                meter.charge_work(1 + u64::from(sites.len().max(1).ilog2()), &path)?;
                if !sites.contains_key(&source.sort_key()) {
                    meter.charge_owned_bytes(
                        (std::mem::size_of::<(Position, Site<'_>)>() + 32) as u64,
                        &path,
                    )?;
                    meter.charge_collection_slots(1, &path)?;
                }
                let site = sites.entry(source.sort_key()).or_insert_with(|| Site {
                    source,
                    nominals: Vec::new(),
                });
                meter.charge_work(
                    scoop_wire::encoded_length(source)
                        .map_err(|_| Error::Encoding)?
                        .saturating_mul(3),
                    &path,
                )?;
                if site.source != source {
                    return Err(Error::ConflictingPosition(source.sort_key()));
                }
                meter.charge_owned_bytes(std::mem::size_of::<Nominal>() as u64, &path)?;
                meter.try_reserve_collection_slots(&mut site.nominals, 1, &path)?;
                site.nominals.push((reference.origin(), owner));
            }
        }
        let mut last: Option<&HirDependencyTypeSiteV1> = None;
        for (position, site) in &mut sites {
            if let Some(previous) = last
                && previous.position() == site.source.position()
                && (previous.origin() != site.source.origin()
                    || previous.role() != HirExpressionTypeRoleV1::Value)
            {
                return Err(Error::ConflictingPosition(*position));
            }
            last = Some(site.source);
            let expected = input.type_site_nominals(site.source.exact(), meter)?;
            let count = site.nominals.len() as u64;
            meter.charge_work(
                count.saturating_mul(3 + u64::from(count.max(1).ilog2())),
                &path,
            )?;
            site.nominals.sort_unstable();
            if site.nominals != expected {
                return Err(Error::NominalClosure(*position));
            }
        }
        for reference in self.records() {
            meter.charge_work(1, &path)?;
            for call in reference.call_sites().records() {
                let key = (call.position(), HirExpressionTypeRoleV1::Value);
                meter.charge_work(1 + u64::from(sites.len().max(1).ilog2()), &path)?;
                if let Some(site) = sites.get(&key) {
                    meter.charge_work(
                        scoop_wire::encoded_length(call).map_err(|_| Error::Encoding)?,
                        &path,
                    )?;
                    if site.source.exact() != call.result() || site.source.origin() != call.origin()
                    {
                        return Err(Error::CallResult(call.position()));
                    }
                } else if !input.type_site_nominals(call.result(), meter)?.is_empty() {
                    return Err(Error::CallResult(call.position()));
                }
            }
        }
        Ok(())
    }
}
