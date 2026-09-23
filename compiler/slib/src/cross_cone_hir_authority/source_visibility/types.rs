use super::*;
use scoop_hir::{
    DefaultSourceTypeAccessDemandV1 as Demand, DefaultSourceTypeAccessVisitError,
    SourceAccessDomainV1, visit_default_source_type_access_demands,
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
        visit_default_source_type_access_demands(
            ty,
            self.meter,
            path,
            &mut |demand, meter, path| {
                if !matches!(demand, Demand::Binder { .. }) {
                    meter.try_reserve_collection_slots(&mut demands, 1, path)?;
                    demands.push(demand);
                }
                Ok::<_, WireError>(())
            },
        )
        .map_err(|error| match error {
            DefaultSourceTypeAccessVisitError::Resource(error)
            | DefaultSourceTypeAccessVisitError::Visitor(error) => TypeError::Resource(error),
        })?;
        let mut constraints = Vec::new();
        for demand in demands {
            let (declaration, arity) = match demand {
                Demand::Nominal(id) => (SourceNominalId::Concrete(id), 0),
                Demand::NominalApplication { origin, arguments } => {
                    (SourceNominalId::GenericTemplate(origin), arguments.len())
                }
                Demand::RawPointer { .. } => {
                    (self.pointer_source_owner(protocols, false, path)?, 1)
                }
                Demand::NativeFunctionPointer { .. } => {
                    (self.pointer_source_owner(protocols, true, path)?, 1)
                }
                Demand::Binder { .. } => continue,
            };
            if self.builtin_type_access(declaration, path)? {
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
            self.meter
                .check_table_entries(constraints.len() as u64 + part.len() as u64, path)?;
            self.meter
                .try_reserve_collection_slots(&mut constraints, part.len(), path)?;
            constraints.extend(part);
        }
        SourceAccessDomainV1::from_constraints(constraints, self.meter, path)
            .map_err(TypeError::Resource)
    }

    fn builtin_type_access(
        &mut self,
        declaration: SourceNominalId,
        path: &WirePath,
    ) -> Result<bool, TypeError> {
        let SourceNominalId::Concrete(id) = declaration else {
            return Ok(false);
        };
        for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
            let record = builtin.identity_record();
            let bytes = scoop_wire::encoded_length(record.key())
                .map_err(|error| TypeError::Encoding(error.to_string()))?;
            self.meter.charge_sha256(bytes, path)?;
            self.meter.charge_work(bytes.saturating_add(65), path)?;
            if record.id() == id {
                self.visibility_work(self.dependencies.len() as u64 + 1)?;
                self.provider_interface(record.key().origin())?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}
