#if C_ONLY != 40 || defined(CXX_ONLY) || defined(__cplusplus)
#error "incorrect C flags"
#endif
int m33_c_value(void) { return C_ONLY + _Generic(0, int: 1, default: 0); }
