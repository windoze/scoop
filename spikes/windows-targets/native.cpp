#include "native.hpp"

#include <cstdio>
#include <stdexcept>
#include <vector>

namespace PROBE_NAMESPACE {
static const std::vector<int> initialized = {7, 35};
struct Finalizer {
    ~Finalizer() { std::fputs("native destructor\n", stdout); }
};
static Finalizer finalizer;

Base::~Base() = default;
Value::Value(int number) : number(number) {}
std::string Value::label() const { return "value=" + std::to_string(number); }
int Value::fail() const { throw std::runtime_error(label()); }
Base *create() { return new Value(initialized[0] + initialized[1]); }
void destroy(Base *value) { delete value; }
} // namespace PROBE_NAMESPACE
