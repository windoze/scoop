#define COBJMACROS
#include <windows.h>

#include <d3d11.h>
#include <dxgi.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct {
  ID3D11Device *device;
  ID3D11DeviceContext *context;
  IDXGISwapChain *swapchain;
  ID3D11RenderTargetView *view;
  ID3D11Query *completion;
  int32_t width, height;
} Surface;

typedef struct {
  Surface *pointer;
  int64_t status;
} SurfaceResult;

static HRESULT drain(Surface *surface) {
  ID3D11DeviceContext_End(surface->context,
                          (ID3D11Asynchronous *)surface->completion);
  ID3D11DeviceContext_Flush(surface->context);
  HRESULT result;
  do {
    result = ID3D11DeviceContext_GetData(
        surface->context, (ID3D11Asynchronous *)surface->completion, NULL, 0,
        D3D11_ASYNC_GETDATA_DONOTFLUSH);
    if (result == S_FALSE)
      Sleep(1);
  } while (result == S_FALSE);
  return result;
}

static void release(Surface *surface) {
  if (surface->context)
    ID3D11DeviceContext_ClearState(surface->context);
  if (surface->view)
    ID3D11RenderTargetView_Release(surface->view);
  if (surface->completion)
    ID3D11Query_Release(surface->completion);
  if (surface->swapchain)
    IDXGISwapChain_Release(surface->swapchain);
  if (surface->context) {
    ID3D11DeviceContext_Flush(surface->context);
    ID3D11DeviceContext_Release(surface->context);
  }
  if (surface->device)
    ID3D11Device_Release(surface->device);
  free(surface);
}

SurfaceResult scoop_example_dxgi_create(HWND hwnd) {
  Surface *surface = calloc(1, sizeof(*surface));
  if (!surface)
    return (SurfaceResult){NULL, E_OUTOFMEMORY};
  DXGI_SWAP_CHAIN_DESC desc = {0};
  desc.BufferDesc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
  desc.SampleDesc.Count = 1;
  desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
  desc.BufferCount = 2;
  desc.OutputWindow = hwnd;
  desc.Windowed = TRUE;
  desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
  HRESULT result = D3D11CreateDeviceAndSwapChain(
      NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
      NULL, 0, D3D11_SDK_VERSION, &desc, &surface->swapchain, &surface->device,
      NULL, &surface->context);
  if (FAILED(result)) {
    // WARP is the system's software D3D11 device for hosts without a usable
    // GPU.
    release(surface);
    surface = calloc(1, sizeof(*surface));
    if (!surface)
      return (SurfaceResult){NULL, E_OUTOFMEMORY};
    result = D3D11CreateDeviceAndSwapChain(
        NULL, D3D_DRIVER_TYPE_WARP, NULL, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        NULL, 0, D3D11_SDK_VERSION, &desc, &surface->swapchain,
        &surface->device, NULL, &surface->context);
  }
  if (SUCCEEDED(result)) {
    IDXGIFactory *factory = NULL;
    result = IDXGISwapChain_GetParent(surface->swapchain, &IID_IDXGIFactory,
                                      (void **)&factory);
    if (SUCCEEDED(result)) {
      result = IDXGIFactory_MakeWindowAssociation(factory, hwnd,
                                                  DXGI_MWA_NO_ALT_ENTER);
      IDXGIFactory_Release(factory);
    }
  }
  if (SUCCEEDED(result)) {
    D3D11_QUERY_DESC query = {D3D11_QUERY_EVENT, 0};
    result =
        ID3D11Device_CreateQuery(surface->device, &query, &surface->completion);
  }
  if (FAILED(result)) {
    release(surface);
    return (SurfaceResult){NULL, result};
  }
  return (SurfaceResult){surface, 0};
}

int64_t scoop_example_dxgi_present(Surface *surface, int32_t width,
                                   int32_t height) {
  HRESULT result;
  if (width != surface->width || height != surface->height) {
    ID3D11DeviceContext_ClearState(surface->context);
    if (surface->view) {
      ID3D11RenderTargetView_Release(surface->view);
      surface->view = NULL;
    }
    result = IDXGISwapChain_ResizeBuffers(surface->swapchain, 0, (UINT)width,
                                          (UINT)height, DXGI_FORMAT_UNKNOWN, 0);
    if (FAILED(result))
      return result;
    surface->width = width;
    surface->height = height;
  }
  if (!surface->view) {
    ID3D11Texture2D *buffer = NULL;
    result = IDXGISwapChain_GetBuffer(surface->swapchain, 0,
                                      &IID_ID3D11Texture2D, (void **)&buffer);
    if (FAILED(result))
      return result;
    result = ID3D11Device_CreateRenderTargetView(
        surface->device, (ID3D11Resource *)buffer, NULL, &surface->view);
    ID3D11Texture2D_Release(buffer);
    if (FAILED(result))
      return result;
  }
  const float color[4] = {0.035f, 0.23f, 0.48f, 1.0f};
  ID3D11DeviceContext_ClearRenderTargetView(surface->context, surface->view,
                                            color);
  result = IDXGISwapChain_Present(surface->swapchain, 1, 0);
  HRESULT completed = drain(surface);
  return FAILED(completed) ? completed : result;
}

int64_t scoop_example_dxgi_destroy(Surface *surface) {
  HRESULT result = drain(surface);
  release(surface);
  return result;
}
