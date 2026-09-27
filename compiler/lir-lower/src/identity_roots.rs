//! Persistent materialization roots consumed while building LIR.

use scoop_lir as lir;
use scoop_mir as mir;

/// Resolves source and generated nominal roots from the MIR materialization
/// plan. Source applications inherit their original specialization group.
pub(crate) struct IdentityRoots<'input> {
    input: &'input mir::ConeMirInput,
}

impl<'input> IdentityRoots<'input> {
    pub(crate) const fn new(input: &'input mir::ConeMirInput) -> Self {
        Self { input }
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

    pub(crate) fn generated_nominal_shapes(&self) -> &[mir::StrongGeneratedNominalShapeRoot] {
        self.input.materialization().generated_nominal_shapes()
    }

    pub(crate) fn for_type(&self, ty: &mir::Type) -> lir::MaterializationRoot {
        if let Some(mir::SourceNominalShapeRoot::Application { group, .. }) =
            self.input.materialization().source_nominal_shape(ty)
        {
            return lir::MaterializationRoot::prior_stage_odr(*group);
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
        assert!(
            self.input
                .materialization()
                .generated_nominal_shape(location)
                .is_some(),
            "the sealed strong plan does not materialize {location:?}"
        );
        lir::MaterializationRoot::cone_owned()
    }

    pub(crate) const fn for_static_storage(
        &self,
        _owner: mir::StaticStorageOwner,
    ) -> lir::MaterializationRoot {
        lir::MaterializationRoot::cone_owned()
    }

    pub(crate) const fn for_immortal_object(
        &self,
        _key: &mir::ImmortalObjectKey,
    ) -> lir::MaterializationRoot {
        lir::MaterializationRoot::cone_owned()
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
