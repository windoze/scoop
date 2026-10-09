000000010002d0b8 <_scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab>:
10002d0b8:     	sub	sp, sp, #0x80
10002d0bc:     	stp	x20, x19, [sp, #0x60]
10002d0c0:     	stp	x29, x30, [sp, #0x70]
10002d0c4:     	add	x29, sp, #0x70
10002d0c8:     	adrp	x8, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d0cc:     	mov	x19, x0
10002d0d0:     	mov	x20, x1
10002d0d4:     	add	x8, x8, #0x5f8
10002d0d8:     	ldr	x9, [x8]
10002d0dc:     	mov	x0, x8
10002d0e0:     	blr	x9
10002d0e4:     	adrp	x8, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d0e8:     	adrp	x10, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d0ec:     	add	x8, x8, #0x848
10002d0f0:     	ldr	x9, [x0]
10002d0f4:     	add	x10, x10, #0x834
10002d0f8:     	ldar	x8, [x8]
10002d0fc:     	ldar	w11, [x10]
10002d100:     	add	x10, x9, #0x8
10002d104:     	ldar	w9, [x9]
10002d108:     	ldar	x10, [x10]
10002d10c:     	cbnz	w11, 0x10002d11c <_scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab+0x64>
10002d110:     	cmp	w9, #0x1
10002d114:     	ccmp	x10, x8, #0x0, eq
10002d118:     	b.eq	0x10002d120 <_scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab+0x68>
10002d11c:     	bl	0x100034250 <_scoop_rt_safepoint>
10002d120:     	movi.2d	v0, #0000000000000000
10002d124:     	add	x0, sp, #0x8
10002d128:     	mov	x1, xzr
10002d12c:     	mov	x2, xzr
10002d130:     	str	xzr, [sp, #0x18]
10002d134:     	stur	q0, [sp, #0x8]
10002d138:     	bl	0x10003f9a4 <_scoop_rt_push_caller_roots>
10002d13c:     	movi.2d	v0, #0000000000000000
10002d140:     	add	x0, sp, #0x20
10002d144:     	add	x1, sp, #0x20
10002d148:     	stp	q0, q0, [sp, #0x20]
10002d14c:     	stp	q0, q0, [sp, #0x40]
10002d150:     	bl	0x100034360 <_scoop_rt_enter_native_borrowed>
10002d154:     	mov	x0, x19
10002d158:     	mov	x1, x20
10002d15c:     	bl	0x100043268 <_pair_step>
10002d160:     	mov	x19, x0
10002d164:     	add	x0, sp, #0x20
10002d168:     	mov	x20, x1
10002d16c:     	bl	0x10003ec10 <_scoop_rt_leave_native_borrowed>
10002d170:     	add	x0, sp, #0x8
10002d174:     	bl	0x10003fa4c <_scoop_rt_pop_caller_roots>
10002d178:     	mov	x0, x19
10002d17c:     	mov	x1, x20
10002d180:     	ldp	x29, x30, [sp, #0x70]
10002d184:     	ldp	x20, x19, [sp, #0x60]
10002d188:     	add	sp, sp, #0x80
10002d18c:     	ret


000000010002d324 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
10002d324:     	sub	sp, sp, #0xb0
10002d328:     	stp	x26, x25, [sp, #0x60]
10002d32c:     	stp	x24, x23, [sp, #0x70]
10002d330:     	stp	x22, x21, [sp, #0x80]
10002d334:     	stp	x20, x19, [sp, #0x90]
10002d338:     	stp	x29, x30, [sp, #0xa0]
10002d33c:     	add	x29, sp, #0xa0
10002d340:     	adrp	x0, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d344:     	add	x0, x0, #0x5f8
10002d348:     	ldr	x8, [x0]
10002d34c:     	blr	x8
10002d350:     	adrp	x21, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d354:     	adrp	x23, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d358:     	add	x21, x21, #0x848
10002d35c:     	ldr	x22, [x0]
10002d360:     	add	x23, x23, #0x834
10002d364:     	add	x10, x22, #0x8
10002d368:     	ldar	x8, [x21]
10002d36c:     	ldar	w11, [x23]
10002d370:     	ldar	w9, [x22]
10002d374:     	ldar	x10, [x10]
10002d378:     	cbnz	w11, 0x10002d388 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x64>
10002d37c:     	cmp	w9, #0x1
10002d380:     	ccmp	x10, x8, #0x0, eq
10002d384:     	b.eq	0x10002d38c <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x68>
10002d388:     	bl	0x100034250 <_scoop_rt_safepoint>
10002d38c:     	mov	w25, #0x9680            ; =38528
10002d390:     	mov	x19, xzr
10002d394:     	mov	w24, wzr
10002d398:     	mov	w20, #0xa               ; =10
10002d39c:     	movk	w25, #0x98, lsl #16
10002d3a0:     	ldar	x8, [x21]
10002d3a4:     	ldar	w9, [x23]
10002d3a8:     	add	x11, x22, #0x8
10002d3ac:     	ldar	w10, [x22]
10002d3b0:     	ldar	x11, [x11]
10002d3b4:     	cmp	w9, #0x0
10002d3b8:     	ccmp	w10, #0x1, #0x0, eq
10002d3bc:     	ccmp	x11, x8, #0x0, eq
10002d3c0:     	b.eq	0x10002d3c8 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0xa4>
10002d3c4:     	bl	0x100034250 <_scoop_rt_safepoint>
10002d3c8:     	cmp	w24, w25
10002d3cc:     	b.ge	0x10002d430 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x10c>
10002d3d0:     	movi.2d	v0, #0000000000000000
10002d3d4:     	add	x0, sp, #0x8
10002d3d8:     	mov	x1, xzr
10002d3dc:     	mov	x2, xzr
10002d3e0:     	str	xzr, [sp, #0x18]
10002d3e4:     	stur	q0, [sp, #0x8]
10002d3e8:     	bl	0x10003f9a4 <_scoop_rt_push_caller_roots>
10002d3ec:     	movi.2d	v0, #0000000000000000
10002d3f0:     	add	x0, sp, #0x20
10002d3f4:     	add	x1, sp, #0x20
10002d3f8:     	stp	q0, q0, [sp, #0x20]
10002d3fc:     	stp	q0, q0, [sp, #0x40]
10002d400:     	bl	0x100034360 <_scoop_rt_enter_native_borrowed>
10002d404:     	mov	x0, x19
10002d408:     	mov	x1, x20
10002d40c:     	bl	0x100043268 <_pair_step>
10002d410:     	mov	x19, x0
10002d414:     	add	x0, sp, #0x20
10002d418:     	mov	x20, x1
10002d41c:     	bl	0x10003ec10 <_scoop_rt_leave_native_borrowed>
10002d420:     	add	x0, sp, #0x8
10002d424:     	bl	0x10003fa4c <_scoop_rt_pop_caller_roots>
10002d428:     	add	w24, w24, #0x1
10002d42c:     	b	0x10002d3a0 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x7c>
10002d430:     	add	x0, x19, x20
10002d434:     	bl	0x100044b58 <dyld_stub_binder+0x100044b58>
10002d438:     	ldp	x29, x30, [sp, #0xa0]
10002d43c:     	ldp	x20, x19, [sp, #0x90]
10002d440:     	ldp	x22, x21, [sp, #0x80]
10002d444:     	ldp	x24, x23, [sp, #0x70]
10002d448:     	ldp	x26, x25, [sp, #0x60]
10002d44c:     	add	sp, sp, #0xb0
10002d450:     	ret


; Callee / provider code

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/small-values-after-darwin/native.o:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000000000000 <ltmp0>:
       0:      	add	x0, x0, #0x1
       4:      	ret
