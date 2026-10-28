use super::*;

/// Immortal constants keyed by their semantic owner and structural site.
///
/// A repeated property-constant read names the same immutable object, while
/// separate literal occurrences remain distinct even when their bytes match.
#[derive(Default)]
pub(super) struct ImmortalRegistry {
    values: Arena<mir::ImmortalConst>,
    by_identity: HashMap<mir::ImmortalObjectKey, mir::ImmortalConstId>,
}

impl ImmortalRegistry {
    pub(super) fn intern(
        &mut self,
        owner: mir::ImmortalObjectOwner,
        ordinal: u32,
        value: String,
    ) -> mir::ImmortalConstId {
        self.intern_from(owner, ordinal, value, mir::ImmortalConstSource::CurrentCone)
    }

    pub(super) fn intern_from(
        &mut self,
        owner: mir::ImmortalObjectOwner,
        ordinal: u32,
        value: String,
        source: mir::ImmortalConstSource,
    ) -> mir::ImmortalConstId {
        self.intern_value(
            owner,
            ordinal,
            mir::ImmortalConstValue::String(value),
            source,
        )
    }

    pub(super) fn intern_value(
        &mut self,
        owner: mir::ImmortalObjectOwner,
        ordinal: u32,
        value: mir::ImmortalConstValue,
        source: mir::ImmortalConstSource,
    ) -> mir::ImmortalConstId {
        let role = match &value {
            mir::ImmortalConstValue::String(_) => mir::StructuralDefinitionSiteRole::StringConstant,
            mir::ImmortalConstValue::ByteArray { .. }
            | mir::ImmortalConstValue::EmbeddedFileArray { .. } => {
                mir::StructuralDefinitionSiteRole::ArrayConstant
            }
        };
        let path = mir::StructuralDefinitionPath::from_first(
            mir::StructuralPathSegment::new(role, ordinal),
            [],
        );
        let identity = match &value {
            mir::ImmortalConstValue::String(_) => {
                mir::ImmortalObjectKey::string_constant(owner, path)
            }
            mir::ImmortalConstValue::ByteArray { .. }
            | mir::ImmortalConstValue::EmbeddedFileArray { .. } => {
                mir::ImmortalObjectKey::array_constant(owner, path)
            }
        };
        if let Some(&id) = self.by_identity.get(&identity) {
            assert_eq!(self.values[id].source, source);
            assert_eq!(
                self.values[id].value, value,
                "one immortal-constant identity cannot name different content"
            );
            return id;
        }

        let id = self.values.alloc(mir::ImmortalConst {
            identity: identity.clone(),
            value,
            source,
        });
        let previous = self.by_identity.insert(identity, id);
        assert!(previous.is_none(), "new immortal identity was absent");
        id
    }

    pub(super) fn finish(self) -> Arena<mir::ImmortalConst> {
        self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_identity::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        CanonicalIdentifier, ConeCoordinate, DeclarationScope, DefinitionOwnerChain, PackagePath,
        PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    };

    fn callable(name: &str) -> mir::ImmortalObjectOwner {
        let site = SourceDeclarationSite::new(
            test_cone(),
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let declaration = SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            vec![],
        );
        let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        mir::ImmortalObjectOwner::Callable(CallableMaterialization::new(
            CallableTemplateOwner::Function(function),
            CallableMaterializationContext::NoSubstitution,
        ))
    }

    fn test_cone() -> scoop_identity::ConeIdentity {
        ConeCoordinate::new("test", "immortals", "0.0.0")
            .unwrap()
            .identity()
            .unwrap()
    }

    #[test]
    fn byte_array_sites_reuse_raw_bytes_and_do_not_alias_string_sites() {
        let mut values = ImmortalRegistry::default();
        let owner = callable("main");
        let value = mir::ImmortalConstValue::ByteArray {
            array_type: mir::ClassId::from_raw(la_arena::RawIdx::from_u32(0)),
            bytes: vec![0, 255, 128],
        };
        let bytes = values.intern_value(
            owner,
            0,
            value.clone(),
            mir::ImmortalConstSource::CurrentCone,
        );
        let repeated = values.intern_value(
            owner,
            0,
            value.clone(),
            mir::ImmortalConstSource::CurrentCone,
        );
        let text = values.intern(owner, 0, "text".into());
        assert_eq!(bytes, repeated);
        assert_ne!(bytes, text);
        assert_eq!(values.finish()[bytes].value, value);
    }

    #[test]
    fn equal_identity_reuses_one_constant() {
        let mut immortals = ImmortalRegistry::default();
        let owner = callable("main");
        let first = immortals.intern(owner, 0, "same".to_string());
        let repeated = immortals.intern(owner, 0, "same".to_string());
        assert_eq!(first, repeated);
        assert_eq!(immortals.finish().len(), 1);
    }

    #[test]
    fn equal_content_at_distinct_sites_remains_distinct() {
        let mut immortals = ImmortalRegistry::default();
        let owner = callable("main");
        let first = immortals.intern(owner, 0, "same".to_string());
        let second = immortals.intern(owner, 1, "same".to_string());
        assert_ne!(first, second);
        assert_eq!(immortals.finish().len(), 2);
    }

    #[test]
    #[should_panic(expected = "one immortal-constant identity cannot name different content")]
    fn equal_identity_cannot_change_content() {
        let mut immortals = ImmortalRegistry::default();
        let owner = callable("main");
        immortals.intern(owner, 0, "first".to_string());
        immortals.intern(owner, 0, "second".to_string());
    }
}
