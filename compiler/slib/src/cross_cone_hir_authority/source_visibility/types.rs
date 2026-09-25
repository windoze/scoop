use super::*;
use scoop_hir::{
    DefaultSourceTypeAccessDemandV1 as Demand, SourceAccessDomainV1,
    visit_default_source_type_access_demands,
};
use scoop_identity::CoreBuiltinNominal;
use scoop_wire::WireError;

use crate::cross_cone_hir_authority::CrossConeHirDefaultTypeAccessError as TypeError;

mod pointers;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    /// Binder membership is checked by the complete provider/body type walk
    /// before this query. Type variables contribute no declaration restriction.
    pub(in crate::cross_cone_hir_authority) fn source_type_access_domain(
        &mut self,
        ty: &SignatureTypeKey,
        protocols: &CoreBootstrapInterfaceSectionV1,
        path: &WirePath,
    ) -> Result<SourceAccessDomainV1, TypeError> {
        let mut demands = Vec::new();
        visit_default_source_type_access_demands(ty, path, &mut |demand, path| {
            if !matches!(demand, Demand::Binder { .. }) {
                scoop_wire::allocation::try_reserve(&mut demands, 1, path)?;
                demands.push(demand);
            }
            Ok::<_, WireError>(())
        })
        .map_err(TypeError::Resource)?;
        let mut constraints = Vec::new();
        for demand in demands {
            let (declaration, arity) = match demand {
                Demand::Nominal(id) => (SourceNominalId::Concrete(id), 0),
                Demand::NominalApplication { origin, arguments } => {
                    (SourceNominalId::GenericTemplate(origin), arguments.len())
                }
                Demand::RawPointer { .. } => (self.pointer_source_owner(protocols, false)?, 1),
                Demand::NativeFunctionPointer { .. } => {
                    (self.pointer_source_owner(protocols, true)?, 1)
                }
                Demand::Binder { .. } => continue,
            };
            if self.builtin_type_access(declaration)? {
                continue;
            }
            let (record, key) = self.visibility_nominal(declaration)?;
            let expected = record.type_parameters().len_u32();
            if usize::try_from(expected).ok() != Some(arity) {
                return Err(TypeError::Arity {
                    declaration,
                    expected,
                    actual: arity,
                });
            }
            let part = self.visibility_domain(
                &key,
                nominal_subject(declaration),
                record.declaration_details().declared_visibility(),
            )?;

            scoop_wire::allocation::try_reserve(&mut constraints, part.len(), path)?;
            constraints.extend(part);
        }
        SourceAccessDomainV1::from_constraints(constraints).map_err(TypeError::Resource)
    }

    fn builtin_type_access(&mut self, declaration: SourceNominalId) -> Result<bool, TypeError> {
        let SourceNominalId::Concrete(id) = declaration else {
            return Ok(false);
        };
        for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
            let record = builtin.identity_record();

            if record.id() == id {
                self.provider_interface(record.key().origin())?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}
