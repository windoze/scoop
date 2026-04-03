// Windows backend placeholder for Scoop C runtime platform unwind layer (v0).
//
// 说明：
// - TODO T1411a 的 v0 仅要求提供“占位接口与 build gate”，不要求实现 Windows 行为；
// - 该 backend 统一返回 0，以便上层通过 capability gating 或稳定诊断处理。

static SCOOP_UNWIND_UNUSED uint32_t scoop_platform_unwind_capture_ips(uintptr_t *out_ips,
                                                                      uint32_t out_cap,
                                                                      uint32_t skip_frames) {
  (void)out_ips;
  (void)out_cap;
  (void)skip_frames;
  return 0;
}

static SCOOP_UNWIND_UNUSED void *scoop_platform_unwind_ctx_capture(void) { return 0; }

static SCOOP_UNWIND_UNUSED void scoop_platform_unwind_ctx_destroy(void *ctx) { (void)ctx; }

static SCOOP_UNWIND_UNUSED uint32_t scoop_platform_unwind_ctx_walk_frames(
    void *ctx,
    uint32_t skip_frames,
    ScoopPlatformUnwindFrameVisitor visitor,
    void *user_data) {
  (void)ctx;
  (void)skip_frames;
  (void)visitor;
  (void)user_data;
  return 0;
}
