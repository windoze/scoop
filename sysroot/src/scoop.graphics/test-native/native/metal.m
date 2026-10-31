#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#include <stdint.h>

@interface SGTestMetal : NSObject
@property(nonatomic, strong) id<MTLDevice> device;
@property(nonatomic, strong) id<MTLCommandQueue> queue;
@property(nonatomic, strong) id<MTLTexture> texture;
@end
@implementation SGTestMetal
@end

void *scoop_graphics_test_metal_create(int32_t width, int32_t height) {
  @autoreleasepool {
    @try {
      SGTestMetal *owner = [SGTestMetal new];
      owner.device = MTLCreateSystemDefaultDevice();
      owner.queue = [owner.device newCommandQueue];
      MTLTextureDescriptor *descriptor = [MTLTextureDescriptor
          texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                       width:(NSUInteger)width
                                      height:(NSUInteger)height
                                   mipmapped:NO];
      descriptor.usage =
          MTLTextureUsageRenderTarget | MTLTextureUsageShaderRead;
      descriptor.storageMode = MTLStorageModePrivate;
      owner.texture = [owner.device newTextureWithDescriptor:descriptor];
      return owner.device && owner.queue && owner.texture
                 ? (__bridge_retained void *)owner
                 : NULL;
    } @catch (NSException *exception) {
      return NULL;
    }
  }
}
void *scoop_graphics_test_metal_object(void *pointer, int32_t kind) {
  SGTestMetal *owner = (__bridge SGTestMetal *)pointer;
  switch (kind) {
  case 0:
    return (__bridge void *)owner.device;
  case 1:
    return (__bridge void *)owner.queue;
  default:
    return (__bridge void *)owner.texture;
  }
}
void scoop_graphics_test_metal_destroy(void *pointer) {
  SGTestMetal *owner = (__bridge_transfer SGTestMetal *)pointer;
  (void)owner;
}
