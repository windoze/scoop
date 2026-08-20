000000010002d898 <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe>:
10002d898:     	sub	sp, sp, #0x90
10002d89c:     	stp	x26, x25, [sp, #0x40]
10002d8a0:     	stp	x24, x23, [sp, #0x50]
10002d8a4:     	stp	x22, x21, [sp, #0x60]
10002d8a8:     	stp	x20, x19, [sp, #0x70]
10002d8ac:     	stp	x29, x30, [sp, #0x80]
10002d8b0:     	add	x29, sp, #0x80
10002d8b4:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d8b8:     	add	x0, x0, #0x5f8
10002d8bc:     	ldr	x8, [x0]
10002d8c0:     	blr	x8
10002d8c4:     	adrp	x19, 0x100150000 <dyld_stub_binder+0x100150000>
10002d8c8:     	adrp	x21, 0x100150000 <dyld_stub_binder+0x100150000>
10002d8cc:     	add	x19, x19, #0x848
10002d8d0:     	ldr	x20, [x0]
10002d8d4:     	add	x21, x21, #0x834
10002d8d8:     	add	x10, x20, #0x8
10002d8dc:     	ldar	x8, [x19]
10002d8e0:     	ldar	w11, [x21]
10002d8e4:     	ldar	w9, [x20]
10002d8e8:     	ldar	x10, [x10]
10002d8ec:     	cbnz	w11, 0x10002d8fc <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x64>
10002d8f0:     	cmp	w9, #0x1
10002d8f4:     	ccmp	x10, x8, #0x0, eq
10002d8f8:     	b.eq	0x10002d900 <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x68>
10002d8fc:     	bl	0x100034920 <_scoop_rt_safepoint>
10002d900:     	add	x8, sp, #0x30
10002d904:     	mov	x0, xzr
10002d908:     	mov	w1, #0xa                ; =10
10002d90c:     	add	x25, sp, #0x30
10002d910:     	bl	0x10002d728 <_scoop$1$cb$acd3e5dda25b7bdf7138e34b146c3daeb32f8f25d217af5a91e32bcd2aeda727>
10002d914:     	ldr	x24, [sp, #0x38]
10002d918:     	mov	w23, #0x9680            ; =38528
10002d91c:     	mov	w22, wzr
10002d920:     	movk	w23, #0x98, lsl #16
10002d924:     	ldr	x25, [x25]
10002d928:     	ldar	x8, [x19]
10002d92c:     	add	x11, x20, #0x8
10002d930:     	ldar	w9, [x21]
10002d934:     	ldar	w10, [x20]
10002d938:     	ldar	x11, [x11]
10002d93c:     	cmp	w9, #0x0
10002d940:     	ccmp	w10, #0x1, #0x0, eq
10002d944:     	ccmp	x11, x8, #0x0, eq
10002d948:     	b.eq	0x10002d950 <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0xb8>
10002d94c:     	bl	0x100034920 <_scoop_rt_safepoint>
10002d950:     	cmp	w22, w23
10002d954:     	b.ge	0x10002d97c <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0xe4>
10002d958:     	stp	x25, x24, [sp, #0x20]
10002d95c:     	add	x8, sp, #0x10
10002d960:     	add	x25, sp, #0x10
10002d964:     	ldr	q0, [sp, #0x20]
10002d968:     	str	q0, [sp]
10002d96c:     	bl	0x10002d87c <_scoop$1$cb$64a215114cc4a4dba695a2ad62a852d38ee16f2cec89138f711ddd904704b462>
10002d970:     	ldr	x24, [sp, #0x18]
10002d974:     	add	w22, w22, #0x1
10002d978:     	b	0x10002d924 <_scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x8c>
10002d97c:     	add	x0, x25, x24
10002d980:     	bl	0x100045220 <dyld_stub_binder+0x100045220>
10002d984:     	ldp	x29, x30, [sp, #0x80]
10002d988:     	ldp	x20, x19, [sp, #0x70]
10002d98c:     	ldp	x22, x21, [sp, #0x60]
10002d990:     	ldp	x24, x23, [sp, #0x50]
10002d994:     	ldp	x26, x25, [sp, #0x40]
10002d998:     	add	sp, sp, #0x90
10002d99c:     	ret


; Callee / provider code
000000010002d380 <_scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18>:
10002d380:     	stp	x29, x30, [sp, #-0x10]!
10002d384:     	mov	x29, sp
10002d388:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d38c:     	add	x0, x0, #0x5f8
10002d390:     	ldr	x8, [x0]
10002d394:     	blr	x8
10002d398:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d39c:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d3a0:     	add	x8, x8, #0x848
10002d3a4:     	ldr	x9, [x0]
10002d3a8:     	add	x10, x10, #0x834
10002d3ac:     	ldar	x8, [x8]
10002d3b0:     	ldar	w11, [x10]
10002d3b4:     	add	x10, x9, #0x8
10002d3b8:     	ldar	w9, [x9]
10002d3bc:     	ldar	x10, [x10]
10002d3c0:     	cbnz	w11, 0x10002d3d0 <_scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18+0x50>
10002d3c4:     	cmp	w9, #0x1
10002d3c8:     	ccmp	x10, x8, #0x0, eq
10002d3cc:     	b.eq	0x10002d3d4 <_scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18+0x54>
10002d3d0:     	bl	0x100034920 <_scoop_rt_safepoint>
10002d3d4:     	adrp	x0, 0x10014f000 <_scoop$1$bs$ae1ad4ee9cec5ccbbbecf55bd44fde027609c423086773223f61c9f6d780f134+0x218>
10002d3d8:     	ldr	x0, [x0, #0xa80]
10002d3dc:     	bl	0x100041290 <_scoop_rt_trap>


000000010002d3e0 <_scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>:
10002d3e0:     	stp	x29, x30, [sp, #-0x10]!
10002d3e4:     	mov	x29, sp
10002d3e8:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d3ec:     	add	x0, x0, #0x5f8
10002d3f0:     	ldr	x8, [x0]
10002d3f4:     	blr	x8
10002d3f8:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d3fc:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d400:     	add	x8, x8, #0x848
10002d404:     	ldr	x9, [x0]
10002d408:     	add	x10, x10, #0x834
10002d40c:     	ldar	x8, [x8]
10002d410:     	ldar	w11, [x10]
10002d414:     	add	x10, x9, #0x8
10002d418:     	ldar	w9, [x9]
10002d41c:     	ldar	x10, [x10]
10002d420:     	cbnz	w11, 0x10002d430 <_scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081+0x50>
10002d424:     	cmp	w9, #0x1
10002d428:     	ccmp	x10, x8, #0x0, eq
10002d42c:     	b.eq	0x10002d434 <_scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081+0x54>
10002d430:     	bl	0x100034920 <_scoop_rt_safepoint>
10002d434:     	adrp	x0, 0x10014f000 <_scoop$1$bs$ae1ad4ee9cec5ccbbbecf55bd44fde027609c423086773223f61c9f6d780f134+0x218>
10002d438:     	ldr	x0, [x0, #0xd68]
10002d43c:     	bl	0x100041290 <_scoop_rt_trap>


000000010002d440 <_scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe>:
10002d440:     	sub	sp, sp, #0x1b0
10002d444:     	stp	x24, x23, [sp, #0x170]
10002d448:     	stp	x22, x21, [sp, #0x180]
10002d44c:     	stp	x20, x19, [sp, #0x190]
10002d450:     	stp	x29, x30, [sp, #0x1a0]
10002d454:     	add	x29, sp, #0x1a0
10002d458:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d45c:     	mov	x9, x0
10002d460:     	mov	x19, x1
10002d464:     	add	x8, x8, #0x5f8
10002d468:     	ldr	x10, [x8]
10002d46c:     	mov	x0, x8
10002d470:     	blr	x10
10002d474:     	stp	x9, x2, [sp, #0x70]
10002d478:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d47c:     	ldr	x9, [x0]
10002d480:     	stp	x3, xzr, [sp, #0x80]
10002d484:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d488:     	add	x8, x8, #0x848
10002d48c:     	str	xzr, [sp, #0x90]
10002d490:     	add	x10, x10, #0x834
10002d494:     	stp	xzr, xzr, [sp, #0x98]
10002d498:     	ldar	x8, [x8]
10002d49c:     	ldar	w11, [x10]
10002d4a0:     	add	x10, x9, #0x8
10002d4a4:     	ldar	w9, [x9]
10002d4a8:     	ldar	x10, [x10]
10002d4ac:     	cbnz	w11, 0x10002d4bc <_scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x7c>
10002d4b0:     	cmp	w9, #0x1
10002d4b4:     	ccmp	x10, x8, #0x0, eq
10002d4b8:     	b.eq	0x10002d4d4 <_scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x94>
10002d4bc:     	ldp	x8, x9, [sp, #0x70]
10002d4c0:     	stp	x9, x8, [sp, #0x20]
10002d4c4:     	bl	0x100034920 <_scoop_rt_safepoint>
10002d4c8:     	ldp	x9, x8, [sp, #0x20]
10002d4cc:     	str	x8, [sp, #0x70]
10002d4d0:     	str	x9, [sp, #0x78]
10002d4d4:     	bl	0x10004052c <_scoop_rt_context_snapshot>
10002d4d8:     	ldp	x8, x9, [sp, #0x70]
10002d4dc:     	adrp	x1, 0x10007f000 <_scoop$1$td$71fcf7979355356ec9a0e407d84898699c8ce153d96caab82feafef0fd1bba72+0x30>
10002d4e0:     	stp	x0, x9, [sp, #0x18]
10002d4e4:     	str	x8, [sp, #0x28]
10002d4e8:     	add	x1, x1, #0x890
10002d4ec:     	bl	0x100034a00 <_scoop_rt_context_fork>
10002d4f0:     	ldp	x10, x8, [sp, #0x20]
10002d4f4:     	ldr	x9, [sp, #0x18]
10002d4f8:     	str	x8, [sp, #0x70]
10002d4fc:     	str	x10, [sp, #0x78]
10002d500:     	str	x9, [sp, #0xa0]
10002d504:     	bl	0x1000409f4 <_scoop_rt_context_enter>
10002d508:     	str	x0, [sp, #0x68]
10002d50c:     	adrp	x9, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d510:     	add	x9, x9, #0x350
10002d514:     	ldr	x8, [sp, #0x70]
10002d518:     	ldr	x20, [sp, #0x70]
10002d51c:     	add	x23, sp, #0x68
10002d520:     	add	x8, sp, #0x78
10002d524:     	sub	x0, x29, #0x48
10002d528:     	sub	x1, x29, #0x78
10002d52c:     	str	x20, [sp, #0x98]
10002d530:     	mov	w2, #0x3                ; =3
10002d534:     	ldr	x21, [sp, #0x78]
10002d538:     	stp	x8, x9, [x29, #-0x78]
10002d53c:     	adrp	x8, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d540:     	add	x8, x8, #0x360
10002d544:     	add	x9, sp, #0x98
10002d548:     	ldr	x22, [sp, #0x80]
10002d54c:     	stp	x23, x8, [x29, #-0x68]
10002d550:     	adrp	x8, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d554:     	add	x8, x8, #0x370
10002d558:     	stp	x9, x8, [x29, #-0x58]
10002d55c:     	stp	xzr, xzr, [x29, #-0x48]
10002d560:     	stur	xzr, [x29, #-0x38]
10002d564:     	bl	0x100040170 <_scoop_rt_push_compiler_roots>
10002d568:     	ldr	x9, [x19]
10002d56c:     	add	x8, sp, #0x30
10002d570:     	mov	x0, x20
10002d574:     	mov	x1, x21
10002d578:     	mov	x2, x22
10002d57c:     	blr	x9
10002d580:     	ldr	x19, [sp, #0x78]
10002d584:     	ldr	x20, [sp, #0x80]
10002d588:     	sub	x0, x29, #0x48
10002d58c:     	ldr	x21, [sp, #0x68]
10002d590:     	bl	0x100040218 <_scoop_rt_pop_compiler_roots>
10002d594:     	str	x20, [sp, #0x80]
10002d598:     	str	x19, [sp, #0x78]
10002d59c:     	str	x21, [sp, #0x68]
10002d5a0:     	ldr	x8, [sp, #0x30]
10002d5a4:     	cbnz	x8, 0x10002d620 <_scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x1e0>
10002d5a8:     	ldp	x8, x9, [sp, #0x38]
10002d5ac:     	add	x0, sp, #0xc8
10002d5b0:     	ldr	x10, [sp, #0x78]
10002d5b4:     	ldr	x21, [sp, #0x80]
10002d5b8:     	add	x1, sp, #0xa8
10002d5bc:     	ldr	x20, [sp, #0x78]
10002d5c0:     	mov	w2, #0x2                ; =2
10002d5c4:     	stp	xzr, xzr, [sp, #0xc8]
10002d5c8:     	stp	x8, x9, [sp, #0x58]
10002d5cc:     	add	x8, sp, #0x68
10002d5d0:     	adrp	x9, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d5d4:     	add	x9, x9, #0x300
10002d5d8:     	str	x20, [sp, #0x88]
10002d5dc:     	add	x19, sp, #0xc8
10002d5e0:     	stp	x8, x9, [sp, #0xa8]
10002d5e4:     	add	x8, sp, #0x88
10002d5e8:     	adrp	x9, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d5ec:     	add	x9, x9, #0x310
10002d5f0:     	str	xzr, [sp, #0xd8]
10002d5f4:     	stp	x8, x9, [sp, #0xb8]
10002d5f8:     	bl	0x100040170 <_scoop_rt_push_compiler_roots>
10002d5fc:     	ldr	x8, [x21]
10002d600:     	ldur	q0, [sp, #0x58]
10002d604:     	mov	x0, x20
10002d608:     	str	q0, [sp]
10002d60c:     	blr	x8
10002d610:     	ldr	x20, [sp, #0x68]
10002d614:     	mov	x0, x19
10002d618:     	bl	0x100040218 <_scoop_rt_pop_compiler_roots>
10002d61c:     	str	x20, [sp, #0x68]
10002d620:     	ldr	x0, [sp, #0x68]
10002d624:     	bl	0x100040a20 <_scoop_rt_context_leave>
10002d628:     	ldp	x29, x30, [sp, #0x1a0]
10002d62c:     	ldp	x20, x19, [sp, #0x190]
10002d630:     	ldp	x22, x21, [sp, #0x180]
10002d634:     	ldp	x24, x23, [sp, #0x170]
10002d638:     	add	sp, sp, #0x1b0
10002d63c:     	ret
10002d640:     	ldr	x20, [sp, #0x78]
10002d644:     	ldr	x21, [sp, #0x80]
10002d648:     	mov	x19, x0
10002d64c:     	ldr	x22, [sp, #0x68]
10002d650:     	bl	0x10004026c <_scoop_rt_pop_top_compiler_roots>
10002d654:     	mov	x0, x19
10002d658:     	str	x21, [sp, #0x80]
10002d65c:     	str	x20, [sp, #0x78]
10002d660:     	str	x22, [sp, #0x68]
10002d664:     	bl	0x10003633c <_scoop_rt_begin_catch>
10002d668:     	ldr	x8, [sp, #0x78]
10002d66c:     	ldr	x9, [sp, #0x68]
10002d670:     	stp	x0, x9, [sp, #0x18]
10002d674:     	str	x8, [sp, #0x28]
10002d678:     	bl	0x100034980 <_scoop_rt_materialize_exception>
10002d67c:     	ldp	x10, x8, [sp, #0x20]
10002d680:     	ldr	x9, [sp, #0x18]
10002d684:     	str	x8, [sp, #0x78]
10002d688:     	str	x10, [sp, #0x68]
10002d68c:     	str	x9, [sp, #0x48]
10002d690:     	str	x0, [sp, #0x50]
10002d694:     	bl	0x1000364e4 <_scoop_rt_end_catch>
10002d698:     	ldr	x8, [sp, #0x78]
10002d69c:     	adrp	x9, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d6a0:     	add	x9, x9, #0x320
10002d6a4:     	add	x8, sp, #0x50
10002d6a8:     	ldr	x22, [sp, #0x80]
10002d6ac:     	ldr	x20, [sp, #0x78]
10002d6b0:     	stp	x8, x9, [x29, #-0xc0]
10002d6b4:     	adrp	x8, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d6b8:     	add	x8, x8, #0x330
10002d6bc:     	stp	x23, x8, [x29, #-0xb0]
10002d6c0:     	add	x8, sp, #0x90
10002d6c4:     	adrp	x9, 0x100125000 <_scoop$1$cr$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>
10002d6c8:     	add	x9, x9, #0x340
10002d6cc:     	ldr	x21, [sp, #0x50]
10002d6d0:     	sub	x0, x29, #0x90
10002d6d4:     	sub	x1, x29, #0xc0
10002d6d8:     	mov	w2, #0x3                ; =3
10002d6dc:     	str	x20, [sp, #0x90]
10002d6e0:     	stp	x8, x9, [x29, #-0xa0]
10002d6e4:     	sub	x19, x29, #0x90
10002d6e8:     	stp	xzr, xzr, [x29, #-0x90]
10002d6ec:     	stur	xzr, [x29, #-0x80]
10002d6f0:     	bl	0x100040170 <_scoop_rt_push_compiler_roots>
10002d6f4:     	ldr	x8, [x22, #0x8]
10002d6f8:     	mov	x0, x20
10002d6fc:     	mov	x1, x21
10002d700:     	blr	x8
10002d704:     	b	0x10002d610 <_scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x1d0>
10002d708:     	ldr	x20, [sp, #0x68]
10002d70c:     	mov	x19, x0
10002d710:     	bl	0x10004026c <_scoop_rt_pop_top_compiler_roots>
10002d714:     	mov	x0, x20
10002d718:     	str	x20, [sp, #0x68]
10002d71c:     	bl	0x100040a20 <_scoop_rt_context_leave>
10002d720:     	mov	x0, x19
10002d724:     	bl	0x100044fd4 <dyld_stub_binder+0x100044fd4>


000000010002d728 <_scoop$1$cb$acd3e5dda25b7bdf7138e34b146c3daeb32f8f25d217af5a91e32bcd2aeda727>:
10002d728:     	stp	x29, x30, [sp, #-0x10]!
10002d72c:     	mov	x29, sp
10002d730:     	stp	x0, x1, [x8]
10002d734:     	ldp	x29, x30, [sp], #0x10
10002d738:     	ret


000000010002d73c <_scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e>:
10002d73c:     	stp	x29, x30, [sp, #-0x10]!
10002d740:     	mov	x29, sp
10002d744:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d748:     	add	x0, x0, #0x5f8
10002d74c:     	ldr	x8, [x0]
10002d750:     	blr	x8
10002d754:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d758:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d75c:     	add	x8, x8, #0x848
10002d760:     	ldr	x9, [x0]
10002d764:     	add	x10, x10, #0x834
10002d768:     	ldar	x8, [x8]
10002d76c:     	ldar	w11, [x10]
10002d770:     	add	x10, x9, #0x8
10002d774:     	ldar	w9, [x9]
10002d778:     	ldar	x10, [x10]
10002d77c:     	cbnz	w11, 0x10002d78c <_scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e+0x50>
10002d780:     	cmp	w9, #0x1
10002d784:     	ccmp	x10, x8, #0x0, eq
10002d788:     	b.eq	0x10002d790 <_scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e+0x54>
10002d78c:     	bl	0x100034920 <_scoop_rt_safepoint>
10002d790:     	adrp	x0, 0x10014f000 <_scoop$1$bs$ae1ad4ee9cec5ccbbbecf55bd44fde027609c423086773223f61c9f6d780f134+0x218>
10002d794:     	ldr	x0, [x0, #0x608]
10002d798:     	bl	0x100041290 <_scoop_rt_trap>


000000010002d79c <_scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389>:
10002d79c:     	stp	x20, x19, [sp, #-0x20]!
10002d7a0:     	stp	x29, x30, [sp, #0x10]
10002d7a4:     	add	x29, sp, #0x10
10002d7a8:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d7ac:     	add	x19, x29, #0x20
10002d7b0:     	add	x20, x29, #0x10
10002d7b4:     	add	x0, x0, #0x5f8
10002d7b8:     	ldr	x8, [x0]
10002d7bc:     	blr	x8
10002d7c0:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d7c4:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d7c8:     	add	x8, x8, #0x848
10002d7cc:     	ldr	x9, [x0]
10002d7d0:     	add	x10, x10, #0x834
10002d7d4:     	ldar	x8, [x8]
10002d7d8:     	ldar	w11, [x10]
10002d7dc:     	add	x10, x9, #0x8
10002d7e0:     	ldar	w9, [x9]
10002d7e4:     	ldar	x10, [x10]
10002d7e8:     	cbnz	w11, 0x10002d7f8 <_scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x5c>
10002d7ec:     	cmp	w9, #0x1
10002d7f0:     	ccmp	x10, x8, #0x0, eq
10002d7f4:     	b.eq	0x10002d7fc <_scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x60>
10002d7f8:     	bl	0x100034920 <_scoop_rt_safepoint>
10002d7fc:     	ldp	x8, x11, [x19]
10002d800:     	ldp	x9, x10, [x20]
10002d804:     	ldp	x29, x30, [sp, #0x10]
10002d808:     	cmp	x9, x8
10002d80c:     	ccmp	x10, x11, #0x0, eq
10002d810:     	cset	w0, eq
10002d814:     	ldp	x20, x19, [sp], #0x20
10002d818:     	ret


000000010002d81c <_scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03>:
10002d81c:     	stp	x29, x30, [sp, #-0x10]!
10002d820:     	mov	x29, sp
10002d824:     	adrp	x0, 0x100150000 <dyld_stub_binder+0x100150000>
10002d828:     	add	x0, x0, #0x5f8
10002d82c:     	ldr	x8, [x0]
10002d830:     	blr	x8
10002d834:     	adrp	x8, 0x100150000 <dyld_stub_binder+0x100150000>
10002d838:     	adrp	x10, 0x100150000 <dyld_stub_binder+0x100150000>
10002d83c:     	add	x8, x8, #0x848
10002d840:     	ldr	x9, [x0]
10002d844:     	add	x10, x10, #0x834
10002d848:     	ldar	x8, [x8]
10002d84c:     	ldar	w11, [x10]
10002d850:     	add	x10, x9, #0x8
10002d854:     	ldar	w9, [x9]
10002d858:     	ldar	x10, [x10]
10002d85c:     	cbnz	w11, 0x10002d86c <_scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03+0x50>
10002d860:     	cmp	w9, #0x1
10002d864:     	ccmp	x10, x8, #0x0, eq
10002d868:     	b.eq	0x10002d870 <_scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03+0x54>
10002d86c:     	bl	0x100034920 <_scoop_rt_safepoint>
10002d870:     	adrp	x0, 0x10014f000 <_scoop$1$bs$ae1ad4ee9cec5ccbbbecf55bd44fde027609c423086773223f61c9f6d780f134+0x218>
10002d874:     	ldr	x0, [x0, #0xbc8]
10002d878:     	bl	0x100041290 <_scoop_rt_trap>


000000010002d87c <_scoop$1$cb$64a215114cc4a4dba695a2ad62a852d38ee16f2cec89138f711ddd904704b462>:
10002d87c:     	stp	x29, x30, [sp, #-0x10]!
10002d880:     	mov	x29, sp
10002d884:     	ldp	x9, x10, [x29, #0x10]
10002d888:     	add	x9, x9, #0x1
10002d88c:     	stp	x9, x10, [x8]
10002d890:     	ldp	x29, x30, [sp], #0x10
10002d894:     	ret
