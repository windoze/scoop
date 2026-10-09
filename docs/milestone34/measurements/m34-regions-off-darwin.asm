
/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/regions-off-darwin/region-stores:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002d76c <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
10002d76c:     	sub	sp, sp, #0xb0
10002d770:     	stp	x28, x27, [sp, #0x50]
10002d774:     	stp	x26, x25, [sp, #0x60]
10002d778:     	stp	x24, x23, [sp, #0x70]
10002d77c:     	stp	x22, x21, [sp, #0x80]
10002d780:     	stp	x20, x19, [sp, #0x90]
10002d784:     	stp	x29, x30, [sp, #0xa0]
10002d788:     	add	x29, sp, #0xa0
10002d78c:     	adrp	x0, 0x100154000 <dyld_stub_binder+0x100154000>
10002d790:     	add	x0, x0, #0x5f8
10002d794:     	ldr	x8, [x0]
10002d798:     	blr	x8
10002d79c:     	adrp	x21, 0x100154000 <dyld_stub_binder+0x100154000>
10002d7a0:     	adrp	x23, 0x100154000 <dyld_stub_binder+0x100154000>
10002d7a4:     	add	x21, x21, #0x848
10002d7a8:     	ldr	x22, [x0]
10002d7ac:     	add	x23, x23, #0x834
10002d7b0:     	add	x10, x22, #0x8
10002d7b4:     	ldar	x8, [x21]
10002d7b8:     	ldar	w11, [x23]
10002d7bc:     	ldar	w9, [x22]
10002d7c0:     	ldar	x10, [x10]
10002d7c4:     	cbnz	w11,  <L0>
10002d7c8:     	cmp	w9, #0x1
10002d7cc:     	ccmp	x10, x8, #0x0, eq
10002d7d0:     	b.eq	 <L1>
<L0>:
10002d7d4:     	bl	 <_scoop_rt_safepoint>
<L1>:
10002d7d8:     	adrp	x19, 0x100154000 <dyld_stub_binder+0x100154000>
10002d7dc:     	adrp	x10, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002d7e0:     	add	x19, x19, #0x5e0
10002d7e4:     	add	x10, x10, #0xa60
10002d7e8:     	str	xzr, [sp, #0x48]
10002d7ec:     	ldr	x24, [x19]
10002d7f0:     	ldr	x26, [x10, #0x18]
10002d7f4:     	mov	x0, x19
10002d7f8:     	blr	x24
10002d7fc:     	ldr	x8, [x0]
10002d800:     	neg	x9, x26
10002d804:     	ldr	x27, [x10, #0x90]
10002d808:     	ldr	x11, [x8]
10002d80c:     	add	x11, x26, x11
10002d810:     	sub	x11, x11, #0x1
10002d814:     	ands	x20, x11, x9
10002d818:     	b.eq	 <L2>
10002d81c:     	add	x10, x26, #0x1f
10002d820:     	and	x2, x10, x9
10002d824:     	mov	w9, #0x7f80             ; =32640
10002d828:     	cmp	x2, x9
10002d82c:     	b.hi	 <L2>
10002d830:     	cbnz	x27,  <L2>
10002d834:     	ldr	x10, [x8, #0x8]
10002d838:     	add	x9, x20, x2
10002d83c:     	cmp	x9, x10
10002d840:     	b.hi	 <L2>
10002d844:     	mov	x10, #0x7fffffffffffffff ; =9223372036854775807
10002d848:     	add	x10, x26, x10
10002d84c:     	cmn	x10, #0x21
10002d850:     	b.hi	 <L2>
10002d854:     	str	x9, [x8]
10002d858:     	adrp	x1, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002d85c:     	mov	x0, x20
10002d860:     	add	x1, x1, #0xa60
10002d864:     	bl	 <_scoop_runtime_finish_tlab_alloc>
10002d868:     	b	 <L3>
<L2>:
10002d86c:     	adrp	x0, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002d870:     	mov	w1, #0x20               ; =32
10002d874:     	add	x0, x0, #0xa60
10002d878:     	bl	 <_scoop_runtime_alloc_slow>
10002d87c:     	mov	x20, x0
10002d880:     	str	xzr, [sp, #0x48]
<L3>:
10002d884:     	ldr	x8, [sp, #0x48]
10002d888:     	adrp	x25, 0x100154000 <dyld_stub_binder+0x100154000>
10002d88c:     	mov	x9, x20
10002d890:     	add	x25, x25, #0x860
10002d894:     	str	x8, [x9, #0x18]!
10002d898:     	stur	xzr, [x9, #-0x8]
10002d89c:     	ldr	x8, [x25]
10002d8a0:     	add	x8, x8, x9, lsr #9
<L4>:
10002d8a4:     	ldxrb	w9, [x8]
10002d8a8:     	orr	w9, w9, #0x1
10002d8ac:     	stxrb	w10, w9, [x8]
10002d8b0:     	cbnz	w10,  <L4>
10002d8b4:     	mov	x0, x19
10002d8b8:     	str	x20, [sp, #0x40]
10002d8bc:     	str	xzr, [sp, #0x18]
10002d8c0:     	blr	x24
10002d8c4:     	ldr	x8, [x0]
10002d8c8:     	neg	x9, x26
10002d8cc:     	add	x12, x26, #0x1f
10002d8d0:     	and	x2, x12, x9
10002d8d4:     	ldp	x10, x11, [x8]
10002d8d8:     	add	x10, x26, x10
10002d8dc:     	sub	x10, x10, #0x1
10002d8e0:     	and	x20, x10, x9
10002d8e4:     	mov	w9, #0x7f80             ; =32640
10002d8e8:     	cmp	x20, #0x0
10002d8ec:     	ccmp	x2, x9, #0x2, ne
10002d8f0:     	add	x9, x20, x2
10002d8f4:     	ccmp	x27, #0x0, #0x0, ls
10002d8f8:     	ccmp	x9, x11, #0x2, eq
10002d8fc:     	b.hi	 <L5>
10002d900:     	mov	x10, #0x7fffffffffffffff ; =9223372036854775807
10002d904:     	add	x10, x26, x10
10002d908:     	cmn	x10, #0x20
10002d90c:     	b.hs	 <L5>
10002d910:     	str	x9, [x8]
10002d914:     	adrp	x1, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002d918:     	mov	x0, x20
10002d91c:     	add	x1, x1, #0xa60
10002d920:     	bl	 <_scoop_runtime_finish_tlab_alloc>
10002d924:     	b	 <L6>
<L5>:
10002d928:     	ldr	x8, [sp, #0x40]
10002d92c:     	adrp	x0, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002d930:     	mov	w1, #0x20               ; =32
10002d934:     	str	x8, [sp, #0x10]
10002d938:     	add	x0, x0, #0xa60
10002d93c:     	bl	 <_scoop_runtime_alloc_slow>
10002d940:     	ldr	x8, [sp, #0x10]
10002d944:     	mov	x20, x0
10002d948:     	str	x8, [sp, #0x40]
10002d94c:     	str	xzr, [sp, #0x18]
<L6>:
10002d950:     	ldr	x8, [sp, #0x18]
10002d954:     	mov	x9, x20
10002d958:     	mov	w10, #0x7               ; =7
10002d95c:     	str	x8, [x9, #0x18]!
10002d960:     	stur	x10, [x9, #-0x8]
10002d964:     	ldr	x8, [x25]
10002d968:     	add	x8, x8, x9, lsr #9
<L7>:
10002d96c:     	ldxrb	w9, [x8]
10002d970:     	orr	w9, w9, #0x1
10002d974:     	stxrb	w10, w9, [x8]
10002d978:     	cbnz	w10,  <L7>
10002d97c:     	mov	x0, x19
10002d980:     	str	x20, [sp, #0x38]
10002d984:     	str	xzr, [sp, #0x20]
10002d988:     	blr	x24
10002d98c:     	ldr	x8, [x0]
10002d990:     	neg	x9, x26
10002d994:     	add	x12, x26, #0x1f
10002d998:     	and	x2, x12, x9
10002d99c:     	ldp	x10, x11, [x8]
10002d9a0:     	add	x10, x26, x10
10002d9a4:     	sub	x10, x10, #0x1
10002d9a8:     	and	x20, x10, x9
10002d9ac:     	mov	w9, #0x7f80             ; =32640
10002d9b0:     	cmp	x20, #0x0
10002d9b4:     	ccmp	x2, x9, #0x2, ne
10002d9b8:     	add	x9, x20, x2
10002d9bc:     	ccmp	x27, #0x0, #0x0, ls
10002d9c0:     	ccmp	x9, x11, #0x2, eq
10002d9c4:     	b.hi	 <L8>
10002d9c8:     	mov	x10, #0x7fffffffffffffff ; =9223372036854775807
10002d9cc:     	add	x10, x26, x10
10002d9d0:     	cmn	x10, #0x20
10002d9d4:     	b.hs	 <L8>
10002d9d8:     	str	x9, [x8]
10002d9dc:     	adrp	x1, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002d9e0:     	mov	x0, x20
10002d9e4:     	add	x1, x1, #0xa60
10002d9e8:     	bl	 <_scoop_runtime_finish_tlab_alloc>
10002d9ec:     	b	 <L9>
<L8>:
10002d9f0:     	ldp	x9, x8, [sp, #0x38]
10002d9f4:     	adrp	x0, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002d9f8:     	mov	w1, #0x20               ; =32
10002d9fc:     	stp	x9, x8, [sp, #0x8]
10002da00:     	add	x0, x0, #0xa60
10002da04:     	bl	 <_scoop_runtime_alloc_slow>
10002da08:     	ldp	x9, x8, [sp, #0x8]
10002da0c:     	mov	x20, x0
10002da10:     	str	x8, [sp, #0x40]
10002da14:     	str	x9, [sp, #0x38]
10002da18:     	str	xzr, [sp, #0x20]
<L9>:
10002da1c:     	ldr	x8, [sp, #0x20]
10002da20:     	mov	x9, x20
10002da24:     	mov	w10, #0x9               ; =9
10002da28:     	str	x8, [x9, #0x18]!
10002da2c:     	stur	x10, [x9, #-0x8]
10002da30:     	ldr	x8, [x25]
10002da34:     	add	x8, x8, x9, lsr #9
<L10>:
10002da38:     	ldxrb	w9, [x8]
10002da3c:     	orr	w9, w9, #0x1
10002da40:     	stxrb	w10, w9, [x8]
10002da44:     	cbnz	w10,  <L10>
10002da48:     	ldp	x9, x8, [sp, #0x38]
10002da4c:     	str	x20, [sp]
10002da50:     	stp	x9, x8, [sp, #0x8]
10002da54:     	bl	 <_scoop_rt_gc_collect>
10002da58:     	ldp	x10, x8, [sp, #0x8]
10002da5c:     	ldr	x9, [sp]
10002da60:     	str	x8, [sp, #0x40]
10002da64:     	str	x10, [sp, #0x38]
10002da68:     	mov	w26, #0x1200            ; =4608
10002da6c:     	mov	x20, xzr
10002da70:     	add	x27, sp, #0x30
10002da74:     	movk	w26, #0x7a, lsl #16
10002da78:     	add	x28, sp, #0x38
10002da7c:     	str	x9, [sp, #0x30]
<L11>:
10002da80:     	ldar	x8, [x21]
10002da84:     	ldar	w9, [x23]
10002da88:     	add	x11, x22, #0x8
10002da8c:     	ldar	w10, [x22]
10002da90:     	ldar	x11, [x11]
10002da94:     	cmp	w9, #0x0
10002da98:     	ccmp	w10, #0x1, #0x0, eq
10002da9c:     	ccmp	x11, x8, #0x0, eq
10002daa0:     	b.eq	 <L12>
10002daa4:     	ldp	x9, x8, [sp, #0x38]
10002daa8:     	ldr	x10, [sp, #0x30]
10002daac:     	str	x10, [sp]
10002dab0:     	stp	x9, x8, [sp, #0x8]
10002dab4:     	bl	 <_scoop_rt_safepoint>
10002dab8:     	ldp	x10, x8, [sp, #0x8]
10002dabc:     	ldr	x9, [sp]
10002dac0:     	str	x8, [sp, #0x40]
10002dac4:     	str	x10, [sp, #0x38]
10002dac8:     	str	x9, [sp, #0x30]
<L12>:
10002dacc:     	cmp	x20, x26
10002dad0:     	b.ge	 <L14>
10002dad4:     	tst	x20, #0x1
10002dad8:     	ldr	x9, [sp, #0x40]
10002dadc:     	csel	x8, x28, x27, eq
10002dae0:     	ldr	x8, [x8]
10002dae4:     	str	x8, [x9, #0x18]!
10002dae8:     	ldr	x8, [x25]
10002daec:     	add	x8, x8, x9, lsr #9
<L13>:
10002daf0:     	ldxrb	w9, [x8]
10002daf4:     	orr	w9, w9, #0x1
10002daf8:     	stxrb	w10, w9, [x8]
10002dafc:     	cbnz	w10,  <L13>
10002db00:     	add	x20, x20, #0x1
10002db04:     	b	 <L11>
<L14>:
10002db08:     	ldr	x8, [sp, #0x40]
10002db0c:     	ldr	x8, [x8, #0x18]
10002db10:     	cbz	x8,  <L15>
10002db14:     	ldr	x0, [x8, #0x10]
10002db18:     	bl	 <dyld_stub_binder+0x1000452e8>
10002db1c:     	ldp	x29, x30, [sp, #0xa0]
10002db20:     	ldp	x20, x19, [sp, #0x90]
10002db24:     	ldp	x22, x21, [sp, #0x80]
10002db28:     	ldp	x24, x23, [sp, #0x70]
10002db2c:     	ldp	x26, x25, [sp, #0x60]
10002db30:     	ldp	x28, x27, [sp, #0x50]
10002db34:     	add	sp, sp, #0xb0
10002db38:     	ret
<L15>:
10002db3c:     	adrp	x10, 0x10007b000 <_scoop$1$td$e638df6f9e8c6a840aa29927372221a41f2ca63154c39dd998c45459b2938e46+0x40>
10002db40:     	mov	x0, x19
10002db44:     	add	x10, x10, #0xa60
10002db48:     	ldr	x9, [x10, #0x18]
10002db4c:     	blr	x24
10002db50:     	ldr	x8, [x0]
10002db54:     	neg	x11, x9
10002db58:     	ldr	x12, [x8]
10002db5c:     	add	x12, x9, x12
10002db60:     	sub	x12, x12, #0x1
10002db64:     	ands	x19, x12, x11
10002db68:     	b.eq	 <L16>
10002db6c:     	add	x12, x9, #0x17
10002db70:     	and	x2, x12, x11
10002db74:     	mov	w11, #0x7f80            ; =32640
10002db78:     	cmp	x2, x11
10002db7c:     	b.hi	 <L16>
10002db80:     	ldr	x10, [x10, #0x90]
10002db84:     	cbnz	x10,  <L16>
10002db88:     	ldr	x11, [x8, #0x8]
10002db8c:     	add	x10, x19, x2
10002db90:     	cmp	x10, x11
10002db94:     	b.hi	 <L16>
10002db98:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10002db9c:     	add	x9, x9, x11
10002dba0:     	cmn	x9, #0x18
10002dba4:     	b.hs	 <L16>
10002dba8:     	str	x10, [x8]
10002dbac:     	adrp	x1, 0x10007b000 <_scoop$1$td$e638df6f9e8c6a840aa29927372221a41f2ca63154c39dd998c45459b2938e46+0x40>
10002dbb0:     	mov	x0, x19
10002dbb4:     	add	x1, x1, #0xa60
10002dbb8:     	bl	 <_scoop_runtime_finish_tlab_alloc>
10002dbbc:     	b	 <L17>
<L16>:
10002dbc0:     	adrp	x0, 0x10007b000 <_scoop$1$td$e638df6f9e8c6a840aa29927372221a41f2ca63154c39dd998c45459b2938e46+0x40>
10002dbc4:     	mov	w1, #0x18               ; =24
10002dbc8:     	add	x0, x0, #0xa60
10002dbcc:     	bl	 <_scoop_runtime_alloc_slow>
10002dbd0:     	mov	x19, x0
<L17>:
10002dbd4:     	mov	x0, x19
10002dbd8:     	str	x19, [sp, #0x10]
10002dbdc:     	bl	 <_scoop$1$cb$a743fee7ba2831ad1852cbb01a2d484559872cb6aa9aa7bc66d5e03510bf8243>
10002dbe0:     	ldr	x0, [sp, #0x10]
10002dbe4:     	str	x0, [sp, #0x28]
10002dbe8:     	bl	 <_scoop_rt_throw>

000000010002f8f8 <_scoop_runtime_finish_tlab_alloc>:
10002f8f8:     	stp	x22, x21, [sp, #-0x30]!
10002f8fc:     	stp	x20, x19, [sp, #0x10]
10002f900:     	stp	x29, x30, [sp, #0x20]
10002f904:     	add	x29, sp, #0x20
10002f908:     	mov	x21, x2
10002f90c:     	mov	x19, x1
10002f910:     	mov	x20, x0
10002f914:     	bl	 <_scoop_gc_stress_move_enabled>
10002f918:     	tbnz	w0, #0x0,  <L0>
10002f91c:     	ldr	x8, [x19, #0x90]
10002f920:     	cbnz	x8,  <L0>
10002f924:     	bl	 <_scoop_thread_current_required>
10002f928:     	mov	x22, x0
10002f92c:     	mov	x0, x19
10002f930:     	mov	x1, x21
10002f934:     	bl	 <_scoop_shape_normalize_allocation>
10002f938:     	mov	x3, x0
10002f93c:     	mov	x0, x22
10002f940:     	mov	x1, x20
10002f944:     	mov	x2, x19
10002f948:     	bl	 <_finish_small_allocation>
10002f94c:     	ldp	x29, x30, [sp, #0x20]
10002f950:     	ldp	x20, x19, [sp, #0x10]
10002f954:     	ldp	x22, x21, [sp], #0x30
10002f958:     	ret
<L0>:
10002f95c:     	adrp	x0, 0x10005e000 <_scoop$1$bs$1de4b2aca1a66b496471d4b817b3c41017506ecf977b4ba9b2b850c01495f9aa+0x20>
10002f960:     	add	x0, x0, #0xa98
10002f964:     	bl	 <_scoop_heap_fatal>

0000000100037e68 <_scoop_rt_gc_write_barrier>:
100037e68:     	stp	x29, x30, [sp, #-0x10]!
100037e6c:     	mov	x29, sp
100037e70:     	cbz	x1,  <L1>
100037e74:     	adrp	x8, 0x100154000 <dyld_stub_binder+0x100154000>
100037e78:     	ldrb	w8, [x8, #0x4b0]
100037e7c:     	cmp	w8, #0x1
100037e80:     	b.ne	 <L2>
100037e84:     	adrp	x8, 0x100154000 <dyld_stub_binder+0x100154000>
100037e88:     	ldr	x8, [x8, #0x440]
100037e8c:     	cmp	x8, x0
100037e90:     	b.hi	 <L2>
100037e94:     	adrp	x9, 0x100154000 <dyld_stub_binder+0x100154000>
100037e98:     	ldr	x9, [x9, #0x448]
100037e9c:     	subs	x9, x9, x0
100037ea0:     	ccmp	x1, x9, #0x2, hi
100037ea4:     	b.hi	 <L2>
100037ea8:     	sub	x8, x0, x8
100037eac:     	add	x9, x1, x8
100037eb0:     	lsr	x8, x8, #9
100037eb4:     	sub	x9, x9, #0x1
100037eb8:     	lsr	x9, x9, #9
100037ebc:     	cmp	x8, x9
100037ec0:     	b.hi	 <L1>
100037ec4:     	add	x9, x9, #0x1
100037ec8:     	adrp	x10, 0x100154000 <dyld_stub_binder+0x100154000>
100037ecc:     	mov	w11, #0x1               ; =1
<L0>:
100037ed0:     	ldr	x12, [x10, #0x470]
100037ed4:     	add	x12, x12, x8
100037ed8:     	ldsetb	w11, w12, [x12]
100037edc:     	add	x8, x8, #0x1
100037ee0:     	cmp	x9, x8
100037ee4:     	b.ne	 <L0>
<L1>:
100037ee8:     	ldp	x29, x30, [sp], #0x10
100037eec:     	ret
<L2>:
100037ef0:     	bl	 <_scoop_rt_gc_write_barrier.cold.1>
