#include "renderer.h"

extern "C" {
SSUResult scoop_surface_d3d_surface_acquire(SSUSurface *surface) {
  const HRESULT error = surface->notificationError.load();
  if (FAILED(error))
    return {nullptr, error};
  if (!surface->ready || !surface->required || surface->active ||
      surface->width <= 0 || surface->height <= 0)
    return {nullptr, S_FALSE};
  surface->ready = false;
  if (!surface->swapchain || surface->width != surface->bufferWidth ||
      surface->height != surface->bufferHeight) {
    HRESULT result = surface->prepareBuffers();
    if (SUCCEEDED(result))
      result = surface->updateDemand();
    // Also wait for the first real frame opportunity after creating/resizing.
    return {nullptr, FAILED(result) ? result : S_FALSE};
  }
  auto frame = std::unique_ptr<SSUFrame>(new (std::nothrow) SSUFrame);
  if (!frame)
    return {nullptr, E_OUTOFMEMORY};
  HRESULT result = surface->swapchain->GetBuffer(
      surface->swapchain->GetCurrentBackBufferIndex(),
      IID_PPV_ARGS(&frame->texture));
  if (FAILED(result))
    return {nullptr, result};
  frame->surface = surface;
  frame->generation = surface->generation;
  surface->active = true;
  return {frame.release(), S_OK};
}

void *scoop_surface_d3d_frame_texture(SSUFrame *frame) {
  return frame->texture.Get();
}

int32_t scoop_surface_d3d_frame_present(SSUFrame *frame) {
  SSUSurface *surface = frame->surface;
  if (frame->generation != surface->generation)
    return S_FALSE;
  // Native size/focus changes can occur inside the painter before their queued
  // WindowEvents update the shared metrics snapshot.
  if (!IsWindowVisible(surface->window) || IsIconic(surface->window))
    return S_FALSE;
  RECT current;
  if (!GetClientRect(surface->window, &current))
    return ssu_windows_error();
  if (current.right != surface->width || current.bottom != surface->height ||
      GetDpiForWindow(surface->window) / 96.0 != surface->scale)
    return S_FALSE;
  const HRESULT result =
      surface->swapchain->Present(1, DXGI_PRESENT_DO_NOT_WAIT);
  if (result == DXGI_ERROR_WAS_STILL_DRAWING || result == DXGI_STATUS_OCCLUDED)
    return S_FALSE;
  if (FAILED(result)) {
    const HRESULT removed = surface->host->device->GetDeviceRemovedReason();
    return FAILED(removed) ? removed : result;
  }
  return result;
}

int32_t scoop_surface_d3d_frame_close(SSUFrame *frame) {
  SSUSurface *surface = frame->surface;
  delete frame;
  surface->active = false;
  return surface->updateDemand();
}
}
