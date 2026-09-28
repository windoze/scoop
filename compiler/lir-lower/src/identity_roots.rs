//! Persistent materialization roots consumed while building LIR.

use scoop_lir as lir;
use scoop_mir as mir;
use std::collections::HashMap;

/// Resolves actual source, generated and structural layout roots. Source
/// applications and existing structural groups retain their MIR ownership.
pub(crate) struct IdentityRoots<'input> {
    input: &'input mir::ConeMirInput,
    immortal_owners: HashMap<mir::ImmortalObjectOwner, lir::MaterializationRoot>,
}

impl<'input> IdentityRoots<'input> {
    pub(crate) fn new(input: &'input mir::ConeMirInput) -> Self {
        let materializations = &input.module().meta.source_callable_materializations;
        let root = |record: &mir::SourceCallableMaterialization| {
            record
                .odr_member_record()
                .map_or_else(lir::MaterializationRoot::cone_owned, |member| {
                    lir::MaterializationRoot::prior_stage_odr(member.key().group())
                })
        };
        let mut immortal_owners = materializations
            .iter()
            .map(|record| {
                (
                    mir::ImmortalObjectOwner::Callable(record.materialization()),
                    root(record),
                )
            })
            .collect::<HashMap<_, _>>();
        for (_, unit) in input.module().initialization_units.iter() {
            let initializer = input
                .materialization()
                .callable_roots()
                .iter()
                .find(|root| root.function() == unit.initializer)
                .expect("an initialization unit retains its callable materialization root");
            let root = match initializer.subject() {
                mir::CallableSignatureSubject::Strong(_) => lir::MaterializationRoot::cone_owned(),
                mir::CallableSignatureSubject::Odr(member) => {
                    lir::MaterializationRoot::prior_stage_odr(member.group())
                }
            };
            immortal_owners.insert(
                mir::ImmortalObjectOwner::InitializationUnit(unit.identity.id()),
                root,
            );
        }
        Self {
            input,
            immortal_owners,
        }
    }

    pub(crate) fn materializes_type(&self, ty: &mir::Type) -> bool {
        if self
            .input
            .materialization()
            .source_nominal_shape(ty)
            .is_some()
        {
            return true;
        }
        generated_location(ty).is_some_and(|location| {
            self.input
                .materialization()
                .generated_nominal_shape(location)
                .is_some()
        })
    }

    pub(crate) fn source_nominal_shapes(&self) -> &[mir::SourceNominalShapeRoot] {
        self.input.materialization().source_nominal_shapes()
    }

    pub(crate) fn generated_nominal_shapes(&self) -> &[mir::GeneratedNominalShapeRoot] {
        self.input.materialization().generated_nominal_shapes()
    }

    pub(crate) fn for_type(&self, ty: &mir::Type) -> lir::MaterializationRoot {
        if let Some(mir::SourceNominalShapeRoot::Application { group, .. }) =
            self.input.materialization().source_nominal_shape(ty)
        {
            return lir::MaterializationRoot::prior_stage_odr(*group);
        }
        if let Some(location) = generated_location(ty)
            && self
                .input
                .materialization()
                .generated_nominal_shape(location)
                .is_some()
        {
            return self.for_generated(location);
        }
        if let Some(source) = self.input.module().meta.source_exact_types.get(ty)
            && source.owner() == mir::SourceExactTypeOwner::Structural
        {
            let root = lir::MaterializationRoot::lir_structural_odr(source.identity_record().id())
                .expect("a validated structural exact type derives its layout group");
            let group = root
                .odr_group_id()
                .expect("a structural root has an ODR group");
            return if self
                .input
                .foundation()
                .odr_group_ids()
                .any(|id| id == group)
            {
                lir::MaterializationRoot::prior_stage_odr(group)
            } else {
                root
            };
        }
        assert!(
            self.materializes_type(ty),
            "the MIR plan does not materialize {ty:?}"
        );
        lir::MaterializationRoot::cone_owned()
    }

    pub(crate) fn for_generated(
        &self,
        location: mir::GeneratedExactTypeLocation,
    ) -> lir::MaterializationRoot {
        match self
            .input
            .materialization()
            .generated_nominal_shape(location)
            .expect("the MIR plan materializes this generated type")
        {
            mir::GeneratedNominalShapeRoot::Cone(_) => lir::MaterializationRoot::cone_owned(),
            mir::GeneratedNominalShapeRoot::Odr { group, .. } => {
                lir::MaterializationRoot::prior_stage_odr(group)
            }
        }
    }

    pub(crate) const fn for_static_storage(
        &self,
        _owner: mir::StaticStorageOwner,
    ) -> lir::MaterializationRoot {
        lir::MaterializationRoot::cone_owned()
    }

    pub(crate) fn for_immortal_object(
        &self,
        key: &mir::ImmortalObjectKey,
    ) -> lir::MaterializationRoot {
        match key.owner() {
            mir::ImmortalObjectOwner::Property(_) => lir::MaterializationRoot::cone_owned(),
            owner @ (mir::ImmortalObjectOwner::Callable(_)
            | mir::ImmortalObjectOwner::InitializationUnit(_)) => {
                self.immortal_owners[&owner].clone()
            }
        }
    }
}

fn generated_location(ty: &mir::Type) -> Option<mir::GeneratedExactTypeLocation> {
    match ty {
        mir::Type::Class(id) => Some(mir::GeneratedExactTypeLocation::Class(*id)),
        mir::Type::Enum(id, _) => Some(mir::GeneratedExactTypeLocation::Enum(*id)),
        mir::Type::Unit
        | mir::Type::Integer(_)
        | mir::Type::MachineScalar(_)
        | mir::Type::Boolean
        | mir::Type::String
        | mir::Type::Struct(_)
        | mir::Type::Interface(_)
        | mir::Type::Tuple(_)
        | mir::Type::Function(_)
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_)
        | mir::Type::Any => None,
    }
}
