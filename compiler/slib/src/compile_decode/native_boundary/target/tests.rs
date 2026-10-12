use super::*;
use scoop_identity::*;

#[test]
fn consumer_rejects_a_symbol_normalized_for_the_wrong_object_format() {
    let source = source_contract();
    let symbol = SourceNativeSymbol::new("native_entry").unwrap();
    for target in [
        scoop_target_data::DARWIN_AARCH64,
        scoop_target_data::LINUX_X86_64_GNU,
    ] {
        for normalization in [
            NativeSymbolNormalization::MachOExternalUnderscore,
            NativeSymbolNormalization::ElfIdentity,
        ] {
            let actual = NativeExternalContractRecord::new(
                source.id(),
                NativeExternalSymbolKey::for_target(target.wire_id(), normalization, &symbol)
                    .unwrap(),
                NativeExternalContract::c_function(
                    NativeLibraryBinding::DefaultNativeNamespace,
                    CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void),
                ),
            )
            .unwrap();
            let exact = HashMap::new();
            let applications = HashMap::new();
            let initializations = HashMap::new();
            let definitions = HashMap::new();
            let mut normalizer = NativeBoundaryNormalizer::new(
                target,
                &exact,
                &applications,
                &initializations,
                &definitions,
            );
            let expected = normalizer.normalize_external(&source, &actual).unwrap();
            let result = require_equal_records(
                &HashMap::from([(source.id(), expected)]),
                &HashMap::from([(source.id(), actual)]),
                NativeBoundaryTargetError::NativeContractMismatch,
            );
            if normalization == target.contract().native_symbol_normalization() {
                result.unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(NativeBoundaryCompileError::Target(
                        NativeBoundaryTargetError::NativeContractMismatch
                    ))
                ));
            }
        }
    }
}

fn source_contract() -> SourceNativeExternalContractRecord {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("entry").unwrap(),
        0,
        None,
        Vec::new(),
    );
    SourceNativeExternalContractRecord::new(
        SourceNativeExternalContractKey::function(&declaration).unwrap(),
        SourceNativeExternalContract::Function {
            symbol: SourceNativeSymbol::new("native_entry").unwrap(),
            library: SourceNativeLibraryBinding::DefaultNativeNamespace,
            abi: SourceExternFunctionAbi::C(SourceCAbiFunctionSignature::new(
                Vec::new(),
                SourceCAbiReturn::Void,
            )),
            calling_convention: SourceCallingConvention::Cdecl,
        },
    )
    .unwrap()
}
