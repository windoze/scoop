use super::*;
use MirTypeBridgeSourceProjectionError as Error;
use scoop_identity::InitializationCallableRole;

pub(super) struct InitializationContracts {
    pub inventory: Vec<PersistentInitializationUnitId>,
    signatures: Vec<[mir::MirBridgeCallableSignatureV1; 2]>,
}

impl InitializationContracts {
    pub fn from_input(input: MirTypeBridgeExportInputV1<'_>) -> Result<Self, Error> {
        let module = input.hir.output().local.module();
        let roots = input.mir.materialization().initialization_roots();
        let mut sources =
            inventory::collect(module.initialization_units.iter().map(|(_, unit)| unit))?;

        sources.sort_unstable_by_key(|unit| unit.identity.id());
        if sources.len() != roots.len()
            || sources
                .windows(2)
                .any(|pair| pair[0].identity.id() == pair[1].identity.id())
        {
            return Err(Error::InitializationInventory);
        }
        let inventory = inventory::collect(sources.iter().map(|unit| unit.identity.id()))?;

        for root in roots {
            if inventory.binary_search(&root.identity()).is_err() {
                return Err(Error::MissingInitializationUnit(root.identity()));
            }
        }
        let mut signatures = Vec::new();
        scoop_wire::allocation::try_reserve(&mut signatures, sources.len(), &WirePath::root())?;
        for unit in sources {
            signatures.push([
                signature(module, unit.initializer)?,
                signature(module, unit.ensure)?,
            ]);
        }
        Ok(Self {
            inventory,
            signatures,
        })
    }

    pub fn signature(
        &self,
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRole,
    ) -> Result<&mir::MirBridgeCallableSignatureV1, Error> {
        let index = self
            .inventory
            .binary_search(&unit)
            .map_err(|_| Error::MissingInitializationUnit(unit))?;
        Ok(&self.signatures[index][match role {
            InitializationCallableRole::Initializer => 0,
            InitializationCallableRole::Ensure => 1,
        }])
    }
}

fn signature(
    module: &hir::concrete::Module,
    function: hir::concrete::FunctionId,
) -> Result<mir::MirBridgeCallableSignatureV1, Error> {
    let source = &module.functions[function];

    Ok(mir::MirBridgeCallableSignatureV1::new(
        crate::source_callables::exact_function_signature(module, function),
        match source.attributes.gc_effect {
            hir::GcEffect::Managed => mir::GcEffect::Managed,
            hir::GcEffect::NoGc => mir::GcEffect::NoGc,
        },
    ))
}
