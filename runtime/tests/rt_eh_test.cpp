/* M8 exception runtime test (milestone8 DESIGN section 4, runtime spec
 * 5): verifies the full throw -> unwind -> catch chain through the C++
 * ABI. scoop_rt_throw (C) copies the object into an ABI exception
 * buffer (__cxa_allocate_exception) and throws the buffer via
 * __cxa_throw with a NULL type_info — throw-by-value semantics: a C++
 * catch-all handler must observe the buffer pointer (never the original
 * object pointer), with the object header TD and payload intact, so
 * is-instance checks work on the copy. scoop_rt_rethrow must propagate
 * the same buffer to an outer handler. Also links against
 * scoop_eh_personality, which generated code references as the
 * personality of functions with landing pads.
 *
 * M9: layouts carry the 16-byte object header ({ td, gc_word }), and
 * the last block verifies exception keep-alive under the GC
 * (milestone9 DESIGN 3.4): a reference payload of an in-flight
 * exception survives collections (the original is pinned and the ABI
 * buffer is registered as an external global root).
 *
 * Provides scoop_main; the runtime's own main() runs it after
 * scoop_rt_init_eh.
 *
 * Expected stdout:
 *   caught
 *   rethrown
 *   kept
 *
 * Build & run (rt.c/gc.c are C; linking needs the C++ ABI for __cxa_*
 * and the personality — the c++ driver adds it, otherwise -lc++abi):
 *   cc  -std=c11 -Wall -Wextra -I runtime/include -c runtime/src/rt.c -o /tmp/scoop_rt.o
 *   cc  -std=c11 -Wall -Wextra -I runtime/include -c runtime/src/gc.c -o /tmp/scoop_gc.o
 *   c++ -std=c++11 -Wall -Wextra -c runtime/tests/rt_eh_test.cpp -o /tmp/scoop_rt_eh_test.o
 *   c++ /tmp/scoop_rt.o /tmp/scoop_gc.o /tmp/scoop_rt_eh_test.o -o /tmp/scoop_rt_eh_test
 *   /tmp/scoop_rt_eh_test
 */
#include <cstdio>
#include <cstdlib>
#include <cstring>

extern "C" {
void *scoop_rt_alloc(const void *td, size_t size);
bool scoop_rt_is_instance(const void *obj, const void *td);
void scoop_rt_throw(const void *obj);
void scoop_rt_rethrow(void);
void scoop_rt_gc_collect(void);
unsigned long long scoop_rt_gc_stats(void);
int scoop_eh_personality(int version, unsigned int actions, unsigned long long exception_class,
                         void *exception, void *context);
}

/* Itanium C++ ABI: adjusted pointer of the exception currently being
 * handled (the handler's __cxa_begin_catch put it on the caught
 * stack). */
extern "C" void *__cxa_current_primary_exception();

/* Matches the layout of the TypeDescriptor globals emitted by codegen
 * (eight 8-byte slots, runtime spec 2.2; slot 1 is the instance size,
 * slot 3 the GC scan descriptor). scoop_td_String is referenced by
 * rt.c's string helpers. Its size is the fixed part (16-byte header +
 * len); this test never scans a String, so the descriptor stays 0. */
struct ScoopTypeDescriptorLayout {
    unsigned long long slots[8];
};
extern "C" const ScoopTypeDescriptorLayout scoop_td_String = {{1, 24, 8, 0, 0, 0, 0, 0}};

namespace {

/* Test exception type: instance size 24 = 16-byte object header + one
 * i64 payload slot. No references, so the scan descriptor is 0. */
const ScoopTypeDescriptorLayout exception_td = {{1000, 24, 8, 0, 0, 0, 0, 0}};
const size_t payload_offset = 16;

/* M9 keep-alive fixture: exception whose payload is a reference
 * (scan descriptor: plain table, one reference at offset 16). */
const unsigned long long ref_payload_scan[] = {1, 16};
const ScoopTypeDescriptorLayout ref_exception_td = {
    {2000, 24, 8, (unsigned long long)&ref_payload_scan[0], 0, 0, 0, 0}};

[[noreturn]] void fail(const char *what) {
    std::fprintf(stderr, "rt_eh_test: %s\n", what);
    std::exit(1);
}

const void *header_td(const void *obj) {
    return *reinterpret_cast<const void *const *>(obj);
}

long long payload(const void *obj) {
    return *reinterpret_cast<const long long *>(static_cast<const char *>(obj) + payload_offset);
}

void set_payload(void *obj, long long value) {
    *reinterpret_cast<long long *>(static_cast<char *>(obj) + payload_offset) = value;
}

void throw_and_rethrow(const void *obj, const void **inner_caught) {
    try {
        scoop_rt_throw(obj);
    } catch (...) {
        *inner_caught = __cxa_current_primary_exception();
        scoop_rt_rethrow();
    }
}

/* String object layout (16-byte header + len + inline data). */
void *make_string(const char *text, size_t len) {
    void *s = scoop_rt_alloc(&scoop_td_String, 24 + len);
    *reinterpret_cast<unsigned long long *>(static_cast<char *>(s) + 16) = len;
    std::memcpy(static_cast<char *>(s) + 24, text, len);
    return s;
}

/* Overwrite dead stack frames so the conservative stack scan (gc.c
 * v1) does not retain pointers that only lived in helper frames. */
void clobber_stack() {
    volatile unsigned long long buf[2048];
    for (size_t i = 0; i < 2048; i++) {
        buf[i] = 0;
    }
}

} // namespace

