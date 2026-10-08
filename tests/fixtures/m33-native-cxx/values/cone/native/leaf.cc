#include "shared.h"
#include <scoop_rt.h>
extern "C" int m33_cpp_leaf(void) {
  return scoop_rt_thread_debug_mode() == 1
             ? m33_adjust(41) + m33_shared_count - 6
             : -1;
}
