#include "renderer.h"

namespace {
constexpr UINT frameMessage = WM_APP + 1;
constexpr wchar_t notificationClass[] = L"ScoopSurfaceD3DNotifications";

LRESULT CALLBACK notificationProcedure(HWND window, UINT message, WPARAM wp,
                                       LPARAM lp) {
  if (message == WM_NCCREATE) {
    const auto *creation = reinterpret_cast<CREATESTRUCTW *>(lp);
    SetWindowLongPtrW(window, GWLP_USERDATA,
                      reinterpret_cast<LONG_PTR>(creation->lpCreateParams));
  } else if (message == frameMessage) {
    auto *surface = reinterpret_cast<SSUSurface *>(
        GetWindowLongPtrW(window, GWLP_USERDATA));
    surface->notified(wp);
    return 0;
  }
  return DefWindowProcW(window, message, wp, lp);
}

void CALLBACK frameAvailable(PTP_CALLBACK_INSTANCE, void *context, PTP_WAIT,
                             TP_WAIT_RESULT) {
  auto *surface = static_cast<SSUSurface *>(context);
  // Only this native notification state is read by the threadpool worker.
  if (!PostMessageW(surface->notification, frameMessage,
                    surface->notificationGeneration.load(), 0))
    surface->notificationError.store(ssu_windows_error());
}
} // namespace

HRESULT SSUSurface::initialize() {
  WNDCLASSEXW type{};
  type.cbSize = sizeof(type);
  type.hInstance = GetModuleHandleW(nullptr);
  type.lpfnWndProc = notificationProcedure;
  type.lpszClassName = notificationClass;
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
    return ssu_windows_error();
  notification = CreateWindowExW(0, notificationClass, L"", 0, 0, 0, 0, 0,
                                 HWND_MESSAGE, nullptr, type.hInstance, this);
  if (!notification)
    return ssu_windows_error();
  wait = CreateThreadpoolWait(frameAvailable, this, nullptr);
  if (!wait)
    return ssu_windows_error();
  HRESULT result =
      host->composition->CreateTargetForHwnd(window, FALSE, &target);
  if (SUCCEEDED(result))
    result = host->composition->CreateVisual(&visual);
  if (SUCCEEDED(result))
    result = target->SetRoot(visual.Get());
  if (SUCCEEDED(result))
    result = host->composition->Commit();
  return result;
}

void SSUSurface::stopNotifications() {
  if (wait) {
    SetThreadpoolWait(wait, nullptr, nullptr);
    WaitForThreadpoolWaitCallbacks(wait, TRUE);
  }
  if (notification) {
    MSG message;
    while (PeekMessageW(&message, notification, frameMessage, frameMessage,
                        PM_REMOVE)) {
      // Retire messages before this HWND can be destroyed and reused.
    }
  }
  ++notificationGeneration;
  waiting = queued = ready = false;
}

HRESULT SSUSurface::updateDemand() {
  HRESULT error = notificationError.load();
  if (FAILED(error))
    return error;
  if (!required || width <= 0 || height <= 0) {
    if (waiting || queued || ready)
      stopNotifications();
    return S_OK;
  }
  if (active || ready || waiting || queued)
    return S_OK;
  if (!swapchain || width != bufferWidth || height != bufferHeight) {
    // A new configuration needs its first target before it has a latency
    // handle.
    if (!PostMessageW(notification, frameMessage, notificationGeneration.load(),
                      0))
      return ssu_windows_error();
    queued = true;
  } else {
    waiting = true;
    SetThreadpoolWait(wait, latency, nullptr);
  }
  return S_OK;
}

void SSUSurface::notified(UINT_PTR serial) {
  if (serial != notificationGeneration.load() || !required)
    return;
  waiting = queued = false;
  ready = true;
  if (!callback(context, 1)) {
    required = false;
    stopNotifications();
  }
}
