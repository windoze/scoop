use super::*;
use scoop_identity::{
    ExactCallableSignature, PersistentConstructorId, StrongCallableDefinitionOwner,
};
use scoop_slib::{
    SharedMirConstructorComponent as Component, SharedMirConstructorValidationError as Error,
};

mod inventory;
mod mutations;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    foundation: &mir::CanonicalMirFoundation,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    let replay = Replay {
        source,
        foundation,
        section,
    };
    replay.validate(section.callables()).unwrap();
    if !name.starts_with("shared-constructors-") {
        return;
    }
    inventory::check(&replay, name.ends_with("combined"));
    mutations::check(&replay);

    replay.validate(section.callables()).unwrap();
}

struct Replay<'a> {
    source: hir::CheckedSharedTypeFoundationV1<'a>,
    foundation: &'a mir::CanonicalMirFoundation,
    section: &'a mir::CrossConeMirTypeBridgeSectionV1<'a>,
}

impl Replay<'_> {
    fn validate(&self, bindings: &mir::CanonicalMirCallableBindingsV1) -> Result<(), Error> {
        scoop_slib::validate_shared_mir_constructors(self.source, bindings)
    }

    fn reject(&self, records: Vec<mir::ParamFreeMirCallableBindingV1>) -> Error {
        let records = mir::CanonicalMirCallableBindingsV1::try_new(records).unwrap();
        self.validate(&records)
            .expect_err("constructor bindings must agree with the retained shared HIR declarations")
    }

    fn replace(
        &self,
        changed: mir::ParamFreeMirCallableBindingV1,
    ) -> Vec<mir::ParamFreeMirCallableBindingV1> {
        self.section
            .callables()
            .entries()
            .iter()
            .map(|binding| {
                if binding.implementation() == changed.implementation() {
                    changed.clone()
                } else {
                    binding.clone()
                }
            })
            .collect()
    }

    fn fixture_bindings(&self) -> impl Iterator<Item = &mir::ParamFreeMirCallableBindingV1> {
        self.section.callables().entries().iter().filter(|binding| {
            let mir::MirCallableOriginV1::Constructor(id) = binding.origin() else { return false };
            let key = self.source.metadata().identities.canonical_key::<_, scoop_identity::SourceDeclarationKey>(*id).unwrap();
            let Some(scoop_identity::DefinitionOwnerAtom::Type(owner)) = key.owners().owners().last() else { return false };
            let owner = self.source.metadata().identities.canonical_key::<_, scoop_identity::SourceDeclarationKey>(*owner).unwrap();
            matches!(owner.name(), scoop_identity::DeclarationName::Named(name) if name.as_str().starts_with("SharedConstructor"))
        })
    }
}

fn declaration(binding: &mir::ParamFreeMirCallableBindingV1) -> PersistentConstructorId {
    let mir::MirCallableOriginV1::Constructor(id) = binding.origin() else {
        panic!("fixture binding is a source constructor")
    };
    *id
}

fn component(error: Error, expected: Component, id: PersistentConstructorId) {
    assert!(
        matches!(error, Error::Mismatch { declaration, component } if declaration == id && component == expected),
        "expected {expected:?}, got {error:?}"
    );
}
