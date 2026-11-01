#pragma once

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <d3d12.h>
#include <dcomp.h>
#include <dxgi1_4.h>
#include <wrl/client.h>

#include <atomic>
#include <cstdint>
#include <memory>
#include <new>

using Microsoft::WRL::ComPtr;
using SSUCallback = int32_t (*)(void *, int32_t);

struct SSUResult {
  void *pointer;
  int32_t status;
};

struct SSUDevice {
  ComPtr<IDXGIFactory4> factory;
  ComPtr<IDXGIAdapter1> adapter;
  ComPtr<ID3D12Device> device;
  ComPtr<ID3D12CommandQueue> queue;
  ComPtr<ID3D12Fence> fence;
  ComPtr<IDCompositionDevice> composition;
  HANDLE completion = nullptr;
  uint64_t serial = 0;

  ~SSUDevice();
  HRESULT initialize();
  HRESULT drain();
};

struct SSUSurface {
  SSUDevice *host;
  HWND window;
  HWND notification = nullptr;
  SSUCallback callback;
  void *context;
  ComPtr<IDXGISwapChain3> swapchain;
  ComPtr<IDCompositionTarget> target;
  ComPtr<IDCompositionVisual> visual;
  HANDLE latency = nullptr;
  PTP_WAIT wait = nullptr;
  std::atomic<UINT_PTR> notificationGeneration{0};
  std::atomic<HRESULT> notificationError{S_OK};
  uint64_t generation = 0;
  int32_t width = 0, height = 0;
  int32_t bufferWidth = 0, bufferHeight = 0;
  double scale = 1;
  bool required = false;
  bool waiting = false;
  bool queued = false;
  bool ready = false;
  bool active = false;

  SSUSurface(SSUDevice *host, HWND window, SSUCallback callback, void *context)
      : host(host), window(window), callback(callback), context(context) {}
  ~SSUSurface();
  HRESULT initialize();
  HRESULT configure(int32_t width, int32_t height, double scale);
  HRESULT updateDemand();
  HRESULT prepareBuffers();
  HRESULT close();
  void stopNotifications();
  void notified(UINT_PTR serial);
};

struct SSUFrame {
  SSUSurface *surface;
  ComPtr<ID3D12Resource> texture;
  uint64_t generation;
};

inline HRESULT ssu_windows_error() {
  const DWORD error = GetLastError();
  return error ? HRESULT_FROM_WIN32(error) : E_FAIL;
}
