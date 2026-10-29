#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include <wayland-client.h>

typedef struct Buffers Buffers;
typedef struct Buffer {
  Buffers *owner;
  struct wl_buffer *native;
  void *memory;
  size_t size;
  struct Buffer *next;
} Buffer;
struct Buffers {
  struct wl_display *wayland;
  struct wl_surface *surface;
  struct wl_registry *registry;
  struct wl_shm *shm;
  Buffer *buffers;
  Display *xlib;
  Window window;
  Visual *visual;
  int depth;
  GC gc;
};

static void buffer_free(Buffer *buffer) {
  wl_buffer_destroy(buffer->native);
  munmap(buffer->memory, buffer->size);
  free(buffer);
}
static void released(void *data, struct wl_buffer *native) {
  Buffer *buffer = data;
  Buffer **link = &buffer->owner->buffers;
  while (*link != buffer)
    link = &(*link)->next;
  *link = buffer->next;
  buffer_free(buffer);
}
static const struct wl_buffer_listener listener = {.release = released};

static void global(void *data, struct wl_registry *registry, uint32_t name,
                   const char *interface, uint32_t version) {
  Buffers *buffers = data;
  if (!strcmp(interface, "wl_shm") && !buffers->shm)
    buffers->shm = wl_registry_bind(registry, name, &wl_shm_interface, 1);
}
static void global_remove(void *data, struct wl_registry *registry,
                          uint32_t name) {
  // A bound wl_shm remains valid for the lifetime of the display connection.
  (void)data;
  (void)registry;
  (void)name;
}
static const struct wl_registry_listener registry_listener = {
    .global = global, .global_remove = global_remove};

void *scoop_example_buffers_wayland(void *display, void *surface) {
  Buffers *buffers = calloc(1, sizeof(*buffers));
  if (!buffers)
    return NULL;
  buffers->wayland = display;
  buffers->surface = surface;
  buffers->registry = wl_display_get_registry(display);
  wl_registry_add_listener(buffers->registry, &registry_listener, buffers);
  if (wl_display_roundtrip(display) < 0 || !buffers->shm) {
    if (buffers->shm)
      wl_shm_destroy(buffers->shm);
    wl_registry_destroy(buffers->registry);
    free(buffers);
    return NULL;
  }
  return buffers;
}
void *scoop_example_buffers_xlib(void *display, uint64_t window,
                                 uint64_t visual, int32_t screen) {
  Buffers *buffers = calloc(1, sizeof(*buffers));
  if (!buffers)
    return NULL;
  buffers->xlib = display;
  buffers->window = (Window)window;
  XVisualInfo query = {.visualid = (VisualID)visual, .screen = screen};
  int count;
  XVisualInfo *info =
      XGetVisualInfo(display, VisualIDMask | VisualScreenMask, &query, &count);
  if (!info || !count) {
    if (info)
      XFree(info);
    free(buffers);
    return NULL;
  }
  buffers->visual = info->visual;
  buffers->depth = info->depth;
  XFree(info);
  buffers->gc = XCreateGC(display, buffers->window, 0, NULL);
  return buffers;
}

static void paint(uint32_t *pixels, int width, int height, uint32_t color) {
  for (int y = 0; y < height; ++y)
    for (int x = 0; x < width; ++x)
      pixels[(size_t)y * width + x] =
          UINT32_C(0xff000000) |
          (((x / 32 + y / 32) % 2) ? color & 0xffffff : UINT32_C(0x243447));
}

int32_t scoop_example_buffers_present(void *pointer, int32_t width,
                                      int32_t height, uint32_t color) {
  Buffers *buffers = pointer;
  if (width <= 0 || height <= 0 || width > INT_MAX / 4 ||
      height > INT_MAX / (width * 4))
    return EOVERFLOW;
  size_t size = (size_t)width * height * 4;
  if (buffers->wayland) {
    if (wl_display_get_error(buffers->wayland))
      return EPIPE;
    int fd = memfd_create("scoop-window-frame", MFD_CLOEXEC);
    if (fd < 0)
      return errno;
    if (ftruncate(fd, (off_t)size) < 0) {
      int error = errno;
      close(fd);
      return error;
    }
    void *memory = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (memory == MAP_FAILED) {
      int error = errno;
      close(fd);
      return error;
    }
    Buffer *buffer = calloc(1, sizeof(*buffer));
    if (!buffer) {
      munmap(memory, size);
      close(fd);
      return ENOMEM;
    }
    paint(memory, width, height, color);
    struct wl_shm_pool *pool =
        wl_shm_create_pool(buffers->shm, fd, (int32_t)size);
    buffer->native = wl_shm_pool_create_buffer(
        pool, 0, width, height, width * 4, WL_SHM_FORMAT_ARGB8888);
    wl_shm_pool_destroy(pool);
    close(fd);
    buffer->owner = buffers;
    buffer->memory = memory;
    buffer->size = size;
    buffer->next = buffers->buffers;
    buffers->buffers = buffer;
    wl_buffer_add_listener(buffer->native, &listener, buffer);
    wl_surface_attach(buffers->surface, buffer->native, 0, 0);
    wl_surface_damage_buffer(buffers->surface, 0, 0, INT_MAX, INT_MAX);
    wl_surface_commit(buffers->surface);
    if (wl_display_flush(buffers->wayland) < 0 && errno != EAGAIN)
      return errno;
  } else {
    XImage *image =
        XCreateImage(buffers->xlib, buffers->visual, buffers->depth, ZPixmap, 0,
                     NULL, (unsigned)width, (unsigned)height, 32, 0);
    if (!image)
      return ENOMEM;
    image->data = calloc((size_t)image->bytes_per_line, (size_t)height);
    if (!image->data) {
      XDestroyImage(image);
      return ENOMEM;
    }
    for (int y = 0; y < height; ++y)
      for (int x = 0; x < width; ++x)
        XPutPixel(image, x, y,
                  UINT32_C(0xff000000) |
                      (((x / 32 + y / 32) % 2) ? color & 0xffffff
                                               : UINT32_C(0x243447)));
    XPutImage(buffers->xlib, buffers->window, buffers->gc, image, 0, 0, 0, 0,
              (unsigned)width, (unsigned)height);
    XDestroyImage(image);
    XFlush(buffers->xlib);
  }
  return 0;
}

void scoop_example_buffers_destroy(void *pointer) {
  Buffers *buffers = pointer;
  while (buffers->buffers) {
    Buffer *buffer = buffers->buffers;
    buffers->buffers = buffer->next;
    buffer_free(buffer);
  }
  if (buffers->shm)
    wl_shm_destroy(buffers->shm);
  if (buffers->registry)
    wl_registry_destroy(buffers->registry);
  if (buffers->gc)
    XFreeGC(buffers->xlib, buffers->gc);
  free(buffers);
}
