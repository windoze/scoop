// The C API always constructs a complete machine-code pipeline, even for data.
#include "llvm-c/Core.h"
#include "llvm-c/TargetMachine.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/CodeGen/MachineModuleInfo.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/CodeGen.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"

extern "C" LLVMMemoryBufferRef
scoop_emit_metadata_object(LLVMTargetMachineRef target, LLVMModuleRef module) {
  auto &machine = *reinterpret_cast<llvm::TargetMachine *>(target);
  llvm::SmallVector<char, 0> bytes;
  llvm::raw_svector_ostream output(bytes);
  llvm::legacy::PassManager passes;
  auto *moduleInfo = new llvm::MachineModuleInfoWrapperPass(&machine);
  passes.add(moduleInfo);
  if (machine.addAsmPrinter(passes, output, nullptr,
                            llvm::CodeGenFileType::ObjectFile,
                            moduleInfo->getMMI().getContext()))
    return nullptr;
  // The metadata partition contains globals, aliases and function declarations.
  // AsmPrinter still owns target layout, TLS, debug info and inline assembly.
  passes.run(*llvm::unwrap(module));
  return LLVMCreateMemoryBufferWithMemoryRangeCopy(bytes.data(), bytes.size(),
                                                   "scoop metadata");
}
