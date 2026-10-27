# Windows backend experiments (W0)

These small native programs qualify the mechanisms required by
[the Windows design](../../docs/windows-targets/DESIGN.md). They do not enable a
Scoop target or substitute for the eventual compiler/runtime/cone tests. Run on
Windows, keeping builds and raw reports outside the repository.

## Scoop SEH personality

`eh.ll` emits real `invoke`, catch-all, cleanup and `resume` with Scoop's SEH
entry. `eh.c` checks the exception pointer, selector, cleanup and single deletion.
The program links the production personality and LSDA decoder. The existing
decoder tests exercise malformed tables and the whole target-sized null entry.

Use LLVM 22.1.8. For the Microsoft environment, from a Build Tools environment
or with LLVM's normal installed SDK discovery:

```powershell
python spikes/windows-targets/build_msvc_unwind.py --llvm $llvm --source $llvmSource --build-dir $work/msvc-unwind
python spikes/windows-targets/run_eh.py --target msvc-ucrt --llvm $llvm --unwind $work/msvc-unwind --build-dir $work/eh-msvc
```

The experimental build uses upstream libunwind sources, MSVC headers, `/MD` and
`__SEH__`. It does not replace MSVC C++ EH. Upstream's CMake entry rejects MSVC
targets categorically; the production preparation recipe will need a scoped
build patch for this distinct Level-I use case.

For each LLVM-MinGW 20260616 prefix, prepare a distinct `$unwind` containing its
`include/*unwind*.h` headers and `x86_64-w64-mingw32/lib/libunwind.a` under
`$unwind/lib`. The separate header directory puts the matching libunwind header
before Clang's builtin `unwind.h`, which lacks the SEH helper declaration.

```powershell
python spikes/windows-targets/run_eh.py --target mingw-ucrt --llvm $ucrt --unwind $ucrtUnwind --build-dir $work/eh-ucrt
python spikes/windows-targets/run_eh.py --target mingw-msvcrt --llvm $msvcrt --unwind $msvcrtUnwind --build-dir $work/eh-msvcrt
```

The driver checks compile diagnostics, runs the decoder and PE executable, then
checks its actual CRT imports. `pe.txt` retains imports and unwind information in
the external build directory. This probe has no managed heap or GC handles;
full exception identity, moving GC and callback testing belongs to W2.
