use super::*;
use scoop_identity::{
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey, ValidatedIdentityGraph,
};

mod representation;
use representation::representation;

pub(super) struct Fixture {
    pub input: mir::SingleConeStrongMirInput,
    pub output: lir::SingleConeStrongLirOutput,
    pub graph: ValidatedIdentityGraph,
    pub types: mir::CanonicalParamFreeMirTypeExportsV1,
    dependencies: lir::CanonicalExactLayoutExportsV1,
}
impl Fixture {
    pub fn new(mut builder: Builder) -> Self {
        let entry = builder.main(Arena::new(), Vec::new());
        let mut module = builder.finish(entry);
        let mut pending = PendingIdentityValidation::new();
        let mut representations = Vec::new();
        for record in module.meta.source_exact_types.iter() {
            let nominal = nominal(record.ty());
            pending
                .register_external_canonical_authority(nominal.clone())
                .unwrap();
            pending
                .register_external_canonical_authority(record.identity_record().clone())
                .unwrap();
            let (representation, facts, base) =
                representation(&module, record.ty(), nominal.key(), &mut pending);
            representations.push((
                record.identity_record().id(),
                nominal.id(),
                representation,
                facts,
                base,
            ));
        }
        for (id, definition) in module.structs.iter_mut() {
            if let mir::StructRepresentation::Declared { fields, .. } =
                &mut definition.representation
            {
                let owner = nominal(&mir::Type::Struct(id));
                for field in fields {
                    field.identity = PersistentFieldId::from_key(
                        &FieldIdentityKey::source_declared(
                            owner.key(),
                            CanonicalIdentifier::new(&field.name).unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                }
            }
        }
        let graph = pending.finish().unwrap();
        let input = seal_strong_input(module);
        let output = crate::lower(
            &input,
            test_runtime_string_descriptor(&input),
            &lir::SelectedExternalLirSet::empty(input.module().cone),
            lir::LirTargetProfile::DARWIN_AARCH64,
        )
        .unwrap();
        let types = representations
            .into_iter()
            .map(|(exact, nominal, representation, facts, base)| {
                mir::ParamFreeMirTypeExportV1::try_new(
                    mir::MirTypeBridgeAuthority {
                        identities: &graph,
                        foundation: input.foundation(),
                    },
                    exact,
                    mir::MirTypeOriginV1::SourceNominal(nominal),
                    facts,
                    representation,
                    mir::MirBaseAndInterfacesV1 {
                        base,
                        interfaces: vec![],
                    },
                )
                .unwrap()
            })
            .collect();
        let (_, provider, _, _) = crate::tests::exact_callable_abi::fixture();
        let dependencies = lir::CanonicalExactLayoutExportsV1::try_new(
            provider.module().meta.target_profile,
            provider.foundation(),
            vec![crate::tests::exact_callable_abi::unit_layout(&provider)],
            &mut meter(),
        )
        .unwrap();
        Self {
            input,
            output,
            graph,
            types: mir::CanonicalParamFreeMirTypeExportsV1::try_new(types).unwrap(),
            dependencies,
        }
    }
    pub fn replay(&self) -> Result<lir::CanonicalExactLayoutExportsV1, ExactLayoutLoweringError> {
        lower_exact_layout_exports(
            &self.input,
            &self.output,
            &self.types,
            &self.graph,
            &[&self.dependencies],
            &mut meter(),
        )
    }
    pub fn replace(
        &mut self,
        ty: &mir::Type,
        representation: mir::MirTypeRepresentationV1,
        facts: mir::MirTypeFactsV1,
    ) {
        let exact = exact(self.input.module(), ty);
        let old = self.types.get(exact).unwrap();
        let replacement = mir::ParamFreeMirTypeExportV1::try_new(
            mir::MirTypeBridgeAuthority {
                identities: &self.graph,
                foundation: self.input.foundation(),
            },
            exact,
            old.origin().clone(),
            facts,
            representation,
            old.base_and_interfaces().clone(),
        )
        .unwrap();
        self.types = mir::CanonicalParamFreeMirTypeExportsV1::try_new(
            self.types
                .records()
                .iter()
                .map(|record| {
                    if record.exact() == exact {
                        replacement.clone()
                    } else {
                        record.clone()
                    }
                })
                .collect(),
        )
        .unwrap();
    }
}

pub(super) fn exact(module: &mir::Module, ty: &mir::Type) -> PersistentExactTypeId {
    module
        .meta
        .source_exact_types
        .get(ty)
        .unwrap()
        .identity_record()
        .id()
}
fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
fn nominal(ty: &mir::Type) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    let (name, kind) = match ty {
        mir::Type::Unit => return CoreBuiltinNominal::Unit.identity_record(),
        mir::Type::Any => return CoreBuiltinNominal::Any.identity_record(),
        mir::Type::Integer(kind) => (
            format!("Test{}", kind.canonical_name()),
            SourceNominalKind::Struct,
        ),
        mir::Type::Boolean => ("TestBoolean".into(), SourceNominalKind::Struct),
        mir::Type::String => ("TestString".into(), SourceNominalKind::Class),
        mir::Type::Struct(id) => (
            format!("TestStruct{}", id.into_raw().into_u32()),
            SourceNominalKind::Struct,
        ),
        mir::Type::Class(id) => (
            format!("TestClass{}", id.into_raw().into_u32()),
            SourceNominalKind::Class,
        ),
        mir::Type::Interface(id) => (
            format!("TestInterface{}", id.into_raw().into_u32()),
            SourceNominalKind::Interface,
        ),
        mir::Type::Enum(id, _) => (
            format!("TestEnum{}", id.into_raw().into_u32()),
            SourceNominalKind::Enum,
        ),
        _ => panic!("finite fixture type"),
    };
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(),
        CanonicalIdentifier::new(&name).unwrap(),
        kind,
        0,
    ))
    .unwrap()
}
