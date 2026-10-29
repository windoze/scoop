# LLVM 22.1.8

M52 pins LLVM and libunwind to LLVM 22.1.8. Obtain the unmodified upstream source archive from:

https://github.com/llvm/llvm-project/releases/download/llvmorg-22.1.8/llvm-project-22.1.8.src.tar.xz

Upstream SHA-256: `922f1817a0df7b1489272d18134ee0087a8b068828f87ac63b9861b1a9965888`.

The local source directory is `llvm-project-22.1.8.src/`. The archive and expanded source are build inputs and are not committed. Host LLVM uses an out-of-source build. Its preparation script applies `patches/llvm-windows-statepoint-return-pc.patch` to the selected external source tree: X86 statepoint labels precede WinEH NOP padding so stack maps contain actual return PCs. Every host that emits Windows objects needs this fix. The libunwind preparation script separately applies `patches/llvm-libunwind-eh-index-miss.patch` to its external build copy, leaving its upstream input unchanged.

Binary packages carry the installed host libraries and target libunwind artifacts. The complete upstream source and these patches are distributed in the matching vendor source archive, under the same versioned directory. LLVM's Apache-2.0-with-LLVM-exception licenses accompany both distributions.

`scripts/build_windows_unwind.py` builds each Windows target's static libunwind from this source. For the Microsoft target, `patches/llvm-libunwind-msvc-seh.patch` adds an explicit clang-cl x64 static Level-I SEH build option. It preserves exception metadata on the C unwinding functions and disables C++ exceptions and RTTI within libunwind. It does not provide an Itanium C++ ABI on MSVC targets or replace VCRuntime's C++ exception handler. The patch is applied only to the preparation work directory; Microsoft objects use `/MD`, and MinGW objects use the selected CRT prefix.

The host SDK also includes Clang headers, clang-cpp, resource headers and debug-file
tools. `scripts/build_lldb.py` builds Python-enabled LLDB and applies
`scripts/patches/lldb-scoop-source-names.patch` to its own source copy. This local
compatibility patch preserves Scoop source names without dropping exact linkage
names. The matching source package includes this patch in its `patches/` folder.
