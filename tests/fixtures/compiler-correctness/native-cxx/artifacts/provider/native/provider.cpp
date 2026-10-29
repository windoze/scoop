#include <cerrno>
#include <cmath>
#include <cstdint>
#include <stdexcept>
#include <string>
#include <vector>
#ifdef _WIN32
#include <windows.h>
#endif

struct Base {
  virtual ~Base() = default;
};
struct Derived : Base {};
static const std::string prefix = "native";
extern "C" int32_t m33_provider_sum(int32_t value) {
#ifdef _WIN32
  if (GetCurrentProcessId() == 0)
    return -2;
#endif
  Derived instance;
  Base *base = &instance;
  if (dynamic_cast<Derived *>(base) == nullptr)
    return -3;
  int32_t result;
  try {
    throw std::runtime_error(prefix);
  } catch (const std::runtime_error &error) {
    volatile double argument = 0.0;
    std::vector<int32_t> values{value, PROVIDER_OFFSET};
    result = std::string(error.what()) == "native" && std::cos(argument) == 1.0
                 ? values[0] + values[1]
                 : -1;
  }
  errno = 17;
  return result;
}
