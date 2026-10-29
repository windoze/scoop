// LLVM's C API cannot prepend a DWARF expression or replace a debug record.
#include "llvm/ADT/STLExtras.h"
#include "llvm/Analysis/CFG.h"
#include "llvm/IR/DebugInfoMetadata.h"
#include "llvm/IR/DebugProgramInstruction.h"
#include "llvm/IR/Dominators.h"
#include "llvm/IR/IntrinsicInst.h"
#include "llvm/IR/Module.h"

static llvm::Value *relocationOrigin(llvm::Value *value) {
  while (auto *relocation = llvm::dyn_cast<llvm::GCRelocateInst>(value))
    value = relocation->getDerivedPtr();
  return value;
}

static void relocateValues(llvm::Function &function) {
  llvm::DenseMap<llvm::Value *, llvm::SmallVector<llvm::GCRelocateInst *>>
      versions;
  for (auto &block : function)
    for (auto &instruction : block)
      if (auto *relocation = llvm::dyn_cast<llvm::GCRelocateInst>(&instruction))
        versions[relocationOrigin(relocation)].push_back(relocation);
  if (versions.empty())
    return;
  llvm::DominatorTree dominators(function);
  for (auto &block : function)
    for (auto &instruction : block)
      for (auto &record : instruction.getDbgRecordRange()) {
        auto *variable = llvm::dyn_cast<llvm::DbgVariableRecord>(&record);
        if (!variable || !variable->isDbgValue())
          continue;
        for (unsigned index = 0; index < variable->getNumVariableLocationOps();
             ++index) {
          auto *value = variable->getVariableLocationOp(index);
          auto found = versions.find(relocationOrigin(value));
          if (found == versions.end())
            continue;
          llvm::Value *current = value;
          bool ambiguous = false;
          for (auto *relocation : found->second) {
            if (relocation == &instruction ||
                dominators.dominates(&instruction, relocation))
              continue;
            if (!dominators.dominates(relocation, &instruction)) {
              ambiguous |= llvm::isPotentiallyReachable(
                  relocation, &instruction, nullptr, &dominators);
              continue;
            }
            auto *previous = llvm::dyn_cast<llvm::Instruction>(current);
            if (!previous || dominators.dominates(previous, relocation))
              current = relocation;
          }
          // RS4GC rewrites real users but can leave metadata on the pre-GC
          // SSA value. Reuse a dominating relocation; never add a debug-only
          // phi, load or root to reconstruct an ambiguous merge.
          variable->replaceVariableLocationOp(
              index,
              ambiguous ? llvm::UndefValue::get(value->getType()) : current);
        }
      }
}

extern "C" void scoop_debug_finalize_values(LLVMModuleRef module) {
  llvm::Module &m = *llvm::unwrap(module);
  if (!m.getNamedMetadata("llvm.dbg.cu"))
    return;
  for (auto &function : m) {
    for (auto &block : function)
      for (auto &instruction : block)
        for (auto &record :
             llvm::make_early_inc_range(instruction.getDbgRecordRange())) {
          auto *variable = llvm::dyn_cast<llvm::DbgVariableRecord>(&record);
          if (!variable || !variable->isDbgDeclare())
            continue;
          // Keep SROA's real fragments, but use one location-list
          // representation so explicit unavailable records can terminate a
          // managed observation.
          auto *expression = llvm::DIExpression::prepend(
              variable->getExpression(), llvm::DIExpression::DerefBefore);
          llvm::DbgVariableRecord::createDbgVariableRecord(
              variable->getVariableLocationOp(0), variable->getVariable(),
              expression, variable->getDebugLoc(), *variable);
          variable->eraseFromParent();
        }
    relocateValues(function);
  }
}
