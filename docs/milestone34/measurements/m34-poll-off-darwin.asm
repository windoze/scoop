
tmp/m34/poll-off-darwin/primitive:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001e760 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
10001e760:     	stp	x22, x21, [sp, #-0x30]!
10001e764:     	stp	x20, x19, [sp, #0x10]
10001e768:     	stp	x29, x30, [sp, #0x20]
10001e76c:     	add	x29, sp, #0x20
10001e770:     	bl	0x100025278 <_scoop_rt_safepoint>
10001e774:     	mov	x21, #-0xf0f0f0f0f0f0f10 ; =-1085102592571150096
10001e778:     	mov	w22, #0x9680            ; =38528
10001e77c:     	mov	x20, xzr
10001e780:     	mov	w19, #0x1               ; =1
10001e784:     	movk	x21, #0xf0f1
10001e788:     	movk	w22, #0x98, lsl #16
10001e78c:     	bl	0x100025278 <_scoop_rt_safepoint>
10001e790:     	cmp	x20, x22
10001e794:     	b.ge	0x10001e7b4 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x54>
10001e798:     	umulh	x8, x20, x21
10001e79c:     	lsr	x8, x8, #4
10001e7a0:     	add	x8, x8, x8, lsl #4
10001e7a4:     	sub	x8, x20, x8
10001e7a8:     	add	x20, x20, #0x1
10001e7ac:     	add	x19, x19, x8
10001e7b0:     	b	0x10001e78c <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x2c>
10001e7b4:     	mov	x0, x19
10001e7b8:     	bl	0x100035db4 <dyld_stub_binder+0x100035db4>
10001e7bc:     	ldp	x29, x30, [sp, #0x20]
10001e7c0:     	ldp	x20, x19, [sp, #0x10]
10001e7c4:     	ldp	x22, x21, [sp], #0x30
10001e7c8:     	ret
