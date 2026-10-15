use super::*;
use scoop_identity::PersistentConstructorId;
mod inventory;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    let replay = Fixture { source, section };
    if !name.starts_with("shared-constructors-") {
        return;
    }
    inventory::check(&replay, name.ends_with("combined"));
}

struct Fixture<'a> {
    source: hir::CheckedSharedTypeFoundationV1<'a>,
    section: &'a mir::CrossConeMirTypeBridgeSectionV1<'a>,
}

impl Fixture<'_> {
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
