extern int m28_leaf(void);
_Thread_local int m28_tls = 7;
int m28_initialized;
__attribute__((constructor)) static void initialize(void) { m28_initialized = 3; }
int m28_old(void) { return 8; }
int m28_new(void) { return m28_leaf() + m28_tls; }
int m28_tls_read(void) { return m28_tls; }
__asm__(".symver m28_old,m28_value@M28_1");
__asm__(".symver m28_new,m28_value@@M28_2");
