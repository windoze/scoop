#include <cerrno>
#include <cmath>
#include <cstdint>
#include <stdexcept>
#include <string>
#include <vector>
static const std::string prefix = "native";
extern "C" int32_t m33_provider_sum(int32_t value) {
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
