# OpenSSL for Scoop

Version: **3.5.9**, from the OpenSSL 3.5 LTS branch, released 2026-09-29.

The unchanged complete upstream source archive is vendored as
`openssl-3.5.9.tar.gz`. Its license is also available as `LICENSE.txt`.
Source: https://github.com/openssl/openssl/releases/tag/openssl-3.5.9
License: Apache-2.0. Upstream maintenance policy:
https://openssl-library.org/policies/releasestrat/

Run `python3 scripts/build_openssl.py --target <triple>` from the repository.
The script invokes upstream Configure and make outside the repository, retaining
the extracted sources. It builds the default provider with normal assembly and
thread support, PIC on POSIX, and static libraries. Dynamic modules, DSO loading, engines,
automatic config loading, upstream applications, documentation and tests are
disabled. Darwin uses deployment target 13.0. GNU uses gcc; musl uses musl-gcc,
or the explicit `--cc` wrapper. Linux also needs the target architecture's Linux
UAPI headers (including linux/mman.h). A musl wrapper can add an isolated UAPI
include directory, as required by scoop.asyncio; it must not add GNU libc headers.
No host OpenSSL package or pkg-config is used.

Supported triples are aarch64-apple-darwin, x86_64-unknown-linux-gnu,
x86_64-unknown-linux-musl, x86_64-pc-windows-msvc-ucrt,
x86_64-w64-windows-llvm-mingw-msvcrt and x86_64-w64-windows-llvm-mingw-ucrt.
Prepare each target on its corresponding OS. Each target has independent objects
and generated headers. `--build-dir`, `--prefix` and `--jobs` select ordinary build
locations and concurrency; NMake builds serially. `--rebuild` explicitly forces
preparation. Unchanged inputs and existing outputs return before reading the
archive or invoking build tools. Selected tools, build environment and actual
external header inputs participate in update checks.

Windows Microsoft builds use upstream VC-WIN64A, native Windows Perl (such as
Strawberry Perl), NASM, NMake and clang-cl or cl in the selected x64 SDK/MSVC
environment. `/MD` overrides OpenSSL's static-library default `/MT`; the output
archives remain static while their CRT is dynamic UCRT. The upstream `.lib`
archives and public/generated headers are published directly, with archives
copied unchanged to the canonical `.a` names. A compiler PDB is copied when cl
produces it; clang-cl instead embeds its CodeView data in the object files.

Windows LLVM-MinGW builds use upstream mingw64 and require explicit `--cc` from
the prefix for the selected CRT. Use complete MSYS Perl, GNU Make and a Unix
shell; native Windows Perl is not suitable for this upstream configuration.
`--perl`, `--make` and `--shell` select these maintenance tools. A native Windows
GNU Make can also run with the selected MSYS shell and Perl. A stripped Perl
installation (for example Git for Windows without the needed Perl modules)
must first be completed. Compiler and archive tools come from the selected
LLVM-MinGW prefix. Applications do not depend on Perl, Make or MSYS.

Target packages include `sysroot/lib/<triple>/openssl/include` and `lib`.
Consumers pass `--library-path <sysroot>/lib/<triple>/openssl/lib`; Cone
manifests select matching headers and declare static crypto/ssl requirements.
The crypto Cone needs only libcrypto. TLS adds libssl. Neither archive is
implicitly embedded into a `.slib`; both are ordinary target package resources.
In particular, musl consumers do not install a distribution-specific OpenSSL.
