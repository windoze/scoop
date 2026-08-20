#include <cstdio>
#include <string>
static int initialized;
struct Global {
  Global() { initialized = 1; }
  ~Global() { std::puts("unexpected-cxx-destructor"); }
};
static Global global;
extern "C" int m33_cxx_initialized() {
  return initialized + static_cast<int>(std::string("ready").size());
}
