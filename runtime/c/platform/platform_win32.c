// Windows backend placeholder for Scoop C runtime platform layer (v0).
//
// 说明：
// - TODO T1402 的 v0 仅要求提供“占位接口与 build gate”，不要求实现 Windows 行为。
// - 这些实现统一返回失败，以便上层通过 capability gating 或稳定诊断处理。
// - 所有 API 必须是 `static`，避免成为对外导出符号（见 T1401）。

static SCOOP_PLATFORM_UNUSED const char *scoop_platform_env_getenv(const char *key_cstr) {
  (void)key_cstr;
  return 0;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_time_now_unix_millis(int64_t *out_unix_millis) {
  (void)out_unix_millis;
  return 0;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_write_stdout_all(const uint8_t *buf,
                                                                    size_t len) {
  (void)buf;
  (void)len;
  return 0;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_write_stderr_all(const uint8_t *buf,
                                                                    size_t len) {
  (void)buf;
  (void)len;
  return 0;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_write_stdout_byte(uint8_t byte) {
  (void)byte;
  return 0;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_write_stderr_byte(uint8_t byte) {
  (void)byte;
  return 0;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_read_stdin(uint8_t *buf,
                                                             size_t len,
                                                             size_t *out_nread) {
  if (out_nread != 0) {
    *out_nread = 0;
  }
  (void)buf;
  (void)len;
  return 0;
}

// --- dynlib (placeholder) ---
//
// Windows backend 尚未实现：统一返回 NULL。
static SCOOP_PLATFORM_UNUSED void *scoop_platform_dynlib_lookup_symbol_default(
    const char *symbol_name_cstr) {
  (void)symbol_name_cstr;
  return 0;
}

// --- sync/thread (placeholder) ---
//
// 说明：
// - Windows backend 尚未实现；这些接口统一返回失败或 no-op。
// - 上层应通过 capability gating 或稳定诊断处理“不支持”。

static SCOOP_PLATFORM_UNUSED int scoop_platform_sync_mutex_init(ScoopPlatformMutex *mutex) {
  (void)mutex;
  return 0;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_mutex_lock(ScoopPlatformMutex *mutex) {
  (void)mutex;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_mutex_unlock(ScoopPlatformMutex *mutex) {
  (void)mutex;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_mutex_destroy(ScoopPlatformMutex *mutex) {
  (void)mutex;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_sync_condvar_init(ScoopPlatformCondVar *condvar) {
  (void)condvar;
  return 0;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_condvar_wait(ScoopPlatformCondVar *condvar,
                                                                  ScoopPlatformMutex *mutex) {
  (void)condvar;
  (void)mutex;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_condvar_signal(ScoopPlatformCondVar *condvar) {
  (void)condvar;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_condvar_broadcast(
    ScoopPlatformCondVar *condvar) {
  (void)condvar;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_condvar_destroy(ScoopPlatformCondVar *condvar) {
  (void)condvar;
}

static SCOOP_PLATFORM_UNUSED ScoopPlatformThread scoop_platform_thread_self(void) {
  return 0;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_thread_equal(ScoopPlatformThread a,
                                                            ScoopPlatformThread b) {
  (void)a;
  (void)b;
  return 0;
}

// --- thread primitives (placeholder) ---

static SCOOP_PLATFORM_UNUSED int scoop_platform_thread_spawn(ScoopPlatformThread *out_thread,
                                                            ScoopPlatformThreadEntryFn entry,
                                                            void *arg) {
  if (out_thread != 0) {
    *out_thread = 0;
  }
  (void)entry;
  (void)arg;
  return 0;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_thread_join(ScoopPlatformThread thread) {
  (void)thread;
  return 0;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_thread_yield(void) {}

static SCOOP_PLATFORM_UNUSED void scoop_platform_thread_sleep_millis(int64_t ms) { (void)ms; }

static SCOOP_PLATFORM_UNUSED int64_t scoop_platform_thread_current_id(void) { return 0; }
