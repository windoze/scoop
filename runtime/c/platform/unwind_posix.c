// POSIX backend for Scoop C runtime platform unwind layer (v0).
//
// 说明：
// - 这里优先使用 `_Unwind_Backtrace`：它属于 Itanium unwind ABI 的一部分，
//   在我们主线 host 平台（clang toolchain）上通常无需额外链接选项；
// - 该实现仅用于 current-thread backtrace；不提供跨线程/remote unwind。

#include <setjmp.h>
#include <stdlib.h>
#include <unwind.h>

typedef struct ScoopPlatformUnwindOpaqueCtx {
  // v0：用 `setjmp` 捕获当前线程的寄存器快照，作为后续 stack walking 的起点（T1505b/T1411b）。
  //
  // 说明：
  // - `ucontext_t/getcontext` 在 macOS 上需要额外 feature macro（且已废弃），因此这里先用
  //   标准 C 的 `jmp_buf` 作为可移植的 opaque ctx；
  // - 该结构的具体解读与“从 ctx 开始逐帧 unwind”仍由后续任务 T1411b 统一在 platform 层落地。
  jmp_buf regs;
} ScoopPlatformUnwindOpaqueCtx;

typedef struct ScoopUnwindCaptureCtx {
  uintptr_t *out_ips;
  uint32_t cap;
  uint32_t len;
  uint32_t skip;
} ScoopUnwindCaptureCtx;

static _Unwind_Reason_Code scoop_unwind_capture_cb(struct _Unwind_Context *context, void *arg) {
  if (arg == 0) {
    return _URC_END_OF_STACK;
  }

  ScoopUnwindCaptureCtx *ctx = (ScoopUnwindCaptureCtx *)arg;
  if (ctx->out_ips == 0 || ctx->cap == 0) {
    return _URC_END_OF_STACK;
  }

  // `_Unwind_GetIP` 返回的是“下一条指令地址”（实现相关）；只用于诊断/定位而非执行。
  uintptr_t ip = (uintptr_t)_Unwind_GetIP(context);
  if (ip == 0) {
    return _URC_NO_REASON;
  }

  if (ctx->skip > 0) {
    ctx->skip--;
    return _URC_NO_REASON;
  }

  if (ctx->len >= ctx->cap) {
    return _URC_END_OF_STACK;
  }

  ctx->out_ips[ctx->len] = ip;
  ctx->len++;
  return _URC_NO_REASON;
}

static SCOOP_UNWIND_UNUSED uint32_t scoop_platform_unwind_capture_ips(uintptr_t *out_ips,
                                                                      uint32_t out_cap,
                                                                      uint32_t skip_frames) {
  if (out_ips == 0 || out_cap == 0) {
    return 0;
  }

  ScoopUnwindCaptureCtx ctx = {
      .out_ips = out_ips,
      .cap = out_cap,
      .len = 0,
      .skip = skip_frames,
  };

  (void)_Unwind_Backtrace(scoop_unwind_capture_cb, (void *)&ctx);
  return ctx.len;
}

static SCOOP_UNWIND_UNUSED void *scoop_platform_unwind_ctx_capture(void) {
  ScoopPlatformUnwindOpaqueCtx *out =
      (ScoopPlatformUnwindOpaqueCtx *)malloc(sizeof(ScoopPlatformUnwindOpaqueCtx));
  if (out == 0) {
    return 0;
  }

  // `setjmp` 返回 0 表示“直接保存”，非 0 表示“从 longjmp 恢复”。
  // 我们只需要保存快照，因此忽略返回值。
  (void)setjmp(out->regs);

  return (void *)out;
}

static SCOOP_UNWIND_UNUSED void scoop_platform_unwind_ctx_destroy(void *ctx) {
  if (ctx == 0) {
    return;
  }

  free(ctx);
}
