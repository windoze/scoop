//! LLVM-independent part of codegen: classify complete physical call signatures.
//! The production driver reaches this through scoop-codegen. Lowering's tests
//! use the same classifier without loading LLVM or another lowering stage.

use scoop_lir as lir;

mod aggregate;
mod c;
mod layout;
mod registers;
mod scoop;

pub use c::{c_indirect_passing, c_integer_extension};

pub use lir::ProcedureAbi;

fn procedure_abi(target: lir::LirTargetProfile) -> Result<ProcedureAbi, AbiPlanError> {
    scoop_target_data::definition(target.id())
        .filter(|definition| definition.selection.target() == target)
        .map(|_| target.procedure_abi())
        .ok_or(AbiPlanError::UnsupportedTarget(target.id()))
}

pub fn plan(input: &lir::AbiPlanningInput) -> Result<lir::AbiPlanSet, AbiPlanError> {
    let abi = procedure_abi(input.target)?;
    Ok(lir::AbiPlanSet {
        scoop: input
            .scoop
            .iter()
            .map(|(_, request)| scoop::classify(input, request, abi))
            .collect::<Result<_, _>>()?,
        c: input
            .c
            .iter()
            .map(|(_, request)| c::classify(input, request, abi))
            .collect::<Result<_, _>>()?,
    })
}

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub enum AbiPlanError {
    UnsupportedTarget(lir::TargetProfileId),
    Value(lir::AbiValueError),
}

impl From<lir::AbiValueError> for AbiPlanError {
    fn from(error: lir::AbiValueError) -> Self {
        Self::Value(error)
    }
}
impl std::fmt::Display for AbiPlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTarget(id) => write!(
                formatter,
                "unsupported ABI planning target {}/{}",
                id.canonical_name(),
                id.major_version()
            ),
            Self::Value(error) => write!(formatter, "{error:?}"),
        }
    }
}
impl std::error::Error for AbiPlanError {}
