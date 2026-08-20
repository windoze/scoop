use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExpectedRoot {
    pub(super) source: scoop_lir::CallerRootSource,
    pub(super) byte_offset: u64,
}

impl ExpectedRoot {
    pub(super) fn key(self) -> (u8, u32, u64) {
        let (kind, index) = match self.source {
            scoop_lir::CallerRootSource::Param(index) => (0, index),
            scoop_lir::CallerRootSource::Local(id) => (1, id.into_raw().into_u32()),
            scoop_lir::CallerRootSource::Temp(id) => (2, id.into_raw().into_u32()),
        };
        (kind, index, self.byte_offset)
    }
}

/// Each group shares one actual SSA root; constants need no relocation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ExpectedRoots {
    pub(super) groups: Vec<Vec<ExpectedRoot>>,
    pub(super) constants: Vec<ExpectedRoot>,
}

impl From<Vec<ExpectedRoot>> for ExpectedRoots {
    fn from(roots: Vec<ExpectedRoot>) -> Self {
        Self {
            groups: roots.into_iter().map(|root| vec![root]).collect(),
            constants: Vec::new(),
        }
    }
}

impl ExpectedRoots {
    pub(super) fn identities(&self) -> BTreeSet<(u8, u32, u64)> {
        self.groups
            .iter()
            .flatten()
            .chain(&self.constants)
            .copied()
            .map(ExpectedRoot::key)
            .collect()
    }

