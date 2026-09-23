use super::*;
use MirTypeBridgeSourceProjectionError as Error;
use scoop_identity::InitializationCallableRole;

pub(super) struct InitializationContracts {
    pub inventory: Vec<PersistentInitializationUnitId>,
    signatures: Vec<[mir::MirBridgeCallableSignatureV1; 2]>,
}

impl InitializationContracts {
    pub fn from_input(
        input: MirTypeBridgeExportInputV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let module = input.hir.output().local.module();
        let roots = input.mir.materialization().initialization_roots();
        let mut sources = inventory::collect(
            module.initialization_units.iter().map(|(_, unit)| unit),
            meter,
        )?;
        inventory::sort_cost(sources.len(), meter)?;
        sources.sort_unstable_by_key(|unit| unit.identity.id());
        if sources.len() != roots.len()
            || sources
                .windows(2)
                .any(|pair| pair[0].identity.id() == pair[1].identity.id())
        {
            return Err(Error::InitializationInventory);
        }
        let inventory = inventory::collect(sources.iter().map(|unit| unit.identity.id()), meter)?;
        meter.charge_work(roots.len() as u64, &WirePath::root())?;
        for root in roots {
            meter.charge_work(
                u64::from(inventory.len().checked_ilog2().unwrap_or(0)) + 1,
                &WirePath::root(),
            )?;
            if inventory.binary_search(&root.identity()).is_err() {
                return Err(Error::MissingInitializationUnit(root.identity()));
            }
        }
        let mut signatures = Vec::new();
        meter.try_reserve_collection_slots(&mut signatures, sources.len(), &WirePath::root())?;
        for unit in sources {
            signatures.push([
                signature(module, unit.initializer, meter)?,
                signature(module, unit.ensure, meter)?,
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
    meter: &mut BudgetMeter,
) -> Result<mir::MirBridgeCallableSignatureV1, Error> {
    let source = &module.functions[function];
    let path = WirePath::root();
    meter.charge_work(source.params.len() as u64 + 1, &path)?;
    meter.charge_owned_bytes(
        std::mem::size_of::<mir::MirBridgeCallableSignatureV1>() as u64
            + (source.params.len() as u64)
                .saturating_mul(std::mem::size_of::<PersistentExactTypeId>() as u64),
        &path,
    )?;
    Ok(mir::MirBridgeCallableSignatureV1::new(
        crate::source_callables::exact_function_signature(module, function),
        match source.attributes.gc_effect {
            hir::GcEffect::Managed => mir::GcEffect::Managed,
            hir::GcEffect::NoGc => mir::GcEffect::NoGc,
        },
    ))
}
