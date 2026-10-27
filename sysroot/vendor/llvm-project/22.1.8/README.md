# LLVM 22.1.8

M52 pins LLVM and libunwind to LLVM 22.1.8. Obtain the unmodified upstream source archive from:

https://github.com/llvm/llvm-project/releases/download/llvmorg-22.1.8/llvm-project-22.1.8.src.tar.xz

Upstream SHA-256: `922f1817a0df7b1489272d18134ee0087a8b068828f87ac63b9861b1a9965888`.

The local source directory is `llvm-project-22.1.8.src/`. The archive and expanded source are build inputs and are not committed. Host LLVM uses an out-of-source build. Its preparation script applies `patches/llvm-windows-statepoint-return-pc.patch` to the selected external source tree: X86 statepoint labels precede WinEH NOP padding so stack maps contain actual return PCs. Every host that emits Windows objects needs this fix. The libunwind preparation script separately applies `patches/llvm-libunwind-eh-index-miss.patch` to its external build copy, leaving its upstream input unchanged.

Binary packages carry the installed host libraries and target libunwind artifacts. The complete upstream source and these patches are distributed in the matching vendor source archive, under the same versioned directory. LLVM's Apache-2.0-with-LLVM-exception licenses accompany both distributions.
