//! Source representation exports from the actual HIR/MIR production pair.

use scoop_hir as hir;
use scoop_identity::{
    ExactTypeKey, PersistentExactTypeId, PersistentTypeId, ValidatedIdentityGraph,
};
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

mod builtins;
mod representation;
mod resources;
use resources::{reserve, work};

/// Combines actual source representations and the sealed finite shape plan.
/// Both projections and their final canonicalization share the caller's budget.
pub fn lower_type_exports(
    hir: &hir::CrossConeTypeSemanticsProductionV1,
    input: &mir::SingleConeStrongMirInput,
    identities: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<mir::CanonicalParamFreeMirTypeExportsV1, SourceMirTypeProductionError> {
    let mut records = lower_source_type_exports(hir, input, identities, meter)?.into_records();
    let finite = mir::CanonicalParamFreeMirTypeExportsV1::from_finite_shape_support(
        input, identities, meter,
    )?;
    reserve(&mut records, finite.records().len(), meter)?;
    records.extend(finite.into_records());
    resources::sort(records.len(), meter)?;
    Ok(mir::CanonicalParamFreeMirTypeExportsV1::try_new(records)?)
}

/// Produces the source and object-backing constituent of the M23-6 type table.
/// The complete section still requires callable, dispatch and shape products.
pub fn lower_source_type_exports(
    hir: &hir::CrossConeTypeSemanticsProductionV1,
    input: &mir::SingleConeStrongMirInput,
    identities: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<mir::CanonicalParamFreeMirTypeExportsV1, SourceMirTypeProductionError> {
    let module = input.module();
    let source = hir.section().representation_support();
    let mut records = Vec::new();
    reserve(
        &mut records,
        source.records().len().saturating_mul(2),
        meter,
    )?;
    let mut produced = 0;
    let authority = mir::MirTypeBridgeAuthority {
        identities,
        foundation: input.foundation(),
    };
    for identity in module.meta.source_exact_types.iter() {
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(SourceMirTypeProductionError::Resource)?;
        work(
            source.records().len().checked_ilog2().unwrap_or(0) as usize + 2,
            meter,
        )?;
        let ExactTypeKey::Nominal(owner) = identity.identity_record().key() else {
            continue;
        };
        let Some(representation) = source.get(*owner) else {
            if let Some(record) =
                builtins::project(hir, input, identities, identity, *owner, meter)?
            {
                reserve(&mut records, 1, meter)?;
                records.push(record);
            }
            continue;
        };
        let exact = identity.identity_record().id();
        let fact = hir
            .section()
            .exact_facts()
            .get(exact)
            .ok_or(SourceMirTypeProductionError::MissingFacts(exact))?;
        let facts = facts(fact)?;
        let inheritance = bases(hir, exact, meter)?;
        let (shape, backing) =
            representation::project(module, identity.ty(), representation, meter)?;
        records.push(mir::ParamFreeMirTypeExportV1::try_new(
            authority,
            exact,
            mir::MirTypeOriginV1::SourceNominal(*owner),
            facts,
            shape,
            inheritance,
        )?);
        if let Some((nominal, backing_exact, shape)) = backing {
            records.push(mir::ParamFreeMirTypeExportV1::try_new(
                authority,
                backing_exact,
                mir::MirTypeOriginV1::GeneratedNominal {
                    nominal,
                    role: scoop_identity::GeneratedNominalKey::ObjectBackingClass {
                        object: *owner,
                    },
                },
                facts,
                shape,
                bases(hir, exact, meter)?,
            )?);
        }
        produced += 1;
    }
    if produced != source.records().len() {
        return Err(SourceMirTypeProductionError::IncompleteSurface {
            expected: source.records().len(),
            actual: produced,
        });
    }
    resources::sort(records.len(), meter)?;
    Ok(mir::CanonicalParamFreeMirTypeExportsV1::try_new(records)?)
}

fn facts(
    source: &hir::ExactTypeFactsV1,
) -> Result<mir::MirTypeFactsV1, SourceMirTypeProductionError> {
    let kind = match source.kind() {
        hir::ExactTypeKindV1::Reference => mir::MirValueKindV1::Reference,
        hir::ExactTypeKindV1::Value {
            zst: hir::ZstStatus::ZeroSized,
        } => mir::MirValueKindV1::ZeroSizedValue,
        hir::ExactTypeKindV1::Value {
            zst: hir::ZstStatus::NonZero,
        } => mir::MirValueKindV1::NonZeroValue,
    };
    Ok(mir::MirTypeFactsV1::try_new(
        kind,
        match source.gc() {
            hir::ExactTypeGcV1::GcFree => mir::MirGcKindV1::GcFree,
            hir::ExactTypeGcV1::ContainsManagedReferences => {
                mir::MirGcKindV1::ContainsManagedReferences
            }
        },
    )?)
}

fn bases(
    hir: &hir::CrossConeTypeSemanticsProductionV1,
    exact: PersistentExactTypeId,
    meter: &mut BudgetMeter,
) -> Result<mir::MirBaseAndInterfacesV1, SourceMirTypeProductionError> {
    let edges = hir.local_inheritance_edges();
    work(edges.len().checked_ilog2().unwrap_or(0) as usize + 1, meter)?;
    let index = edges
        .binary_search_by_key(&exact, hir::NominalInheritanceEdgesV1::owner)
        .map_err(|_| SourceMirTypeProductionError::MissingInheritance(exact))?;
    let source = &edges[index];
    let mut interfaces = Vec::new();
    reserve(&mut interfaces, source.direct_interfaces().len(), meter)?;
    interfaces.extend_from_slice(source.direct_interfaces());
    Ok(mir::MirBaseAndInterfacesV1 {
        base: match source.direct_base() {
            hir::DirectClassBaseV1::NoClassBase => mir::MirBaseClassV1::None,
            hir::DirectClassBaseV1::ClassBase { exact } => mir::MirBaseClassV1::Base(exact),
        },
        interfaces,
    })
}

#[derive(Debug)]
pub enum SourceMirTypeProductionError {
    Resource(scoop_wire::WireError),
    Bridge(mir::MirTypeBridgeError),
    MissingFacts(PersistentExactTypeId),
    MissingInheritance(PersistentExactTypeId),
    MissingFieldType,
    RepresentationMismatch(PersistentTypeId),
    IncompleteSurface { expected: usize, actual: usize },
    Identity(String),
}
impl From<mir::MirTypeBridgeError> for SourceMirTypeProductionError {
    fn from(error: mir::MirTypeBridgeError) -> Self {
        Self::Bridge(error)
    }
}
impl std::fmt::Display for SourceMirTypeProductionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot produce source MIR type exports: {self:?}")
    }
}
impl std::error::Error for SourceMirTypeProductionError {}
