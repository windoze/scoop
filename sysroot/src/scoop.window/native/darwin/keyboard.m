#import "window.h"

void sw_modifiers(SWView *view, NSEventModifierFlags flags) {
  if (view.modifiers == flags)
    return;
  view.modifiers = flags;
  SWEvent *event = sw_event(view.owner, SWModifiers);
  SWEventData data = event.data;
  data.a = (int64_t)flags;
  event.data = data;
  [view.owner.application enqueue:event];
}

static void key(SWView *view, NSEvent *native, BOOL down) {
  NSNumber *code = @(native.keyCode);
  if (!down && !view.pressedKeys[code])
    return;
  SWEvent *event = sw_event(view.owner, SWKeyboard);
  SWEventData data = event.data;
  data.a = native.keyCode;
  data.b = (int64_t)native.modifierFlags;
  BOOL keyEvent =
      native.type == NSEventTypeKeyDown || native.type == NSEventTypeKeyUp;
  data.flags = down ? 1 | (keyEvent && native.isARepeat ? 2 : 0) : 0;
  if (keyEvent) {
    NSString *text = native.characters;
    if (!text.length || (native.modifierFlags & (NSEventModifierFlagControl |
                                                 NSEventModifierFlagCommand)))
      text =
          [native charactersByApplyingModifiers:native.modifierFlags &
                                                ~(NSEventModifierFlagControl |
                                                  NSEventModifierFlagCommand)];
    if (!native.characters.length && text.length)
      data.flags |= 8;
    event.text = [(text ?: native.charactersIgnoringModifiers)
        dataUsingEncoding:NSUTF8StringEncoding];
  }
  event.data = data;
  if (down)
    view.pressedKeys[code] = event;
  else
    [view.pressedKeys removeObjectForKey:code];
  [view.owner.application enqueue:event];
}

@implementation SWView (Input)
- (void)keyDown:(NSEvent *)event {
  sw_modifiers(self, event.modifierFlags);
  BOOL shortcut = (event.modifierFlags & (NSEventModifierFlagControl |
                                          NSEventModifierFlagCommand)) != 0;
  [self beginTextBatch];
  @try {
    if (self.imeAllowed && !shortcut)
      [self interpretKeyEvents:@[ event ]];
    else if (!shortcut)
      sw_commit(self, event.characters);
    if (shortcut || !self.compositionConsumed || self.forwardKey)
      key(self, event, YES);
  } @finally {
    [self endTextBatch];
  }
}
- (void)keyUp:(NSEvent *)event {
  sw_modifiers(self, event.modifierFlags);
  key(self, event, NO);
}
- (BOOL)performKeyEquivalent:(NSEvent *)event {
  if (event.type != NSEventTypeKeyDown ||
      !(event.modifierFlags &
        (NSEventModifierFlagCommand | NSEventModifierFlagControl)))
    return NO;
  [self keyDown:event];
  return YES;
}
- (void)flagsChanged:(NSEvent *)event {
  sw_modifiers(self, event.modifierFlags);
  NSEventModifierFlags aggregate = 0, side = 0, pair = 0;
  switch (event.keyCode) {
  case 54:
    aggregate = NSEventModifierFlagCommand;
    side = 0x10;
    pair = 0x18;
    break;
  case 55:
    aggregate = NSEventModifierFlagCommand;
    side = 0x08;
    pair = 0x18;
    break;
  case 56:
    aggregate = NSEventModifierFlagShift;
    side = 0x02;
    pair = 0x06;
    break;
  case 60:
    aggregate = NSEventModifierFlagShift;
    side = 0x04;
    pair = 0x06;
    break;
  case 58:
    aggregate = NSEventModifierFlagOption;
    side = 0x20;
    pair = 0x60;
    break;
  case 61:
    aggregate = NSEventModifierFlagOption;
    side = 0x40;
    pair = 0x60;
    break;
  case 59:
    aggregate = NSEventModifierFlagControl;
    side = 0x01;
    pair = 0x2001;
    break;
  case 62:
    aggregate = NSEventModifierFlagControl;
    side = 0x2000;
    pair = 0x2001;
    break;
  case 63:
    aggregate = NSEventModifierFlagFunction;
    break;
  default:
    return;
  }
  BOOL held = self.pressedKeys[@(event.keyCode)] != nil;
  BOOL down = (event.modifierFlags & aggregate) != 0;
  if (down && pair)
    down = (event.modifierFlags & pair) ? (event.modifierFlags & side) != 0
                                        : !held;
  if (down != held)
    key(self, event, down);
}
- (void)resetInput {
  [self cancelComposition];
  NSEventModifierFlags locks = self.modifiers & NSEventModifierFlagCapsLock;
  for (NSNumber *code in [[self.pressedKeys allKeys]
           sortedArrayUsingSelector:@selector(compare:)]) {
    SWEvent *original = self.pressedKeys[code];
    SWEvent *event = sw_event(self.owner, SWKeyboard);
    SWEventData data = original.data;
    data.flags = 4;
    data.b = (int64_t)locks;
    event.data = data;
    event.text = original.text;
    [self.owner.application enqueue:event];
  }
  [self.pressedKeys removeAllObjects];
  sw_modifiers(self, locks);
  self.pointerInside = NO;
  [self updateCursor];
}
@end
