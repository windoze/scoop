use super::*;

pub(in crate::production::nominal_interfaces::source_contracts) struct Node {
    pub local: LocalNominalId,
    pub parent: Option<SourceNominalId>,
    pub visibility: DeclaredVisibility,
}
pub(in crate::production::nominal_interfaces::source_contracts) struct Index {
    pub nodes: BTreeMap<SourceNominalId, Node>,
    pub children: BTreeMap<SourceNominalId, Vec<SourceNominalId>>,
}
impl Index {
    pub fn new(export: &ExportHir) -> Result<Self, Error> {
        let mut nodes = BTreeMap::new();
        let mut children = BTreeMap::<_, Vec<_>>::new();

        for local in locals(export) {
            let identity = local
                .identity(export)
                .ok_or_else(|| invalid("sealed nominal has no source identity"))?;
            let Some(source) = identity.source() else {
                continue;
            };
            let key = source.declaration();
            if key.origin() != export.cone {
                continue;
            }
            let owner = source_nominal_id(source);

            let parent = projection::header(export, local)
                .1
                .map(|owner| source_owner(export, owner))
                .transpose()?;
            if key.owners().owners().last() != parent.map(owner_atom).as_ref() {
                return Err(invalid("source root has a different lexical owner"));
            }

            if nodes
                .insert(
                    owner,
                    Node {
                        local,
                        parent,
                        visibility: visibility(export, local),
                    },
                )
                .is_some()
            {
                return Err(invalid("duplicate source nominal identity in sealed HIR"));
            }
            if let Some(parent) = parent {
                push(children.entry(parent).or_default(), owner)?;
            }
        }
        for node in nodes.values() {
            if node
                .parent
                .is_some_and(|parent| !nodes.contains_key(&parent))
            {
                return Err(invalid("source root has no local lexical owner"));
            }
        }
        Ok(Self { nodes, children })
    }
}

pub(super) fn source(export: &ExportHir, local: LocalNominalId) -> Result<SourceNominalId, Error> {
    local
        .identity(export)
        .and_then(HirNominalIdentity::source)
        .map(source_nominal_id)
        .ok_or_else(|| invalid("public nominal has no source identity"))
}

fn visibility(export: &ExportHir, local: LocalNominalId) -> DeclaredVisibility {
    match local {
        LocalNominalId::Class(id) => export.classes[id].access.declared,
        LocalNominalId::Interface(id) => export.interfaces[id].access.declared,
        LocalNominalId::Struct(id) => export.structs[id].access.declared,
        LocalNominalId::Enum(id) => export.enums[id].access.declared,
        LocalNominalId::Object(id) => export.objects[id].access.declared,
    }
}

pub(super) fn public(export: &ExportHir) -> impl Iterator<Item = LocalNominalId> + '_ {
    export
        .public_surface
        .classes
        .iter()
        .copied()
        .map(LocalNominalId::Class)
        .chain(
            export
                .public_surface
                .interfaces
                .iter()
                .copied()
                .map(LocalNominalId::Interface),
        )
        .chain(
            export
                .public_surface
                .structs
                .iter()
                .copied()
                .map(LocalNominalId::Struct),
        )
        .chain(
            export
                .public_surface
                .enums
                .iter()
                .copied()
                .map(LocalNominalId::Enum),
        )
        .chain(
            export
                .public_surface
                .objects
                .iter()
                .copied()
                .map(LocalNominalId::Object),
        )
}

pub(super) fn visit_bases(
    export: &ExportHir,
    local: LocalNominalId,
    mut visit: impl FnMut(TypeId) -> Result<(), Error>,
) -> Result<(), Error> {
    let (base, interfaces) = match local {
        LocalNominalId::Class(id) => {
            let value = &export.classes[id];
            (value.base_class, value.interfaces.as_slice())
        }
        LocalNominalId::Object(id) => {
            let value = &export.classes[export.objects[id].backing_class];
            (value.base_class, value.interfaces.as_slice())
        }
        LocalNominalId::Struct(id) => (None, export.structs[id].interfaces.as_slice()),
        LocalNominalId::Enum(id) => (None, export.enums[id].interfaces.as_slice()),
        LocalNominalId::Interface(id) => {
            for parent in &export.interfaces[id].parents {
                visit(*parent)?;
            }
            return Ok(());
        }
    };
    if let Some(base) = base {
        visit(base)?;
    }
    for interface in interfaces {
        visit(*interface)?;
    }
    Ok(())
}
