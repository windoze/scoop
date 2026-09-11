//! Persistent materialization roots consumed while building LIR metadata.

use std::collections::HashSet;

use scoop_lir as lir;
use scoop_mir as mir;

/// Resolves each MIR exact type to the unique Cone or ODR root that owns its
/// LIR physical entities. `mir_groups` is the earlier-stage delta, used only
/// to distinguish an inherited structural group from one first created here.
pub(crate) struct IdentityRoots<'module> {
    module: &'module mir::Module,
    mir_groups: HashSet<mir::OdrGroupId>,
}

impl<'module> IdentityRoots<'module> {
    pub(crate) fn new(module: &'module mir::Module) -> Self {
        let mir_groups = mir::CanonicalMirFoundation::from_module(module)
            .expect("validated MIR must have a canonical identity foundation")
            .odr_group_ids()
            .collect();
        Self { module, mir_groups }
    }

    pub(crate) fn for_type(&self, ty: &mir::Type) -> lir::MaterializationRoot {
        if let Some(source) = self.module.meta.source_exact_types.get(ty) {
            return match source.owner() {
                mir::SourceExactTypeOwner::ConeOwned => lir::MaterializationRoot::cone_owned(),
                mir::SourceExactTypeOwner::NominalApplication(group) => {
                    lir::MaterializationRoot::prior_stage_odr(group)
                }
                mir::SourceExactTypeOwner::Structural => {
                    self.structural(source.identity_record().id())
                }
            };
        }

        let location = match ty {
            mir::Type::Class(id) => mir::GeneratedExactTypeLocation::Class(*id),
            mir::Type::Enum(id, _) => mir::GeneratedExactTypeLocation::Enum(*id),
            _ => panic!("validated MIR is missing the source exact identity for {ty:?}"),
        };
        self.for_generated(location)
    }

    pub(crate) fn for_generated(
        &self,
        location: mir::GeneratedExactTypeLocation,
    ) -> lir::MaterializationRoot {
        let identity = self
            .module
            .meta
            .generated_exact_types
            .get(location)
            .unwrap_or_else(|| panic!("validated MIR is missing exact identity for {location:?}"));
        match identity.owner() {
            mir::GeneratedExactTypeOwner::ConeOwned => lir::MaterializationRoot::cone_owned(),
            mir::GeneratedExactTypeOwner::OdrOwned(member) => {
                lir::MaterializationRoot::prior_stage_odr(member.key().group())
            }
        }
    }

    pub(crate) fn for_static_storage(
        &self,
        owner: mir::StaticStorageOwner,
    ) -> lir::MaterializationRoot {
        match owner {
            mir::StaticStorageOwner::PropertyBacking(_)
            | mir::StaticStorageOwner::PropertyDelegate(_)
            | mir::StaticStorageOwner::SingletonPublishedRoot(_) => {
                lir::MaterializationRoot::cone_owned()
            }
            mir::StaticStorageOwner::InitializationFailureRoot(unit) => {
                self.initialization_unit(unit)
            }
        }
    }

    pub(crate) fn for_immortal_object(
        &self,
        key: &mir::ImmortalObjectKey,
    ) -> lir::MaterializationRoot {
        match key.owner() {
            mir::ImmortalObjectOwner::Property(_) => lir::MaterializationRoot::cone_owned(),
            mir::ImmortalObjectOwner::InitializationUnit(unit) => self.initialization_unit(unit),
            mir::ImmortalObjectOwner::Callable(materialization) => {
                let identity = self
                    .module
                    .meta
                    .source_callable_materializations
                    .get_by_materialization(materialization)
                    .expect("validated MIR retains an immortal object's callable owner");
                match identity.odr_member_record() {
                    Some(member) => lir::MaterializationRoot::prior_stage_odr(member.key().group()),
                    None => lir::MaterializationRoot::cone_owned(),
                }
            }
        }
    }

    fn initialization_unit(
        &self,
        identity: mir::PersistentInitializationUnitId,
    ) -> lir::MaterializationRoot {
        let unit = self
            .module
            .initialization_units
            .iter()
            .find_map(|(_, unit)| (unit.identity.id() == identity).then_some(unit))
            .expect("validated MIR retains a referenced initialization unit");
        match unit
            .odr_group_id()
            .expect("validated initialization identity must derive its ODR group")
        {
            Some(group) => lir::MaterializationRoot::prior_stage_odr(group),
            None => lir::MaterializationRoot::cone_owned(),
        }
    }

    fn structural(&self, exact_type: mir::PersistentExactTypeId) -> lir::MaterializationRoot {
        let root = lir::MaterializationRoot::lir_structural_odr(exact_type)
            .expect("validated exact type must derive a structural ODR group");
        let group = root
            .odr_group_id()
            .expect("a structural materialization root has an ODR group");
        if self.mir_groups.contains(&group) {
            lir::MaterializationRoot::prior_stage_odr(group)
        } else {
            root
        }
    }
}
