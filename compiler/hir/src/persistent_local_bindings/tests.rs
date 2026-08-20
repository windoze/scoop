use scoop_identity::{
    BindingTarget, CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOrigin,
    DefinitionOwnerChain, LocalBindingKey, LocalBindingRole, NormalizedSourcePath, PackagePath,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceSpan,
};

use super::*;

#[test]
fn repeated_bindings_choose_the_least_origin_independent_of_insertion_order() {
    let source = source("src/main.scoop");
    let declaration = source_function("run");
    let binding = BindingTarget::function(&declaration).unwrap();
    let key = LocalBindingKey::new(
        source.clone(),
        PackagePath::root(),
        CanonicalIdentifier::new("run").unwrap(),
        binding,
        LocalBindingRole::ExactImport,
    );
    let early = origin(source.clone(), 2, 5);
    let late = origin(source, 12, 18);
    let late_entry = HirLocalBindingIdentity::new(key.clone(), late).unwrap();
    let early_entry = HirLocalBindingIdentity::new(key, early.clone()).unwrap();

    let forward =
        HirLocalBindingIdentities::canonicalize(vec![late_entry.clone(), early_entry.clone()])
            .unwrap();
    let reverse = HirLocalBindingIdentities::canonicalize(vec![early_entry, late_entry]).unwrap();

    assert_eq!(forward.len(), 1);
    assert_eq!(forward.iter().next().unwrap().origin(), &early);
    assert_eq!(
        forward.iter().next().unwrap(),
        reverse.iter().next().unwrap()
    );
}

#[test]
fn binding_origin_must_use_the_key_source() {
    let key_source = source("src/key.scoop");
    let declaration = source_function("run");
    let key = LocalBindingKey::new(
        key_source,
        PackagePath::root(),
        CanonicalIdentifier::new("run").unwrap(),
        BindingTarget::function(&declaration).unwrap(),
        LocalBindingRole::Declaration,
    );

    assert_eq!(
        HirLocalBindingIdentity::new(key, origin(source("src/other.scoop"), 0, 0)),
        Err(HirLocalBindingIdentityError::OriginSourceMismatch)
    );
}

fn source(path: &str) -> SourceIdentity {
    SourceIdentity::new(ConeIdentity::CORE, NormalizedSourcePath::new(path).unwrap()).unwrap()
}

fn source_function(name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    )
}

fn origin(source: SourceIdentity, start: u64, end: u64) -> DefinitionOrigin {
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    DefinitionOrigin::new(source, SourceSpan::new(start, end).unwrap(), &context).unwrap()
}
