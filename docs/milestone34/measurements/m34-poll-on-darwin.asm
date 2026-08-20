
tmp/m34/poll-cached-darwin/primitive:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002d464 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
10002d464:     	stp	x26, x25, [sp, #-0x50]!
10002d468:     	stp	x24, x23, [sp, #0x10]
10002d46c:     	stp	x22, x21, [sp, #0x20]
10002d470:     	stp	x20, x19, [sp, #0x30]
10002d474:     	stp	x29, x30, [sp, #0x40]
10002d478:     	add	x29, sp, #0x40
10002d47c:     	adrp	x0, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d480:     	add	x0, x0, #0x5f8
10002d484:     	ldr	x8, [x0]
10002d488:     	blr	x8
10002d48c:     	adrp	x20, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d490:     	adrp	x22, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002d494:     	add	x20, x20, #0x830
10002d498:     	ldr	x21, [x0]
10002d49c:     	add	x22, x22, #0x81c
10002d4a0:     	add	x10, x21, #0x8
10002d4a4:     	ldar	x8, [x20]
10002d4a8:     	ldar	w11, [x22]
10002d4ac:     	ldar	w9, [x21]
10002d4b0:     	ldar	x10, [x10]
10002d4b4:     	cbnz	w11, 0x10002d4c4 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x60>
10002d4b8:     	cmp	w9, #0x1
10002d4bc:     	ccmp	x10, x8, #0x0, eq
10002d4c0:     	b.eq	0x10002d4c8 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x64>
10002d4c4:     	bl	0x100034048 <_scoop_rt_safepoint>
10002d4c8:     	mov	x24, #-0xf0f0f0f0f0f0f10 ; =-1085102592571150096
10002d4cc:     	mov	w25, #0x9680            ; =38528
10002d4d0:     	mov	x23, xzr
10002d4d4:     	mov	w19, #0x1               ; =1
10002d4d8:     	movk	x24, #0xf0f1
10002d4dc:     	movk	w25, #0x98, lsl #16
10002d4e0:     	add	x9, x21, #0x8
10002d4e4:     	ldar	x8, [x20]
10002d4e8:     	ldar	w11, [x22]
10002d4ec:     	ldar	w10, [x21]
10002d4f0:     	ldar	x9, [x9]
10002d4f4:     	cbnz	w11, 0x10002d508 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0xa4>
10002d4f8:     	cmp	w10, #0x1
10002d4fc:     	b.ne	0x10002d508 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0xa4>
10002d500:     	cmp	x9, x8
10002d504:     	b.eq	0x10002d50c <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0xa8>
10002d508:     	bl	0x100034048 <_scoop_rt_safepoint>
10002d50c:     	cmp	x23, x25
10002d510:     	b.ge	0x10002d530 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0xcc>
10002d514:     	umulh	x8, x23, x24
10002d518:     	lsr	x8, x8, #4
10002d51c:     	add	x8, x8, x8, lsl #4
10002d520:     	sub	x8, x23, x8
10002d524:     	add	x23, x23, #0x1
10002d528:     	add	x19, x19, x8
10002d52c:     	b	0x10002d4e0 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x7c>
10002d530:     	mov	x0, x19
10002d534:     	bl	0x100044bd0 <dyld_stub_binder+0x100044bd0>
10002d538:     	ldp	x29, x30, [sp, #0x40]
10002d53c:     	ldp	x20, x19, [sp, #0x30]
10002d540:     	ldp	x22, x21, [sp, #0x20]
10002d544:     	ldp	x24, x23, [sp, #0x10]
10002d548:     	ldp	x26, x25, [sp], #0x50
10002d54c:     	ret
