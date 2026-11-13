#include "llvm-c/Core.h"
#include "llvm-c/TargetMachine.h"
#include "llvm/IR/Module.h"
#include "llvm/Passes/PassBuilder.h"
#include "llvm/ProfileData/InstrProfReader.h"
#include "llvm/Support/Error.h"
#include "llvm/Support/MemoryBuffer.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Transforms/Instrumentation/InstrProfiling.h"

extern "C" void scoop_lower_profile_counters(LLVMModuleRef module,
                                             LLVMTargetMachineRef target) {
  auto *ir = llvm::unwrap(module);
  // Scoop links compiler-rt explicitly and owns its initialization and flush.
  // The declaration tells InstrProfiling not to emit a per-module hook body.
  ir->getOrInsertGlobal("__llvm_profile_runtime",
                        llvm::Type::getInt32Ty(ir->getContext()));
  auto *machine = reinterpret_cast<llvm::TargetMachine *>(target);
  llvm::LoopAnalysisManager loops;
  llvm::FunctionAnalysisManager functions;
  llvm::CGSCCAnalysisManager callGraph;
  llvm::ModuleAnalysisManager modules;
  llvm::PassBuilder builder(machine);
  builder.registerModuleAnalyses(modules);
  builder.registerCGSCCAnalyses(callGraph);
  builder.registerFunctionAnalyses(functions);
  builder.registerLoopAnalyses(loops);
  builder.crossRegisterProxies(loops, functions, callGraph, modules);

  llvm::InstrProfOptions options;
  options.Atomic = true;
  llvm::ModulePassManager passes;
  passes.addPass(llvm::InstrProfilingLoweringPass(options));
  passes.run(*ir, modules);
}

using ScoopProfileRecordSink = void (*)(void *, const char *, size_t, uint64_t,
                                        const uint64_t *, size_t);

extern "C" char *scoop_read_indexed_profile(const char *data, size_t size,
                                            void *context,
                                            ScoopProfileRecordSink sink) {
  // The Rust caller owns these bytes until this synchronous call returns.
  auto buffer = llvm::MemoryBuffer::getMemBuffer(llvm::StringRef(data, size),
                                                 "Scoop profile", false);
  auto expected = llvm::IndexedInstrProfReader::create(std::move(buffer));
  if (!expected)
    return LLVMCreateMessage(llvm::toString(expected.takeError()).c_str());
  auto &reader = **expected;
  for (const auto &record : reader)
    sink(context, record.Name.data(), record.Name.size(), record.Hash,
         record.Counts.data(), record.Counts.size());
  if (auto error = reader.getError())
    return LLVMCreateMessage(llvm::toString(std::move(error)).c_str());
  return nullptr;
}
