use super::*;

impl<'a> StaticNominalShape<'a> {
    pub(super) fn primitive(module: &'a Module, ty: TypeId, kind: IntrinsicTypeKind) -> Self {
        let (declaration, origin, definition) = match &module.core_protocols {
            CoreProtocols::Defined(core) => {
                let core = &core.fundamental_types;
                let owner = match kind {
                    IntrinsicTypeKind::Integer(integer) => {
                        NominalOwner::Struct(core.integers.owner(integer))
                    }
                    IntrinsicTypeKind::Boolean => NominalOwner::Struct(core.boolean),
                    IntrinsicTypeKind::String => NominalOwner::Class(core.string),
                    _ => unreachable!("only primitive Type variants use this projection"),
                };
                let (identity, definition) = match owner {
                    NominalOwner::Struct(id) => (
                        module.nominal_identities[id].declaration_id(),
                        Definition::Struct(&module.structs[id].definition),
                    ),
                    NominalOwner::Class(id) => (
                        module.nominal_identities[id].declaration_id(),
                        Definition::Class(&module.classes[id].definition),
                    ),
                    _ => unreachable!("primitive source declarations are structs or classes"),
                };
                (identity, StaticNominalOrigin::Current(owner), definition)
            }
            CoreProtocols::Imported(core) => {
                let core = core.fundamental_types();
                let identity = match kind {
                    IntrinsicTypeKind::Integer(integer) => core.integer(integer).persistent(),
                    IntrinsicTypeKind::Boolean => core.boolean().persistent(),
                    IntrinsicTypeKind::String => core.string().persistent(),
                    _ => unreachable!("only primitive Type variants use this projection"),
                };
                let declaration = SourceNominalId::Concrete(identity);
                (
                    declaration,
                    StaticNominalOrigin::Dependency(declaration),
                    Definition::Primitive(kind),
                )
            }
        };
        Self {
            module,
            ty,
            declaration,
            arguments: &[],
            origin,
            definition,
            kind: StaticNominalKind::Intrinsic(kind),
        }
    }
}
