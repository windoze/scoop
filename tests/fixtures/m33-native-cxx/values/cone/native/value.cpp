#include "shared.h"
#include <cerrno>
#include <cstdio>
#include <scoop_rt.h>
#include <stdexcept>
#include <string>
#include <vector>
extern "C" int m33_c_value(void);
extern "C" int m33_lower(void);
extern "C" int m33_upper(void);
static int constructed;
struct Lifetime {
  Lifetime() { ++constructed; }
  ~Lifetime() { std::puts("cxx-finalize"); }
};
static Lifetime lifetime;
thread_local int slot = m33_shared_count + 1;
extern "C" int m33_cpp_value(void) {
  if (scoop_rt_thread_debug_mode() != 0 || constructed != 1 || slot != 7)
    return -1;
  try {
    const std::string message = "native exception";
    throw std::runtime_error(message);
  } catch (const std::runtime_error &error) {
    if (std::string(error.what()) != "native exception")
      return -2;
    std::vector<int> values{m33_c_value(), m33_lower(), m33_upper()};
    return m33_adjust(values[0]) + values[1] + values[2];
  }
}
extern "C" int m33_cpp_errno(void) {
  errno = E2BIG;
  return m33_adjust(41);
}
