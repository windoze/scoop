#ifndef M33_CXX_SHARED_H
#define M33_CXX_SHARED_H
#if !CXX_ONLY || defined(C_ONLY)
#error "incorrect C++ flags"
#endif
static_assert(__cplusplus >= 202002L);
inline int m33_shared_count = 6;
template <class T> __attribute__((noinline)) T m33_adjust(T value) {
  return value + 1;
}
#endif
