#include <cstdint>
#include <string>
#include <vector>

extern "C" int64_t cone_cpp_compute(int64_t a, int64_t b) {
  std::string s = "hi";
  std::vector<int> v;
  v.push_back(1);
  v.push_back(2);
  v.push_back(3);
  return a + b + (int64_t)s.size() + (int64_t)v.size();
}

