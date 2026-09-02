# runtime

C runtime: Immix-core GC (`src/gc.c`, M9 — block/line heap, mark-region
collection, pin/handle; see `docs/milestone9/DESIGN.md` section 2),
M13 pthread registration, TLS thread state, cooperative STW epochs and
native-safe/native-borrowed transition roots (`src/thread.c`),
target-selected platform components (`src/platform/`; M15 currently selects
only `os/darwin.c`),
managed foreign-callback token registry and gateway (`src/callback.c`),
runtime entry points and backing implementations of core types
(`src/rt.c`). See `docs/specs/SCOOP-RUNTIME-SPEC.md`.

The driver (`scoopc`) compiles the source set carried by the selected target
profile into `target/scoop-rt/
libscoop_rt.a` (via the `cc` crate) and links it into every compiled
program. Standalone test builds:

```
cc -std=c11 -Wall -Wextra -pthread -I runtime/include runtime/src/rt.c runtime/src/gc.c runtime/src/thread.c runtime/src/callback.c runtime/src/platform/os/darwin.c runtime/tests/rt_test.c -o /tmp/scoop_rt_test -lc++abi
/tmp/scoop_rt_test   # stdout must match runtime/tests/rt_test_expected.txt

cc  -std=c11 -Wall -Wextra -I runtime/include -c runtime/src/rt.c -o /tmp/scoop_rt.o
cc  -std=c11 -Wall -Wextra -I runtime/include -c runtime/src/gc.c -o /tmp/scoop_gc.o
cc  -std=c11 -Wall -Wextra -pthread -I runtime/include -c runtime/src/thread.c -o /tmp/scoop_thread.o
cc  -std=c11 -Wall -Wextra -pthread -I runtime/include -c runtime/src/callback.c -o /tmp/scoop_callback.o
cc  -std=c11 -Wall -Wextra -pthread -I runtime/include -c runtime/src/platform/os/darwin.c -o /tmp/scoop_platform_darwin.o
c++ -std=c++11 -Wall -Wextra -c runtime/tests/rt_eh_test.cpp -o /tmp/scoop_rt_eh_test.o
c++ -pthread /tmp/scoop_rt.o /tmp/scoop_gc.o /tmp/scoop_thread.o /tmp/scoop_callback.o /tmp/scoop_platform_darwin.o /tmp/scoop_rt_eh_test.o -o /tmp/scoop_rt_eh_test
/tmp/scoop_rt_eh_test
```
