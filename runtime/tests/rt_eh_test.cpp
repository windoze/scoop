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
 * Provides scoop_main; the runtime's own main() runs it after
 * scoop_rt_init_eh.
 *
 * Expected stdout:
 *   caught
 *   rethrown
 *
 * Build & run (rt.c is C; linking needs the C++ ABI for __cxa_* and the
 * personality — the c++ driver adds it, otherwise use -lc++abi):
 *   cc  -std=c11 -Wall -Wextra -I runtime/include -c runtime/src/rt.c -o /tmp/scoop_rt.o
 *   c++ -std=c++11 -Wall -Wextra -c runtime/tests/rt_eh_test.cpp -o /tmp/scoop_rt_eh_test.o
 *   c++ /tmp/scoop_rt.o /tmp/scoop_rt_eh_test.o -o /tmp/scoop_rt_eh_test
 *   /tmp/scoop_rt_eh_test
 */
#include <cstdio>
#include <cstdlib>

extern "C" {
void *scoop_rt_alloc(const void *td, size_t size);
bool scoop_rt_is_instance(const void *obj, const void *td);
void scoop_rt_throw(const void *obj);
void scoop_rt_rethrow(void);
int scoop_eh_personality(int version, unsigned int actions, unsigned long long exception_class,
                         void *exception, void *context);
}

/* Itanium C++ ABI: adjusted pointer of the exception currently being
 * handled (the handler's __cxa_begin_catch put it on the caught
 * stack). */
extern "C" void *__cxa_current_primary_exception();

/* Matches the layout of the TypeDescriptor globals emitted by codegen
 * (eight 8-byte slots, runtime spec 2.2; slot 1 is the instance size).
 * scoop_td_String is referenced by rt.c's string helpers. */
struct ScoopTypeDescriptorLayout {
    unsigned long long slots[8];
};
extern "C" const ScoopTypeDescriptorLayout scoop_td_String = {{1, 16, 8, 0, 0, 0, 0, 0}};

namespace {

/* Test exception type: instance size 16 = object header (8) + one i64
 * payload slot. */
const ScoopTypeDescriptorLayout exception_td = {{1000, 16, 8, 0, 0, 0, 0, 0}};
const size_t payload_offset = 8;

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

} // namespace

extern "C" void scoop_main(void) {
    void *original = scoop_rt_alloc(&exception_td, 16);
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
}
