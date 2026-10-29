#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#include <stdint.h>

@interface ScoopExampleMetalSurface : NSObject
@property(nonatomic, strong) NSView *view;
@property(nonatomic, strong) CALayer *previousLayer;
@property(nonatomic) BOOL previousWantsLayer;
@property(nonatomic, strong) CAMetalLayer *layer;
@property(nonatomic, strong) id<MTLCommandQueue> queue;
@end
@implementation ScoopExampleMetalSurface
@end

void *scoop_example_metal_create(void *pointer) {
  @autoreleasepool {
    NSView *view = (__bridge NSView *)pointer;
    if ([view.layer isKindOfClass:CAMetalLayer.class])
      return NULL;
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    if (!device)
      return NULL;
    ScoopExampleMetalSurface *surface = [ScoopExampleMetalSurface new];
    surface.view = view;
    surface.previousLayer = view.layer;
    surface.previousWantsLayer = view.wantsLayer;
    surface.queue = [device newCommandQueue];
    if (!surface.queue)
      return NULL;
    surface.layer = [CAMetalLayer layer];
    surface.layer.device = device;
    surface.layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
    surface.layer.framebufferOnly = YES;
    surface.layer.frame = view.bounds;
    surface.layer.autoresizingMask =
        kCALayerWidthSizable | kCALayerHeightSizable;
    view.wantsLayer = YES;
    view.layer = surface.layer;
    return (__bridge_retained void *)surface;
  }
}

int32_t scoop_example_metal_present(void *pointer, int32_t width,
                                    int32_t height, double scale) {
  @autoreleasepool {
    ScoopExampleMetalSurface *surface =
        (__bridge ScoopExampleMetalSurface *)pointer;
    surface.layer.contentsScale = scale;
    surface.layer.drawableSize = CGSizeMake(width, height);
    id<CAMetalDrawable> drawable = [surface.layer nextDrawable];
    if (!drawable)
      return -1;
    MTLRenderPassDescriptor *pass =
        [MTLRenderPassDescriptor renderPassDescriptor];
    pass.colorAttachments[0].texture = drawable.texture;
    pass.colorAttachments[0].loadAction = MTLLoadActionClear;
    pass.colorAttachments[0].storeAction = MTLStoreActionStore;
    pass.colorAttachments[0].clearColor =
        MTLClearColorMake(0.035, 0.23, 0.48, 1.0);
    id<MTLCommandBuffer> command = [surface.queue commandBuffer];
    id<MTLRenderCommandEncoder> encoder =
        [command renderCommandEncoderWithDescriptor:pass];
    if (!encoder)
      return -1;
    [encoder endEncoding];
    [command presentDrawable:drawable];
    [command commit];
    [command waitUntilCompleted];
    return command.status == MTLCommandBufferStatusCompleted ? 1 : -1;
  }
}

void scoop_example_metal_destroy(void *pointer) {
  ScoopExampleMetalSurface *surface =
      (__bridge_transfer ScoopExampleMetalSurface *)pointer;
  surface.view.layer = surface.previousLayer;
  surface.view.wantsLayer = surface.previousWantsLayer;
}
