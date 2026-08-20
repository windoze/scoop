000000010002d380 <_scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab>:
10002d380:     	sub	sp, sp, #0xb0
10002d384:     	stp	x20, x19, [sp, #0x90]
10002d388:     	stp	x29, x30, [sp, #0xa0]
10002d38c:     	add	x29, sp, #0xa0
10002d390:     	adrp	x0, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d394:     	mov	x19, x8
10002d398:     	add	x20, x29, #0x10
10002d39c:     	add	x0, x0, #0x5f8
10002d3a0:     	ldr	x8, [x0]
10002d3a4:     	blr	x8
10002d3a8:     	adrp	x8, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d3ac:     	adrp	x10, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d3b0:     	add	x8, x8, #0x848
10002d3b4:     	ldr	x9, [x0]
10002d3b8:     	add	x10, x10, #0x834
10002d3bc:     	ldar	x8, [x8]
10002d3c0:     	ldar	w11, [x10]
10002d3c4:     	add	x10, x9, #0x8
10002d3c8:     	ldar	w9, [x9]
10002d3cc:     	ldar	x10, [x10]
10002d3d0:     	cbnz	w11, 0x10002d3e0 <_scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab+0x60>
10002d3d4:     	cmp	w9, #0x1
10002d3d8:     	ccmp	x10, x8, #0x0, eq
10002d3dc:     	b.eq	0x10002d3e4 <_scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab+0x64>
10002d3e0:     	bl	0x100034520 <_scoop_rt_safepoint>
10002d3e4:     	movi.2d	v0, #0000000000000000
10002d3e8:     	ldp	x8, x9, [x20]
10002d3ec:     	add	x0, sp, #0x38
10002d3f0:     	mov	x1, xzr
10002d3f4:     	mov	x2, xzr
10002d3f8:     	str	xzr, [sp, #0x48]
10002d3fc:     	stp	x8, x9, [sp, #0x28]
10002d400:     	stur	q0, [sp, #0x38]
10002d404:     	bl	0x10003fc74 <_scoop_rt_push_caller_roots>
10002d408:     	movi.2d	v0, #0000000000000000
10002d40c:     	add	x0, sp, #0x50
10002d410:     	add	x1, sp, #0x50
10002d414:     	stp	q0, q0, [sp, #0x50]
10002d418:     	stp	q0, q0, [sp, #0x70]
10002d41c:     	bl	0x100034630 <_scoop_rt_enter_native_borrowed>
10002d420:     	ldur	q0, [sp, #0x28]
10002d424:     	add	x8, sp, #0x18
10002d428:     	str	q0, [sp]
10002d42c:     	bl	0x100043538 <_pair_step>
10002d430:     	add	x0, sp, #0x50
10002d434:     	bl	0x10003eee0 <_scoop_rt_leave_native_borrowed>
10002d438:     	add	x0, sp, #0x38
10002d43c:     	bl	0x10003fd1c <_scoop_rt_pop_caller_roots>
10002d440:     	ldp	x8, x9, [sp, #0x18]
10002d444:     	ldp	x29, x30, [sp, #0xa0]
10002d448:     	stp	x8, x9, [x19]
10002d44c:     	ldp	x20, x19, [sp, #0x90]
10002d450:     	add	sp, sp, #0xb0
10002d454:     	ret


000000010002d5f0 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
10002d5f0:     	sub	sp, sp, #0xe0
10002d5f4:     	stp	x26, x25, [sp, #0x90]
10002d5f8:     	stp	x24, x23, [sp, #0xa0]
10002d5fc:     	stp	x22, x21, [sp, #0xb0]
10002d600:     	stp	x20, x19, [sp, #0xc0]
10002d604:     	stp	x29, x30, [sp, #0xd0]
10002d608:     	add	x29, sp, #0xd0
10002d60c:     	adrp	x0, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d610:     	add	x0, x0, #0x5f8
10002d614:     	ldr	x8, [x0]
10002d618:     	blr	x8
10002d61c:     	adrp	x19, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d620:     	adrp	x21, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d624:     	add	x19, x19, #0x848
10002d628:     	ldr	x20, [x0]
10002d62c:     	add	x21, x21, #0x834
10002d630:     	add	x10, x20, #0x8
10002d634:     	ldar	x8, [x19]
10002d638:     	ldar	w11, [x21]
10002d63c:     	ldar	w9, [x20]
10002d640:     	ldar	x10, [x10]
10002d644:     	cbnz	w11, 0x10002d654 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x64>
10002d648:     	cmp	w9, #0x1
10002d64c:     	ccmp	x10, x8, #0x0, eq
10002d650:     	b.eq	0x10002d658 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x68>
10002d654:     	bl	0x100034520 <_scoop_rt_safepoint>
10002d658:     	mov	w23, #0x9680            ; =38528
10002d65c:     	mov	x24, xzr
10002d660:     	mov	w22, wzr
10002d664:     	mov	w25, #0xa               ; =10
10002d668:     	movk	w23, #0x98, lsl #16
10002d66c:     	ldar	x8, [x19]
10002d670:     	ldar	w9, [x21]
10002d674:     	add	x11, x20, #0x8
10002d678:     	ldar	w10, [x20]
10002d67c:     	ldar	x11, [x11]
10002d680:     	cmp	w9, #0x0
10002d684:     	ccmp	w10, #0x1, #0x0, eq
10002d688:     	ccmp	x11, x8, #0x0, eq
10002d68c:     	b.eq	0x10002d694 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0xa4>
10002d690:     	bl	0x100034520 <_scoop_rt_safepoint>
10002d694:     	cmp	w22, w23
10002d698:     	b.ge	0x10002d700 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x110>
10002d69c:     	movi.2d	v0, #0000000000000000
10002d6a0:     	add	x0, sp, #0x38
10002d6a4:     	mov	x1, xzr
10002d6a8:     	mov	x2, xzr
10002d6ac:     	stp	x24, x25, [sp, #0x28]
10002d6b0:     	str	xzr, [sp, #0x48]
10002d6b4:     	stur	q0, [sp, #0x38]
10002d6b8:     	bl	0x10003fc74 <_scoop_rt_push_caller_roots>
10002d6bc:     	movi.2d	v0, #0000000000000000
10002d6c0:     	add	x0, sp, #0x50
10002d6c4:     	add	x1, sp, #0x50
10002d6c8:     	stp	q0, q0, [sp, #0x50]
10002d6cc:     	stp	q0, q0, [sp, #0x70]
10002d6d0:     	bl	0x100034630 <_scoop_rt_enter_native_borrowed>
10002d6d4:     	ldur	q0, [sp, #0x28]
10002d6d8:     	add	x8, sp, #0x18
10002d6dc:     	str	q0, [sp]
10002d6e0:     	bl	0x100043538 <_pair_step>
10002d6e4:     	add	x0, sp, #0x50
10002d6e8:     	bl	0x10003eee0 <_scoop_rt_leave_native_borrowed>
10002d6ec:     	add	x0, sp, #0x38
10002d6f0:     	bl	0x10003fd1c <_scoop_rt_pop_caller_roots>
10002d6f4:     	ldp	x24, x25, [sp, #0x18]
10002d6f8:     	add	w22, w22, #0x1
10002d6fc:     	b	0x10002d66c <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x7c>
10002d700:     	add	x0, x24, x25
10002d704:     	bl	0x100044e30 <dyld_stub_binder+0x100044e30>
10002d708:     	ldp	x29, x30, [sp, #0xd0]
10002d70c:     	ldp	x20, x19, [sp, #0xc0]
10002d710:     	ldp	x22, x21, [sp, #0xb0]
10002d714:     	ldp	x24, x23, [sp, #0xa0]
10002d718:     	ldp	x26, x25, [sp, #0x90]
10002d71c:     	add	sp, sp, #0xe0
10002d720:     	ret


; Callee / provider code

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/small-values-before-darwin/native.o:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000000000000 <ltmp0>:
       0:      	ldp	x10, x9, [sp]
       4:      	add	x10, x10, #0x1
       8:      	stp	x10, x9, [x8]
       c:      	ret