    pub(super) fn group_keys(&self) -> BTreeSet<BTreeSet<(u8, u32, u64)>> {
        self.groups
            .iter()
            .map(|group| group.iter().copied().map(ExpectedRoot::key).collect())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ExpectedStatepoint {
    Relocating(ExpectedRoots),
    NativeTransition(scoop_lir::RuntimeAbiSymbolV1),
    ZeroLiveInvoke,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExpectedSite {
    pub(super) function: String,
    pub(super) block: String,
    pub(super) statepoint: ExpectedStatepoint,
}

#[derive(Clone, Debug)]
pub(crate) struct ExpectedSafepoints {
    pub(super) sites: BTreeMap<u64, ExpectedSite>,
    pub(super) functions: BTreeMap<String, GcEffect>,
}

impl ExpectedSafepoints {
    pub(crate) fn site_count(&self) -> usize {
        self.sites.len()
    }

    pub(crate) fn root_count(&self, safepoint: u64) -> Option<usize> {
        self.sites
            .get(&safepoint)
            .map(|site| match &site.statepoint {
                ExpectedStatepoint::Relocating(roots) => roots.groups.len(),
                ExpectedStatepoint::NativeTransition(_) | ExpectedStatepoint::ZeroLiveInvoke => 0,
            })
    }

    pub(crate) fn without_body_sites(&self) -> Self {
        Self {
            sites: BTreeMap::new(),
            functions: self.functions.clone(),
        }
    }

    pub(crate) fn for_function(&self, symbol: &str) -> Result<Self, CodegenError> {
        if !self.functions.contains_key(symbol) {
            return Err(CodegenError(format!(
                "cannot select safepoints for unknown LIR function `{symbol}`"
            )));
        }
        Ok(Self {
            sites: self
                .sites
                .iter()
                .filter(|(_, site)| site.function == symbol)
                .map(|(id, site)| (*id, site.clone()))
                .collect(),
            functions: self.functions.clone(),
        })
    }
}

pub(crate) fn expectations(module: &scoop_lir::Module) -> Result<ExpectedSafepoints, CodegenError> {
    let mut sites = BTreeMap::new();
    let mut functions = BTreeMap::new();
    for function in module.callable_bodies() {
        if functions
            .insert(function.symbol().to_string(), function.gc_effect)
            .is_some()
        {
            return Err(CodegenError(format!(
                "duplicate LIR function symbol `{}` in statepoint manifest",
                function.symbol()
            )));
        }
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                let expectation = match instruction {
                    scoop_lir::Instruction::ManagedPoll { site } => {
                        Some((site.safepoint, relocating_roots(&site.live)))
                    }
                    scoop_lir::Instruction::Call { site } => match site {
                        scoop_lir::CallSite::Managed(site) => {
                            Some((site.safepoint, relocating_roots(&site.live)))
                        }
                        scoop_lir::CallSite::NativeSafe(site) => Some((
                            site.safepoint,
                            ExpectedStatepoint::NativeTransition(
                                scoop_lir::RuntimeAbiSymbolV1::EnterNativeSafe,
                            ),
                        )),
                        scoop_lir::CallSite::NativeBorrowed(site) => Some((
                            site.safepoint,
                            ExpectedStatepoint::NativeTransition(
                                scoop_lir::RuntimeAbiSymbolV1::EnterNativeBorrowed,
                            ),
                        )),
                        scoop_lir::CallSite::NoGc(_)
                        | scoop_lir::CallSite::ReleaseScoop(_)
                        | scoop_lir::CallSite::NativeGcLeaf(_)
                        | scoop_lir::CallSite::ReleaseNativeLeaf(_) => None,
                    },
                    scoop_lir::Instruction::Invoke { site } => match site {
                        scoop_lir::InvokeSite::Managed(site) => {
                            Some((site.safepoint, ExpectedStatepoint::ZeroLiveInvoke))
                        }
                        scoop_lir::InvokeSite::NoGc(_) => None,
                    },
                    scoop_lir::Instruction::NativeGlobalLoad { protocol, .. }
                    | scoop_lir::Instruction::NativeGlobalStore { protocol, .. }
                    | scoop_lir::Instruction::NativeGlobalAddress { protocol, .. } => {
                        protocol.safepoint().map(|safepoint| {
                            (
                                safepoint,
                                ExpectedStatepoint::NativeTransition(
                                    scoop_lir::RuntimeAbiSymbolV1::EnterNativeSafe,
                                ),
                            )
                        })
                    }
                    scoop_lir::Instruction::BoxValue {
                        safepoint, live, ..
                    }
                    | scoop_lir::Instruction::ArrayAllocDynamic {
                        safepoint, live, ..
                    }
                    | scoop_lir::Instruction::ArrayAlloc {
                        safepoint, live, ..
                    }
                    | scoop_lir::Instruction::ArrayAssembly {
                        safepoint, live, ..
                    }
                    | scoop_lir::Instruction::ArrayClone {
                        safepoint, live, ..
                    } => Some((*safepoint, relocating_roots(live))),
                    _ => None,
                };
                let Some((safepoint, expectation)) = expectation else {
                    continue;
                };
                let safepoint = function
                    .safepoints
                    .get(safepoint)
                    .ok_or_else(|| {
                        CodegenError(format!(
                            "LIR function `{}` references missing safepoint site {}",
                            function.symbol(),
                            safepoint.into_u32()
                        ))
                    })?
                    .runtime_id();
                if function.gc_effect == GcEffect::NoGc {
                    return Err(CodegenError(format!(
                        "NoGc LIR function `{}` contains safepoint {}",
                        function.symbol(),
                        safepoint.get()
                    )));
                }
                insert_expectation(
                    &mut sites,
                    function.symbol(),
                    &block.name,
                    safepoint,
                    expectation,
                )?;
            }
        }
    }
    Ok(ExpectedSafepoints { sites, functions })
}

fn relocating_roots(live: &scoop_lir::StatepointLiveSet) -> ExpectedStatepoint {
    ExpectedStatepoint::Relocating(
        live.as_slice()
            .iter()
            .flat_map(|value| {
                value.leaves.as_slice().iter().map(|leaf| ExpectedRoot {
                    source: value.source,
                    byte_offset: leaf.byte_offset,
                })
            })
            .collect::<Vec<_>>()
            .into(),
    )
}

fn insert_expectation(
    expected: &mut BTreeMap<u64, ExpectedSite>,
    function: &str,
    block: &str,
    id: scoop_lir::SafepointId,
    statepoint: ExpectedStatepoint,
) -> Result<(), CodegenError> {
    if expected
        .insert(
            id.get(),
            ExpectedSite {
                function: function.to_string(),
                block: block.to_string(),
                statepoint,
            },
        )
        .is_some()
    {
        return Err(CodegenError(format!(
            "duplicate image SafepointId {} in complete LIR",
            id.get()
        )));
    }
    Ok(())
}