extern "C" void scoop_main(void) {
    void *original = scoop_rt_alloc(&exception_td, 24);
    if (original == nullptr) {
        fail("out of memory");
    }
    set_payload(original, 42);

    // Full chain: scoop_rt_throw -> buffer copy -> __cxa_throw ->
    // unwind -> catch-all. The handler observes the exception buffer,
    // not the original object (throw-by-value)...
    const void *caught = nullptr;
    try {
        scoop_rt_throw(original);
    } catch (...) {
        caught = __cxa_current_primary_exception();
    }
    if (caught == nullptr || caught == original) {
        fail("scoop_rt_throw: handler must observe the buffer copy, not the original");
    }
    // ...but the copy carries the identical TD and payload, and
    // is-instance works on it.
    if (header_td(caught) != &exception_td) {
        fail("scoop_rt_throw: copy lost the object header TD");
    }
    if (payload(caught) != 42) {
        fail("scoop_rt_throw: copy lost the payload");
    }
    if (!scoop_rt_is_instance(caught, &exception_td)) {
        fail("scoop_rt_throw: is-instance failed on the copy");
    }
    std::puts("caught");

    // Rethrow: the same exception buffer reaches the outer handler.
    const void *inner = nullptr;
    const void *recaught = nullptr;
    try {
        throw_and_rethrow(original, &inner);
    } catch (...) {
        recaught = __cxa_current_primary_exception();
    }
    if (inner == nullptr || recaught != inner) {
        fail("scoop_rt_rethrow: outer handler did not observe the same buffer");
    }
    if (header_td(recaught) != &exception_td || payload(recaught) != 42) {
        fail("scoop_rt_rethrow: buffer content changed");
    }
    std::puts("rethrown");

    // The personality function must be linked in; generated functions
    // with a landing pad reference it.
    using PersonalityFn = int (*)(int, unsigned int, unsigned long long, void *, void *);
    PersonalityFn personality = &scoop_eh_personality;
    if (personality == nullptr) {
        fail("scoop_eh_personality missing");
    }

    // M9 (DESIGN 3.4): a reference payload of an in-flight exception
    // survives collections. The string is referenced only from the
    // exception payload; the throw pins the original and registers the
    // ABI buffer as an external global root.
    void *ref_ex = scoop_rt_alloc(&ref_exception_td, 24);
    void *kept = make_string("zombie", 6);
    *reinterpret_cast<void **>(static_cast<char *>(ref_ex) + payload_offset) = kept;
    kept = nullptr;
    clobber_stack();
    try {
        scoop_rt_throw(ref_ex);
    } catch (...) {
        const void *in_flight = __cxa_current_primary_exception();
        scoop_rt_gc_collect();
        scoop_rt_gc_collect();
        const char *payload_data =
            *reinterpret_cast<char *const *>(static_cast<char *>(const_cast<void *>(in_flight)) +
                                             payload_offset);
        if (payload_data == nullptr) {
            fail("gc: in-flight exception lost its payload reference");
        }
        unsigned long long kept_len =
            *reinterpret_cast<const unsigned long long *>(payload_data + 16);
        if (kept_len != 6 || std::memcmp(payload_data + 24, "zombie", 6) != 0) {
            fail("gc: in-flight exception payload string was collected");
        }
        // Collect only while the exception is in flight: after the
        // handler exits, the C++ ABI frees the buffer while the v1
        // root registration stays (known conservative approximation,
        // DESIGN 3.4/6) — tracing it then would be a use-after-free.
    }
    std::puts("kept");
}
