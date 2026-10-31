#include "renderer.h"

#include <cmath>

namespace {
constexpr UINT swapchainFlags =
    DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT;
}

SGUSurface::~SGUSurface() {
  stopNotifications();
  if (wait)
    CloseThreadpoolWait(wait);
  if (notification)
    DestroyWindow(notification);
  if (latency)
    CloseHandle(latency);
}

HRESULT SGUSurface::configure(int32_t newWidth, int32_t newHeight,
                              double newScale) {
  if (newWidth < 0 || newHeight < 0 || !std::isfinite(newScale) ||
      newScale <= 0)
    return E_INVALIDARG;
  if (width != newWidth || height != newHeight || scale != newScale) {
    stopNotifications();
    ++generation;
    width = newWidth;
    height = newHeight;
    scale = newScale;
  }
  return updateDemand();
}

HRESULT SGUSurface::prepareBuffers() {
  HRESULT result = host->device->GetDeviceRemovedReason();
  if (FAILED(result))
    return result;
  if (swapchain) {
    if (width == bufferWidth && height == bufferHeight)
      return S_OK;
    result = host->drain();
    if (SUCCEEDED(result))
      result = swapchain->ResizeBuffers(0, static_cast<UINT>(width),
                                        static_cast<UINT>(height),
                                        DXGI_FORMAT_UNKNOWN, swapchainFlags);
  } else {
    DXGI_SWAP_CHAIN_DESC1 description{};
    description.Width = static_cast<UINT>(width);
    description.Height = static_cast<UINT>(height);
    description.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    description.SampleDesc.Count = 1;
    description.BufferUsage =
        DXGI_USAGE_RENDER_TARGET_OUTPUT | DXGI_USAGE_SHADER_INPUT;
    description.BufferCount = 2;
    description.Scaling = DXGI_SCALING_STRETCH;
    description.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;
    description.AlphaMode = DXGI_ALPHA_MODE_PREMULTIPLIED;
    description.Flags = swapchainFlags;
    ComPtr<IDXGISwapChain1> created;
    result = host->factory->CreateSwapChainForComposition(
        host->queue.Get(), &description, nullptr, &created);
    if (SUCCEEDED(result))
      result = created.As(&swapchain);
    if (SUCCEEDED(result))
      result = swapchain->SetMaximumFrameLatency(1);
    if (SUCCEEDED(result)) {
      latency = swapchain->GetFrameLatencyWaitableObject();
      if (!latency)
        result = sgu_windows_error();
    }
    if (SUCCEEDED(result))
      result = visual->SetContent(swapchain.Get());
    if (SUCCEEDED(result))
      result = host->composition->Commit();
  }
  if (SUCCEEDED(result)) {
    bufferWidth = width;
    bufferHeight = height;
  }
  return result;
}

HRESULT SGUSurface::close() {
  required = false;
  stopNotifications();
  HRESULT result = host->drain();
  const HRESULT notifiedError = notificationError.load();
  if (SUCCEEDED(result))
    result = notifiedError;
  if (target) {
    const HRESULT detached = target->SetRoot(nullptr);
    if (FAILED(detached) && SUCCEEDED(result))
      result = detached;
  }
  if (visual) {
    const HRESULT detached = visual->SetContent(nullptr);
    if (FAILED(detached) && SUCCEEDED(result))
      result = detached;
  }
  const HRESULT committed = host->composition->Commit();
  if (FAILED(committed) && SUCCEEDED(result))
    result = committed;
  if (SUCCEEDED(committed)) {
    const HRESULT completed = host->composition->WaitForCommitCompletion();
    if (FAILED(completed) && SUCCEEDED(result))
      result = completed;
  }
  return result;
}

extern "C" {
SGUResult scoop_gui_d3d_surface_create(SGUDevice *host, HWND window,
                                       SGUCallback callback, void *context) {
  auto surface = std::unique_ptr<SGUSurface>(
      new (std::nothrow) SGUSurface(host, window, callback, context));
  if (!surface)
    return {nullptr, E_OUTOFMEMORY};
  const HRESULT result = surface->initialize();
  if (FAILED(result)) {
    surface->close();
    return {nullptr, result};
  }
  return {surface.release(), S_OK};
}

int32_t scoop_gui_d3d_surface_configure(SGUSurface *surface, int32_t width,
                                        int32_t height, double scale) {
  return surface->configure(width, height, scale);
}

int32_t scoop_gui_d3d_surface_demand(SGUSurface *surface, bool required) {
  surface->required = required;
  return surface->updateDemand();
}

int32_t scoop_gui_d3d_surface_close(SGUSurface *surface) {
  const HRESULT result = surface->close();
  delete surface;
  return result;
}
}
