#if !C_ONLY || defined(OBJC_ONLY) || defined(__OBJC__) || defined(__cplusplus)
#error incorrect C configuration
#endif
int scoop_objc_from_c(void) { return _Generic(0, int: 1, default: 0); }
