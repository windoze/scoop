use super::*;
use scoop_identity::{
    DeclarationName, GeneratedCallableKey, SignatureTypeKey, SourceDeclarationKey,
    StrongCallableDefinitionOwner,
};
use scoop_slib::{
    SharedMirDispatchComponent as Component, SharedMirDispatchValidationError as Error,
};

mod dump;
mod mutations;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    foundation: &mir::OdrFreeMirFoundation,
    ordinary: &mir::CrossConeMirBridgeSectionV1,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    let replay = Replay {
        source,
        foundation,
        ordinary,
        callables: mir::MirTypeBridgeCallableIndexV1::try_new(&[section.callables()], &[ordinary])
            .unwrap(),
        section,
    };
    replay
        .validate(section.dispatch(), section.callables())
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    if !name.starts_with("shared-dispatch-") {
        return;
    }
    let combined = name.ends_with("combined");
    dump::check(&replay, name);
    mutations::check(&replay, combined);
    let missing = replay.owner(if combined {
        "SharedDispatchMutableImpl"
    } else {
        "SharedDispatchChild"
    });
    let remaining = replay
        .section
        .dispatch()
        .records()
        .iter()
        .filter(|record| record.owner() != missing)
        .cloned()
        .collect();
    let error = replay.reject(remaining);
    assert!(
        matches!(error, Error::Missing(owner) if owner == missing),
        "{error:?}"
    );

    replay
        .validate(section.dispatch(), section.callables())
        .unwrap();
}

struct Replay<'a> {
    source: hir::CheckedSharedTypeFoundationV1<'a>,
    foundation: &'a mir::OdrFreeMirFoundation,
    ordinary: &'a mir::CrossConeMirBridgeSectionV1,
    callables: mir::MirTypeBridgeCallableIndexV1<'a>,
    section: &'a mir::CrossConeMirTypeBridgeSectionV1<'a>,
}

impl Replay<'_> {
    fn validate(
        &self,
        dispatch: &mir::CanonicalMirDispatchSchemasV1,
        callables: &mir::CanonicalMirCallableBindingsV1,
    ) -> Result<(), Error> {
        scoop_slib::validate_shared_mir_dispatch(
            self.source,
            &[],
            callables,
            &[],
            &[self.ordinary],
            dispatch,
        )
    }
    fn authority(&self) -> mir::MirDispatchSchemaAuthority<'_> {
        mir::MirDispatchSchemaAuthority {
            identities: self.source.metadata().identities,
            types: self.section.types(),
            callables: &self.callables,
        }
    }
    fn reject(&self, records: Vec<mir::ParamFreeMirDispatchSchemaV1>) -> Error {
        let table = mir::CanonicalMirDispatchSchemasV1::try_new(self.authority(), records)
            .expect("the changed table retains all MIR identity, signature and receiver relations");
        self.validate(&table, self.section.callables())
            .expect_err("shared HIR must reject a different source dispatch choice")
    }
    fn replace(
        &self,
        changed: mir::ParamFreeMirDispatchSchemaV1,
    ) -> Vec<mir::ParamFreeMirDispatchSchemaV1> {
        self.section
            .dispatch()
            .records()
            .iter()
            .map(|record| {
                if record.owner() == changed.owner() {
                    changed.clone()
                } else {
                    record.clone()
                }
            })
            .collect()
    }
    fn owner(&self, name: &str) -> scoop_identity::PersistentExactTypeId {
        let metadata = self.source.metadata();
        let owner = metadata
            .public
            .nominal_interfaces()
            .all_records()
            .find_map(|record| {
                let hir::SourceNominalId::Concrete(owner) = record.declaration() else {
                    return None;
                };
                let key = metadata
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(owner)
                    .unwrap();
                matches!(key.name(), DeclarationName::Named(actual) if actual.as_str() == name)
                    .then_some(owner)
            })
            .unwrap();
        metadata
            .signature_exact_type(&SignatureTypeKey::Nominal(owner))
            .unwrap()
    }
    fn vtable(
        &self,
        owner: scoop_identity::PersistentExactTypeId,
        entries: Vec<mir::MirDispatchEntryV1>,
    ) -> mir::ParamFreeMirDispatchSchemaV1 {
        let original = self.section.dispatch().get(owner).unwrap();
        mir::ParamFreeMirDispatchSchemaV1::try_new(
            self.authority(),
            owner,
            mir::MirClassVtableSchemaV1::ClassVtable(entries),
            original.itables().to_vec(),
        )
        .unwrap()
    }
}

fn component(error: Error, expected: Component) {
    assert!(
        matches!(error, Error::Entry { component, .. } | Error::Schema { component, .. } if component == expected),
        "expected {expected:?}, got {error:?}"
    );
}
