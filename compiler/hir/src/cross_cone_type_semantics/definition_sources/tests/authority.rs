use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum UseKey {
    Representation(PersistentTypeId),
    Constructor(PersistentExactTypeId, PersistentConstructorId),
    Slot(PersistentExactTypeId, PersistentDispatchSlotId),
    Implementation(
        PersistentExactTypeId,
        PersistentDispatchSlotId,
        InheritanceCallableDeclarationV1,
    ),
    Protected(ProtectedDeclarationRefV1),
    Nested(SourceNominalId, NestedSupportDeclarationV1),
    Setter(PersistentPropertyId),
    Const(PersistentPropertyId),
    Parameter(CallableTemplateOrigin, usize),
    Default(ProtectedDefaultTemplateKeyV1, DefaultSite),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DefaultSite {
    Root,
    Local(LocalValueSelector),
    Body(DefaultBodyOriginSiteV1),
    Callable,
    Constructor,
    Type,
    Global,
    Singleton,
    Field,
}
#[derive(Clone)]
pub(super) struct Expected {
    pub key: UseKey,
    pub origin: ExportDefinitionSourceV1,
}
pub(super) struct Authority<'a> {
    expected: &'a [Expected],

    path: &'a WirePath,
    pub seen: Vec<bool>,
    pub calls: usize,
}
impl<'a> Authority<'a> {
    pub fn new(expected: &'a [Expected], path: &'a WirePath) -> Self {
        Self {
            expected,

            path,
            seen: vec![false; expected.len()],
            calls: 0,
        }
    }
}
impl TypeDefinitionSourceSemanticAuthority<&'static str> for Authority<'_> {
    fn validate_type_definition_source_use(
        &mut self,
        source_use: TypeDefinitionSourceUseV1<'_>,
        source: &ExportDefinitionSourceV1,

        path: &WirePath,
    ) -> Result<(), &'static str> {
        assert!(path.segments().starts_with(self.path.segments()));

        let key = match source_use {
            TypeDefinitionSourceUseV1::Representation(record) => {
                UseKey::Representation(record.owner())
            }
            TypeDefinitionSourceUseV1::InheritanceConstructor { owner, constructor } => {
                UseKey::Constructor(owner, constructor.declaration())
            }
            TypeDefinitionSourceUseV1::SlotDeclaration { owner, slot } => {
                UseKey::Slot(owner, slot.slot())
            }
            TypeDefinitionSourceUseV1::SlotImplementation {
                owner,
                slot,
                target,
            } => UseKey::Implementation(owner, slot.slot(), target.declaration()),
            TypeDefinitionSourceUseV1::ProtectedDeclaration(record) => {
                UseKey::Protected(record.reference())
            }
            TypeDefinitionSourceUseV1::NestedSupport { owner, declaration } => {
                UseKey::Nested(owner, declaration.declaration())
            }
            TypeDefinitionSourceUseV1::PropertySetter { property, .. } => UseKey::Setter(property),
            TypeDefinitionSourceUseV1::NestedConst { property, value } => {
                assert_eq!(property.declaration(), value.property());
                UseKey::Const(value.property())
            }
            TypeDefinitionSourceUseV1::SourceParameter {
                source,
                parameter_index,
            } => UseKey::Parameter(source.owner(), parameter_index),
            TypeDefinitionSourceUseV1::Default { template, site } => UseKey::Default(
                template.key(),
                match site {
                    TypeDefinitionDefaultOriginSiteV1::Root => DefaultSite::Root,
                    TypeDefinitionDefaultOriginSiteV1::Local(local) => {
                        DefaultSite::Local(local.selector().clone())
                    }
                    TypeDefinitionDefaultOriginSiteV1::Body(site) => DefaultSite::Body(site),
                    TypeDefinitionDefaultOriginSiteV1::Callable(_) => DefaultSite::Callable,
                    TypeDefinitionDefaultOriginSiteV1::Constructor(_) => DefaultSite::Constructor,
                    TypeDefinitionDefaultOriginSiteV1::Type(_) => DefaultSite::Type,
                    TypeDefinitionDefaultOriginSiteV1::Global(_) => DefaultSite::Global,
                    TypeDefinitionDefaultOriginSiteV1::Singleton(_) => DefaultSite::Singleton,
                    TypeDefinitionDefaultOriginSiteV1::Field(_) => DefaultSite::Field,
                },
            ),
        };
        let Some(index) = self
            .expected
            .iter()
            .enumerate()
            .position(|(index, expected)| {
                !self.seen[index] && expected.key == key && &expected.origin == source
            })
        else {
            return Err("origin disagrees with independent typed provider use");
        };
        self.seen[index] = true;
        self.calls += 1;
        Ok(())
    }
}
