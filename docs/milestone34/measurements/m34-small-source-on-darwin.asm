000000010002d5cc <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe>:
10002d5cc:     	stp	x26, x25, [sp, #-0x50]!
10002d5d0:     	stp	x24, x23, [sp, #0x10]
10002d5d4:     	stp	x22, x21, [sp, #0x20]
10002d5d8:     	stp	x20, x19, [sp, #0x30]
10002d5dc:     	stp	x29, x30, [sp, #0x40]
10002d5e0:     	add	x29, sp, #0x40
10002d5e4:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d5e8:     	add	x0, x0, #0x5f8
10002d5ec:     	ldr	x8, [x0]
10002d5f0:     	blr	x8
10002d5f4:     	adrp	x19, 0x100150000 <dyld_stub_binder+0x100150000>
10002d5f8:     	adrp	x21, 0x100150000 <dyld_stub_binder+0x100150000>
10002d5fc:     	add	x19, x19, #0x848
10002d600:     	ldr	x20, [x0]
10002d604:     	add	x21, x21, #0x834
10002d608:     	add	x10, x20, #0x8
10002d60c:     	ldar	x8, [x19]
10002d610:     	ldar	w11, [x21]
10002d614:     	ldar	w9, [x20]
10002d618:     	ldar	x10, [x10]
10002d61c:     	cbnz	w11, 0x10002d62c <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x60>
10002d620:     	cmp	w9, #0x1
10002d624:     	ccmp	x10, x8, #0x0, eq
10002d628:     	b.eq	0x10002d630 <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x64>
10002d62c:     	bl	0x100034634 <_scoop_rt_safepoint>
10002d630:     	mov	x0, xzr
10002d634:     	mov	w1, #0xa                ; =10
10002d638:     	bl	0x10002d45c <_scoop$1$cb$acd3e5dda25b7bdf7138e34b146c3daeb32f8f25d217af5a91e32bcd2aeda727>
10002d63c:     	mov	w23, #0x9680            ; =38528
10002d640:     	mov	w22, wzr
10002d644:     	movk	w23, #0x98, lsl #16
10002d648:     	ldar	x8, [x19]
10002d64c:     	ldar	w9, [x21]
10002d650:     	add	x11, x20, #0x8
10002d654:     	ldar	w10, [x20]
10002d658:     	ldar	x11, [x11]
10002d65c:     	cmp	w9, #0x0
10002d660:     	ccmp	w10, #0x1, #0x0, eq
10002d664:     	ccmp	x11, x8, #0x0, eq
10002d668:     	b.eq	0x10002d680 <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0xb4>
10002d66c:     	mov	x24, x0
10002d670:     	mov	x25, x1
10002d674:     	bl	0x100034634 <_scoop_rt_safepoint>
10002d678:     	mov	x1, x25
10002d67c:     	mov	x0, x24
10002d680:     	cmp	w22, w23
10002d684:     	b.ge	0x10002d694 <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0xc8>
10002d688:     	bl	0x10002d5b8 <_scoop$1$cb$64a215114cc4a4dba695a2ad62a852d38ee16f2cec89138f711ddd904704b462>
10002d68c:     	add	w22, w22, #0x1
10002d690:     	b	0x10002d648 <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x7c>
10002d694:     	add	x0, x0, x1
10002d698:     	bl	0x100044f34 <dyld_stub_binder+0x100044f34>
10002d69c:     	ldp	x29, x30, [sp, #0x40]
10002d6a0:     	ldp	x20, x19, [sp, #0x30]
10002d6a4:     	ldp	x22, x21, [sp, #0x20]
10002d6a8:     	ldp	x24, x23, [sp, #0x10]
10002d6ac:     	ldp	x26, x25, [sp], #0x50
10002d6b0:     	ret


; Callee / provider code
000000010002d0b8 <_scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18>:
10002d0b8:     	stp	x29, x30, [sp, #-0x10]!
10002d0bc:     	mov	x29, sp
10002d0c0:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d0c4:     	add	x0, x0, #0x5f8
10002d0c8:     	ldr	x8, [x0]
10002d0cc:     	blr	x8
10002d0d0:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d0d4:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d0d8:     	add	x8, x8, #0x848
10002d0dc:     	ldr	x9, [x0]
10002d0e0:     	add	x10, x10, #0x834
10002d0e4:     	ldar	x8, [x8]
10002d0e8:     	ldar	w11, [x10]
10002d0ec:     	add	x10, x9, #0x8
10002d0f0:     	ldar	w9, [x9]
10002d0f4:     	ldar	x10, [x10]
10002d0f8:     	cbnz	w11, 0x10002d108 <_scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18+0x50>
10002d0fc:     	cmp	w9, #0x1
10002d100:     	ccmp	x10, x8, #0x0, eq
10002d104:     	b.eq	0x10002d10c <_scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18+0x54>
10002d108:     	bl	0x100034634 <_scoop_rt_safepoint>
10002d10c:     	adrp	x0, 0x10014f000 <_scoop$1$bs$ae1ad4ee9cec5ccbbbecf55bd44fde027609c423086773223f61c9f6d780f134+0x218>
10002d110:     	ldr	x0, [x0, #0xa80]
10002d114:     	bl	0x100040fa4 <_scoop_rt_trap>


000000010002d118 <_scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>:
10002d118:     	stp	x29, x30, [sp, #-0x10]!
10002d11c:     	mov	x29, sp
10002d120:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d124:     	add	x0, x0, #0x5f8
10002d128:     	ldr	x8, [x0]
10002d12c:     	blr	x8
10002d130:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d134:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d138:     	add	x8, x8, #0x848
10002d13c:     	ldr	x9, [x0]
10002d140:     	add	x10, x10, #0x834
10002d144:     	ldar	x8, [x8]
10002d148:     	ldar	w11, [x10]
10002d14c:     	add	x10, x9, #0x8
10002d150:     	ldar	w9, [x9]
10002d154:     	ldar	x10, [x10]
10002d158:     	cbnz	w11, 0x10002d168 <_scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081+0x50>
10002d15c:     	cmp	w9, #0x1
10002d160:     	ccmp	x10, x8, #0x0, eq
10002d164:     	b.eq	0x10002d16c <_scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081+0x54>
10002d168:     	bl	0x100034634 <_scoop_rt_safepoint>
10002d16c:     	adrp	x0, 0x10014f000 <_scoop$1$bs$ae1ad4ee9cec5ccbbbecf55bd44fde027609c423086773223f61c9f6d780f134+0x218>
10002d170:     	ldr	x0, [x0, #0xd68]
10002d174:     	bl	0x100040fa4 <_scoop_rt_trap>


000000010002d178 <_scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe>:
10002d178:     	sub	sp, sp, #0x190
10002d17c:     	stp	x24, x23, [sp, #0x150]
10002d180:     	stp	x22, x21, [sp, #0x160]
10002d184:     	stp	x20, x19, [sp, #0x170]
10002d188:     	stp	x29, x30, [sp, #0x180]
10002d18c:     	add	x29, sp, #0x180
10002d190:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d194:     	mov	x9, x0
10002d198:     	mov	x19, x1
10002d19c:     	add	x8, x8, #0x5f8
10002d1a0:     	ldr	x10, [x8]
10002d1a4:     	mov	x0, x8
10002d1a8:     	blr	x10
10002d1ac:     	stp	x9, x2, [sp, #0x50]
10002d1b0:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d1b4:     	ldr	x9, [x0]
10002d1b8:     	stp	x3, xzr, [sp, #0x60]
10002d1bc:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d1c0:     	add	x8, x8, #0x848
10002d1c4:     	str	xzr, [sp, #0x70]
10002d1c8:     	add	x10, x10, #0x834
10002d1cc:     	stp	xzr, xzr, [sp, #0x78]
10002d1d0:     	ldar	x8, [x8]
10002d1d4:     	ldar	w11, [x10]
10002d1d8:     	add	x10, x9, #0x8
10002d1dc:     	ldar	w9, [x9]
10002d1e0:     	ldar	x10, [x10]
10002d1e4:     	cbnz	w11, 0x10002d1f4 <_scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x7c>
10002d1e8:     	cmp	w9, #0x1
10002d1ec:     	ccmp	x10, x8, #0x0, eq
10002d1f0:     	b.eq	0x10002d20c <_scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x94>
10002d1f4:     	ldp	x8, x9, [sp, #0x50]
10002d1f8:     	stp	x9, x8, [sp, #0x10]
10002d1fc:     	bl	0x100034634 <_scoop_rt_safepoint>
10002d200:     	ldp	x9, x8, [sp, #0x10]
10002d204:     	str	x8, [sp, #0x50]
10002d208:     	str	x9, [sp, #0x58]
10002d20c:     	bl	0x100040240 <_scoop_rt_context_snapshot>
10002d210:     	ldp	x8, x9, [sp, #0x50]
10002d214:     	adrp	x1, 0x10007f000 <_scoop$1$td$71fcf7979355356ec9a0e407d84898699c8ce153d96caab82feafef0fd1bba72+0x30>
10002d218:     	stp	x0, x9, [sp, #0x8]
10002d21c:     	str	x8, [sp, #0x18]
10002d220:     	add	x1, x1, #0x890
10002d224:     	bl	0x100034714 <_scoop_rt_context_fork>
10002d228:     	ldp	x10, x8, [sp, #0x10]
10002d22c:     	ldr	x9, [sp, #0x8]
10002d230:     	str	x8, [sp, #0x50]
10002d234:     	str	x10, [sp, #0x58]
10002d238:     	str	x9, [sp, #0x80]
10002d23c:     	bl	0x100040708 <_scoop_rt_context_enter>
10002d240:     	str	x0, [sp, #0x48]
10002d244:     	adrp	x9, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d248:     	add	x9, x9, #0x350
10002d24c:     	ldr	x8, [sp, #0x50]
10002d250:     	ldr	x20, [sp, #0x50]
10002d254:     	add	x23, sp, #0x48
10002d258:     	add	x8, sp, #0x58
10002d25c:     	sub	x0, x29, #0x48
10002d260:     	sub	x1, x29, #0x78
10002d264:     	str	x20, [sp, #0x78]
10002d268:     	mov	w2, #0x3                ; =3
10002d26c:     	ldr	x21, [sp, #0x58]
10002d270:     	stp	x8, x9, [x29, #-0x78]
10002d274:     	adrp	x8, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d278:     	add	x8, x8, #0x360
10002d27c:     	add	x9, sp, #0x78
10002d280:     	ldr	x22, [sp, #0x60]
10002d284:     	stp	x23, x8, [x29, #-0x68]
10002d288:     	adrp	x8, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d28c:     	add	x8, x8, #0x370
10002d290:     	stp	x9, x8, [x29, #-0x58]
10002d294:     	stp	xzr, xzr, [x29, #-0x48]
10002d298:     	stur	xzr, [x29, #-0x38]
10002d29c:     	bl	0x10003fe84 <_scoop_rt_push_compiler_roots>
10002d2a0:     	ldr	x9, [x19]
10002d2a4:     	add	x8, sp, #0x20
10002d2a8:     	mov	x0, x20
10002d2ac:     	mov	x1, x21
10002d2b0:     	mov	x2, x22
10002d2b4:     	blr	x9
10002d2b8:     	ldr	x19, [sp, #0x58]
10002d2bc:     	ldr	x20, [sp, #0x60]
10002d2c0:     	sub	x0, x29, #0x48
10002d2c4:     	ldr	x21, [sp, #0x48]
10002d2c8:     	bl	0x10003ff2c <_scoop_rt_pop_compiler_roots>
10002d2cc:     	str	x20, [sp, #0x60]
10002d2d0:     	str	x19, [sp, #0x58]
10002d2d4:     	str	x21, [sp, #0x48]
10002d2d8:     	ldr	x8, [sp, #0x20]
10002d2dc:     	cbnz	x8, 0x10002d354 <_scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x1dc>
10002d2e0:     	ldp	x20, x21, [sp, #0x28]
10002d2e4:     	adrp	x9, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d2e8:     	add	x9, x9, #0x300
10002d2ec:     	ldr	x8, [sp, #0x58]
10002d2f0:     	ldr	x23, [sp, #0x60]
10002d2f4:     	ldr	x22, [sp, #0x58]
10002d2f8:     	add	x8, sp, #0x48
10002d2fc:     	add	x0, sp, #0xa8
10002d300:     	stp	x8, x9, [sp, #0x88]
10002d304:     	add	x8, sp, #0x68
10002d308:     	adrp	x9, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d30c:     	add	x9, x9, #0x310
10002d310:     	add	x1, sp, #0x88
10002d314:     	mov	w2, #0x2                ; =2
10002d318:     	str	x22, [sp, #0x68]
10002d31c:     	add	x19, sp, #0xa8
10002d320:     	stp	x8, x9, [sp, #0x98]
10002d324:     	stp	xzr, xzr, [sp, #0xa8]
10002d328:     	str	xzr, [sp, #0xb8]
10002d32c:     	bl	0x10003fe84 <_scoop_rt_push_compiler_roots>
10002d330:     	ldr	x8, [x23]
10002d334:     	mov	x0, x22
10002d338:     	mov	x1, x20
10002d33c:     	mov	x2, x21
10002d340:     	blr	x8
10002d344:     	ldr	x20, [sp, #0x48]
10002d348:     	mov	x0, x19
10002d34c:     	bl	0x10003ff2c <_scoop_rt_pop_compiler_roots>
10002d350:     	str	x20, [sp, #0x48]
10002d354:     	ldr	x0, [sp, #0x48]
10002d358:     	bl	0x100040734 <_scoop_rt_context_leave>
10002d35c:     	ldp	x29, x30, [sp, #0x180]
10002d360:     	ldp	x20, x19, [sp, #0x170]
10002d364:     	ldp	x22, x21, [sp, #0x160]
10002d368:     	ldp	x24, x23, [sp, #0x150]
10002d36c:     	add	sp, sp, #0x190
10002d370:     	ret
10002d374:     	ldr	x20, [sp, #0x58]
10002d378:     	ldr	x21, [sp, #0x60]
10002d37c:     	mov	x19, x0
10002d380:     	ldr	x22, [sp, #0x48]
10002d384:     	bl	0x10003ff80 <_scoop_rt_pop_top_compiler_roots>
10002d388:     	mov	x0, x19
10002d38c:     	str	x21, [sp, #0x60]
10002d390:     	str	x20, [sp, #0x58]
10002d394:     	str	x22, [sp, #0x48]
10002d398:     	bl	0x100036050 <_scoop_rt_begin_catch>
10002d39c:     	ldr	x8, [sp, #0x58]
10002d3a0:     	ldr	x9, [sp, #0x48]
10002d3a4:     	stp	x0, x9, [sp, #0x8]
10002d3a8:     	str	x8, [sp, #0x18]
10002d3ac:     	bl	0x100034694 <_scoop_rt_materialize_exception>
10002d3b0:     	ldp	x10, x8, [sp, #0x10]
10002d3b4:     	ldr	x9, [sp, #0x8]
10002d3b8:     	str	x8, [sp, #0x58]
10002d3bc:     	str	x10, [sp, #0x48]
10002d3c0:     	str	x9, [sp, #0x38]
10002d3c4:     	str	x0, [sp, #0x40]
10002d3c8:     	bl	0x1000361f8 <_scoop_rt_end_catch>
10002d3cc:     	ldr	x8, [sp, #0x58]
10002d3d0:     	adrp	x9, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d3d4:     	add	x9, x9, #0x320
10002d3d8:     	add	x8, sp, #0x40
10002d3dc:     	ldr	x22, [sp, #0x60]
10002d3e0:     	ldr	x20, [sp, #0x58]
10002d3e4:     	stp	x8, x9, [sp, #0xc0]
10002d3e8:     	adrp	x8, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d3ec:     	add	x8, x8, #0x330
10002d3f0:     	stp	x23, x8, [sp, #0xd0]
10002d3f4:     	add	x8, sp, #0x70
10002d3f8:     	adrp	x9, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d3fc:     	add	x9, x9, #0x340
10002d400:     	ldr	x21, [sp, #0x40]
10002d404:     	sub	x0, x29, #0x90
10002d408:     	add	x1, sp, #0xc0
10002d40c:     	mov	w2, #0x3                ; =3
10002d410:     	str	x20, [sp, #0x70]
10002d414:     	stp	x8, x9, [sp, #0xe0]
10002d418:     	sub	x19, x29, #0x90
10002d41c:     	stp	xzr, xzr, [x29, #-0x90]
10002d420:     	stur	xzr, [x29, #-0x80]
10002d424:     	bl	0x10003fe84 <_scoop_rt_push_compiler_roots>
10002d428:     	ldr	x8, [x22, #0x8]
10002d42c:     	mov	x0, x20
10002d430:     	mov	x1, x21
10002d434:     	blr	x8
10002d438:     	b	0x10002d344 <_scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x1cc>
10002d43c:     	ldr	x20, [sp, #0x48]
10002d440:     	mov	x19, x0
10002d444:     	bl	0x10003ff80 <_scoop_rt_pop_top_compiler_roots>
10002d448:     	mov	x0, x20
10002d44c:     	str	x20, [sp, #0x48]
10002d450:     	bl	0x100040734 <_scoop_rt_context_leave>
10002d454:     	mov	x0, x19
10002d458:     	bl	0x100044ce8 <dyld_stub_binder+0x100044ce8>


000000010002d45c <_scoop$1$cb$acd3e5dda25b7bdf7138e34b146c3daeb32f8f25d217af5a91e32bcd2aeda727>:
10002d45c:     	stp	x29, x30, [sp, #-0x10]!
10002d460:     	mov	x29, sp
10002d464:     	ldp	x29, x30, [sp], #0x10
10002d468:     	ret


000000010002d46c <_scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e>:
10002d46c:     	stp	x29, x30, [sp, #-0x10]!
10002d470:     	mov	x29, sp
10002d474:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d478:     	add	x0, x0, #0x5f8
10002d47c:     	ldr	x8, [x0]
10002d480:     	blr	x8
10002d484:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d488:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d48c:     	add	x8, x8, #0x848
10002d490:     	ldr	x9, [x0]
10002d494:     	add	x10, x10, #0x834
10002d498:     	ldar	x8, [x8]
10002d49c:     	ldar	w11, [x10]
10002d4a0:     	add	x10, x9, #0x8
10002d4a4:     	ldar	w9, [x9]
10002d4a8:     	ldar	x10, [x10]
10002d4ac:     	cbnz	w11, 0x10002d4bc <_scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e+0x50>
10002d4b0:     	cmp	w9, #0x1
10002d4b4:     	ccmp	x10, x8, #0x0, eq
10002d4b8:     	b.eq	0x10002d4c0 <_scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e+0x54>
10002d4bc:     	bl	0x100034634 <_scoop_rt_safepoint>
10002d4c0:     	adrp	x0, 0x10014f000 <_scoop$1$bs$ae1ad4ee9cec5ccbbbecf55bd44fde027609c423086773223f61c9f6d780f134+0x218>
10002d4c4:     	ldr	x0, [x0, #0x608]
10002d4c8:     	bl	0x100040fa4 <_scoop_rt_trap>


000000010002d4cc <_scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389>:
10002d4cc:     	stp	x22, x21, [sp, #-0x30]!
10002d4d0:     	stp	x20, x19, [sp, #0x10]
10002d4d4:     	stp	x29, x30, [sp, #0x20]
10002d4d8:     	add	x29, sp, #0x20
10002d4dc:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d4e0:     	mov	x21, x0
10002d4e4:     	mov	x19, x3
10002d4e8:     	add	x8, x8, #0x5f8
10002d4ec:     	mov	x20, x2
10002d4f0:     	mov	x22, x1
10002d4f4:     	ldr	x9, [x8]
10002d4f8:     	mov	x0, x8
10002d4fc:     	blr	x9
10002d500:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d504:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d508:     	add	x8, x8, #0x848
10002d50c:     	ldr	x9, [x0]
10002d510:     	add	x10, x10, #0x834
10002d514:     	ldar	x8, [x8]
10002d518:     	ldar	w11, [x10]
10002d51c:     	add	x10, x9, #0x8
10002d520:     	ldar	w9, [x9]
10002d524:     	ldar	x10, [x10]
10002d528:     	cbnz	w11, 0x10002d538 <_scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x6c>
10002d52c:     	cmp	w9, #0x1
10002d530:     	ccmp	x10, x8, #0x0, eq
10002d534:     	b.eq	0x10002d53c <_scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x70>
10002d538:     	bl	0x100034634 <_scoop_rt_safepoint>
10002d53c:     	cmp	x21, x20
10002d540:     	ldp	x29, x30, [sp, #0x20]
10002d544:     	ccmp	x22, x19, #0x0, eq
10002d548:     	ldp	x20, x19, [sp, #0x10]
10002d54c:     	cset	w0, eq
10002d550:     	ldp	x22, x21, [sp], #0x30
10002d554:     	ret


000000010002d558 <_scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03>:
10002d558:     	stp	x29, x30, [sp, #-0x10]!
10002d55c:     	mov	x29, sp
10002d560:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d564:     	add	x0, x0, #0x5f8
10002d568:     	ldr	x8, [x0]
10002d56c:     	blr	x8
10002d570:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d574:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d578:     	add	x8, x8, #0x848
10002d57c:     	ldr	x9, [x0]
10002d580:     	add	x10, x10, #0x834
10002d584:     	ldar	x8, [x8]
10002d588:     	ldar	w11, [x10]
10002d58c:     	add	x10, x9, #0x8
10002d590:     	ldar	w9, [x9]
10002d594:     	ldar	x10, [x10]
10002d598:     	cbnz	w11, 0x10002d5a8 <_scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03+0x50>
10002d59c:     	cmp	w9, #0x1
10002d5a0:     	ccmp	x10, x8, #0x0, eq
10002d5a4:     	b.eq	0x10002d5ac <_scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03+0x54>
10002d5a8:     	bl	0x100034634 <_scoop_rt_safepoint>
10002d5ac:     	adrp	x0, 0x10014f000 <_scoop$1$bs$ae1ad4ee9cec5ccbbbecf55bd44fde027609c423086773223f61c9f6d780f134+0x218>
10002d5b0:     	ldr	x0, [x0, #0xbc8]
10002d5b4:     	bl	0x100040fa4 <_scoop_rt_trap>


000000010002d5b8 <_scoop$1$cb$64a215114cc4a4dba695a2ad62a852d38ee16f2cec89138f711ddd904704b462>:
10002d5b8:     	stp	x29, x30, [sp, #-0x10]!
10002d5bc:     	mov	x29, sp
10002d5c0:     	add	x0, x0, #0x1
10002d5c4:     	ldp	x29, x30, [sp], #0x10
10002d5c8:     	ret
