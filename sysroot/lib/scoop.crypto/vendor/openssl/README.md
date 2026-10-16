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
thread support, PIC, and static libraries. Dynamic modules, DSO loading, engines,
automatic config loading, upstream applications, documentation and tests are
disabled. Darwin uses deployment target 13.0. GNU uses gcc; musl uses musl-gcc,
or the explicit `--cc` wrapper. Linux also needs the target architecture's Linux
UAPI headers (including linux/mman.h). A musl wrapper can add an isolated UAPI
include directory, as required by scoop.asyncio; it must not add GNU libc headers.
No host OpenSSL package or pkg-config is used.

Supported triples are aarch64-apple-darwin, x86_64-unknown-linux-gnu, and
x86_64-unknown-linux-musl. Prepare Darwin on Darwin and Linux on Linux. Each
target has independent objects and generated headers. `--build-dir`, `--prefix`
and `--jobs` select ordinary build locations and concurrency; `--rebuild` forces
preparation after changing the compiler or its environment. Unchanged inputs
and existing outputs return before reading the archive or invoking build tools.

Target packages include `sysroot/native/<triple>/openssl/include` and `lib`.
Consumers pass `--library-path <sysroot>/native/<triple>/openssl/lib`; Cone
manifests select matching headers and declare static crypto/ssl requirements.
The crypto Cone needs only libcrypto. TLS adds libssl. Neither archive is
implicitly embedded into a `.slib`; both are ordinary target package resources.
In particular, musl consumers do not install a distribution-specific OpenSSL.
