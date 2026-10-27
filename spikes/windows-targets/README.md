# Windows backend experiments (W0)

These small native programs qualify the mechanisms required by
[the Windows design](../../docs/windows-targets/DESIGN.md). They do not enable a
Scoop target or substitute for the eventual compiler/runtime/cone tests. Run on
Windows, keeping builds and raw reports outside the repository.

## Scoop SEH personality

`eh.ll` emits real statepoint `invoke`, catch-all, cleanup and `resume` with Scoop's SEH
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

## Frame context and relocated roots

```powershell
python spikes/windows-targets/run_roots.py --llvm $llvm --build-dir $work/roots-msvc
python spikes/windows-targets/run_roots.py --llvm $ucrt --build-dir $work/roots-ucrt
python spikes/windows-targets/run_roots.py --llvm $msvcrt --build-dir $work/roots-msvcrt
```

The probe links the production stackmap reader, reads the loaded PE's stackmap,
and uses `RtlVirtualUnwind` to recover the statepoint caller's context. It updates
the exact recorded roots and checks that the LLVM function returns the relocated
pointer. Both frame-pointer modes use a 16 KiB frame and real stack probing.
The IR models the compiler's post-RS4GC scalar root and explicit zero-live invoke
forms. It does not claim to run the full compiler or collector.

## Native C++ and Scoop SEH in one executable

```powershell
python spikes/windows-targets/run_native.py --target msvc-ucrt --llvm $llvm --unwind $work/msvc-unwind --build-dir $work/native-msvc --toolset $toolset --sdk $sdk --sdk-version 10.0.26100.0
python spikes/windows-targets/run_native.py --target mingw-ucrt --llvm $ucrt --unwind $ucrtUnwind --build-dir $work/native-ucrt
python spikes/windows-targets/run_native.py --target mingw-msvcrt --llvm $msvcrt --unwind $msvcrtUnwind --build-dir $work/native-msvcrt
```

`$toolset` is the versioned MSVC directory, e.g. `VC/Tools/MSVC/14.44.35207`;
`$sdk` is `Windows Kits/10`. This experiment supplies those real include and
library roots explicitly. MSVC builds the same static library and DLL once with
cl, then tests wrappers built with cl/link.exe and clang-cl/lld-link. MinGW uses
its matching libc++ environment. Each case checks STL values, class RTTI, native
exceptions, static initialization/destruction, Win32 threads, CRT TLS/errno and
an independent C locale, alongside the production Scoop personality. Raw
commands and imports stay in the build directory.

## Rust text dependency closure

Install the Rust 1.99.1 standard libraries for `x86_64-pc-windows-msvc`,
`x86_64-pc-windows-gnullvm` and `x86_64-pc-windows-gnu`. Extract
`sysroot/vendor/text_backend/crates.tar.gz` into `$vendorParent`; the extracted
`crates` directory is `$vendor`. Then run:

```powershell
python spikes/windows-targets/run_text.py --target msvc-ucrt --llvm $llvm --unwind $work/msvc-unwind --vendor $vendor --cargo-target-dir $work/text-target --build-dir $work/text-msvc
python spikes/windows-targets/run_text.py --target mingw-ucrt --llvm $ucrt --unwind $ucrtUnwind --vendor $vendor --cargo-target-dir $work/text-target --build-dir $work/text-ucrt
python spikes/windows-targets/run_text.py --target mingw-msvcrt --llvm $msvcrt --unwind $msvcrtUnwind --vendor $vendor --cargo-target-dir $work/text-target --build-dir $work/text-msvcrt
```

Cargo reuses its ordinary build results. Each probe consumes the actual locked
text backend, links Rust's reported native dependencies, exercises owned text
results and converter lifetimes, and throws through Scoop's SEH handler. Explicit
qualification checks the final CRT imports and absence of a second Level-I
provider in the text archive. Rust 1.99.1's GNU staticlib closes with the selected
MSVCRT LLVM-MinGW environment; this validated version needs no rebuilt std.
