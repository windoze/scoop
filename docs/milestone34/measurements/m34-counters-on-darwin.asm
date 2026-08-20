
tmp/m34/counters-on-darwin/single:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002f60c <_scoop_runtime_finish_tlab_alloc>:
10002f60c:     	stp	x22, x21, [sp, #-0x30]!
10002f610:     	stp	x20, x19, [sp, #0x10]
10002f614:     	stp	x29, x30, [sp, #0x20]
10002f618:     	add	x29, sp, #0x20
10002f61c:     	mov	x21, x2
10002f620:     	mov	x19, x1
10002f624:     	mov	x20, x0
10002f628:     	bl	0x100037ebc <_scoop_gc_stress_move_enabled>
10002f62c:     	tbnz	w0, #0x0, 0x10002f670 <_scoop_runtime_finish_tlab_alloc+0x64>
10002f630:     	ldr	x8, [x19, #0x90]
10002f634:     	cbnz	x8, 0x10002f670 <_scoop_runtime_finish_tlab_alloc+0x64>
10002f638:     	bl	0x100031628 <_scoop_thread_current_required>
10002f63c:     	mov	x22, x0
10002f640:     	mov	x0, x19
10002f644:     	mov	x1, x21
10002f648:     	bl	0x10003fc14 <_scoop_shape_normalize_allocation>
10002f64c:     	mov	x3, x0
10002f650:     	mov	x0, x22
10002f654:     	mov	x1, x20
10002f658:     	mov	x2, x19
10002f65c:     	bl	0x10002f67c <_finish_small_allocation>
10002f660:     	ldp	x29, x30, [sp, #0x20]
10002f664:     	ldp	x20, x19, [sp, #0x10]
10002f668:     	ldp	x22, x21, [sp], #0x30
10002f66c:     	ret
10002f670:     	adrp	x0, 0x10005e000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x320>
10002f674:     	add	x0, x0, #0x458
10002f678:     	bl	0x1000443cc <_scoop_heap_fatal>

000000010002f67c <_finish_small_allocation>:
10002f67c:     	sub	sp, sp, #0x40
10002f680:     	stp	x22, x21, [sp, #0x10]
10002f684:     	stp	x20, x19, [sp, #0x20]
10002f688:     	stp	x29, x30, [sp, #0x30]
10002f68c:     	add	x29, sp, #0x30
10002f690:     	cbz	x1, 0x10002f758 <_finish_small_allocation+0xdc>
10002f694:     	mov	x22, x2
10002f698:     	cbz	x2, 0x10002f758 <_finish_small_allocation+0xdc>
10002f69c:     	mov	x19, x3
10002f6a0:     	mov	w8, #0x7f80             ; =32640
10002f6a4:     	cmp	x3, x8
10002f6a8:     	b.hi	0x10002f758 <_finish_small_allocation+0xdc>
10002f6ac:     	mov	x21, x1
10002f6b0:     	ldr	x8, [x22, #0x18]
10002f6b4:     	udiv	x9, x1, x8
10002f6b8:     	msub	x8, x9, x8, x1
10002f6bc:     	cbnz	x8, 0x10002f758 <_finish_small_allocation+0xdc>
10002f6c0:     	mov	x20, x0
10002f6c4:     	add	x1, sp, #0xc
10002f6c8:     	mov	x0, x21
10002f6cc:     	bl	0x100037ee0 <_scoop_heap_pointer_block_index>
10002f6d0:     	tbz	w0, #0x0, 0x10002f758 <_finish_small_allocation+0xdc>
10002f6d4:     	mov	x0, x21
10002f6d8:     	mov	x1, x19
10002f6dc:     	bl	0x100044e28 <dyld_stub_binder+0x100044e28>
10002f6e0:     	stp	x22, xzr, [x21]
10002f6e4:     	ldr	w0, [sp, #0xc]
10002f6e8:     	mov	x1, x21
10002f6ec:     	mov	x2, x19
10002f6f0:     	mov	w3, #0x0                ; =0
10002f6f4:     	bl	0x1000323b0 <_scoop_heap_record_small_object>
10002f6f8:     	adrp	x8, 0x10014c000 <dyld_stub_binder+0x10014c000>
10002f6fc:     	add	x8, x8, #0x400
10002f700:     	ldr	x8, [x8, #0x58]
10002f704:     	ldr	w9, [sp, #0xc]
10002f708:     	add	x8, x8, x9, lsl #7
10002f70c:     	ldr	w8, [x8, #0x8]
10002f710:     	ldr	x9, [x20, #0x100]
10002f714:     	add	x9, x9, #0x1
10002f718:     	str	x9, [x20, #0x100]
10002f71c:     	ldr	x9, [x20, #0x110]
10002f720:     	add	x9, x9, x19
10002f724:     	str	x9, [x20, #0x110]
10002f728:     	cbnz	w8, 0x10002f744 <_finish_small_allocation+0xc8>
10002f72c:     	ldr	x8, [x20, #0x108]
10002f730:     	ldr	x9, [x20, #0x118]
10002f734:     	add	x8, x8, #0x1
10002f738:     	str	x8, [x20, #0x108]
10002f73c:     	add	x8, x9, x19
10002f740:     	str	x8, [x20, #0x118]
10002f744:     	ldp	x29, x30, [sp, #0x30]
10002f748:     	ldp	x20, x19, [sp, #0x20]
10002f74c:     	ldp	x22, x21, [sp, #0x10]
10002f750:     	add	sp, sp, #0x40
10002f754:     	ret
10002f758:     	adrp	x0, 0x10005e000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x320>
10002f75c:     	add	x0, x0, #0x4d9
10002f760:     	bl	0x1000443cc <_scoop_heap_fatal>
