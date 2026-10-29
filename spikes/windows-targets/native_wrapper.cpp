#define PROBE_NAMESPACE static_library
#include "native.hpp"
#undef PROBE_NAMESPACE
#define PROBE_NAMESPACE dynamic_library
#define PROBE_DLL_IMPORT
#include "native.hpp"

#include <cassert>
#include <stdexcept>

extern "C" int native_probe(void) {
    auto *static_value = static_library::create();
    auto *dynamic_value = dynamic_library::create();
    int caught = 0;
    assert(dynamic_cast<static_library::Value *>(static_value) != nullptr);
    assert(dynamic_cast<dynamic_library::Value *>(dynamic_value) != nullptr);
    assert(static_value->label() == "value=42");
    assert(dynamic_value->label() == "value=42");
    try {
        dynamic_cast<static_library::Value *>(static_value)->fail();
    } catch (const std::runtime_error &error) {
        assert(std::string(error.what()) == "value=42");
        ++caught;
    }
    try {
        dynamic_cast<dynamic_library::Value *>(dynamic_value)->fail();
    } catch (const std::runtime_error &error) {
        assert(std::string(error.what()) == "value=42");
        ++caught;
    }
    static_library::destroy(static_value);
    dynamic_library::destroy(dynamic_value);
    return caught;
}
