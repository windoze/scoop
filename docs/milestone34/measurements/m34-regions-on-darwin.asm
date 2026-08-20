
/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/regions-on-darwin/region-stores:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002e5a8 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
10002e5a8:     	sub	sp, sp, #0xb0
10002e5ac:     	stp	x28, x27, [sp, #0x50]
10002e5b0:     	stp	x26, x25, [sp, #0x60]
10002e5b4:     	stp	x24, x23, [sp, #0x70]
10002e5b8:     	stp	x22, x21, [sp, #0x80]
10002e5bc:     	stp	x20, x19, [sp, #0x90]
10002e5c0:     	stp	x29, x30, [sp, #0xa0]
10002e5c4:     	add	x29, sp, #0xa0
10002e5c8:     	adrp	x0, 0x100154000 <dyld_stub_binder+0x100154000>
10002e5cc:     	add	x0, x0, #0x600
10002e5d0:     	ldr	x8, [x0]
10002e5d4:     	blr	x8
10002e5d8:     	adrp	x21, 0x100154000 <dyld_stub_binder+0x100154000>
10002e5dc:     	adrp	x23, 0x100154000 <dyld_stub_binder+0x100154000>
10002e5e0:     	add	x21, x21, #0x850
10002e5e4:     	ldr	x22, [x0]
10002e5e8:     	add	x23, x23, #0x83c
10002e5ec:     	add	x10, x22, #0x8
10002e5f0:     	ldar	x8, [x21]
10002e5f4:     	ldar	w11, [x23]
10002e5f8:     	ldar	w9, [x22]
10002e5fc:     	ldar	x10, [x10]
10002e600:     	cbnz	w11,  <L0>
10002e604:     	cmp	w9, #0x1
10002e608:     	ccmp	x10, x8, #0x0, eq
10002e60c:     	b.eq	 <L1>
<L0>:
10002e610:     	bl	 <_scoop_rt_safepoint>
<L1>:
10002e614:     	adrp	x19, 0x100154000 <dyld_stub_binder+0x100154000>
10002e618:     	adrp	x10, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002e61c:     	add	x19, x19, #0x5e8
10002e620:     	add	x10, x10, #0xa60
10002e624:     	str	xzr, [sp, #0x48]
10002e628:     	ldr	x24, [x19]
10002e62c:     	ldr	x26, [x10, #0x18]
10002e630:     	mov	x0, x19
10002e634:     	blr	x24
10002e638:     	ldr	x8, [x0]
10002e63c:     	neg	x9, x26
10002e640:     	ldr	x27, [x10, #0x90]
10002e644:     	ldr	x11, [x8]
10002e648:     	add	x11, x26, x11
10002e64c:     	sub	x11, x11, #0x1
10002e650:     	ands	x20, x11, x9
10002e654:     	b.eq	 <L2>
10002e658:     	add	x10, x26, #0x1f
10002e65c:     	and	x2, x10, x9
10002e660:     	mov	w9, #0x7f80             ; =32640
10002e664:     	cmp	x2, x9
10002e668:     	b.hi	 <L2>
10002e66c:     	cbnz	x27,  <L2>
10002e670:     	ldr	x10, [x8, #0x8]
10002e674:     	add	x9, x20, x2
10002e678:     	cmp	x9, x10
10002e67c:     	b.hi	 <L2>
10002e680:     	mov	x10, #0x7fffffffffffffff ; =9223372036854775807
10002e684:     	add	x10, x26, x10
10002e688:     	cmn	x10, #0x21
10002e68c:     	b.hi	 <L2>
10002e690:     	str	x9, [x8]
10002e694:     	adrp	x1, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002e698:     	mov	x0, x20
10002e69c:     	add	x1, x1, #0xa60
10002e6a0:     	bl	 <_scoop_runtime_finish_tlab_alloc>
10002e6a4:     	b	 <L3>
<L2>:
10002e6a8:     	adrp	x0, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002e6ac:     	mov	w1, #0x20               ; =32
10002e6b0:     	add	x0, x0, #0xa60
10002e6b4:     	bl	 <_scoop_runtime_alloc_slow>
10002e6b8:     	mov	x20, x0
10002e6bc:     	str	xzr, [sp, #0x48]
<L3>:
10002e6c0:     	ldr	x8, [sp, #0x48]
10002e6c4:     	mov	x9, x20
10002e6c8:     	adrp	x25, 0x100154000 <dyld_stub_binder+0x100154000>
10002e6cc:     	str	x8, [x9, #0x18]!
10002e6d0:     	lsr	x8, x9, #52
10002e6d4:     	add	x25, x25, #0x868
10002e6d8:     	stur	xzr, [x9, #-0x8]
10002e6dc:     	ubfx	x10, x9, #40, #12
10002e6e0:     	add	x8, x25, x8, lsl #3
10002e6e4:     	ldar	x8, [x8]
10002e6e8:     	add	x8, x8, x10, lsl #3
10002e6ec:     	ubfx	x10, x9, #28, #12
10002e6f0:     	ldar	x8, [x8]
10002e6f4:     	add	x8, x8, x10, lsl #3
10002e6f8:     	ubfx	x10, x9, #16, #12
10002e6fc:     	ldar	x8, [x8]
10002e700:     	add	x8, x8, x10, lsl #3
10002e704:     	ldar	x8, [x8]
10002e708:     	ldr	x10, [x8]
10002e70c:     	ldr	x8, [x8, #0x10]
10002e710:     	sub	x9, x9, x10
10002e714:     	add	x8, x8, x9, lsr #9
<L4>:
10002e718:     	ldxrb	w9, [x8]
10002e71c:     	orr	w9, w9, #0x1
10002e720:     	stxrb	w10, w9, [x8]
10002e724:     	cbnz	w10,  <L4>
10002e728:     	mov	x0, x19
10002e72c:     	str	x20, [sp, #0x40]
10002e730:     	str	xzr, [sp, #0x18]
10002e734:     	blr	x24
10002e738:     	ldr	x8, [x0]
10002e73c:     	neg	x9, x26
10002e740:     	add	x12, x26, #0x1f
10002e744:     	and	x2, x12, x9
10002e748:     	ldp	x10, x11, [x8]
10002e74c:     	add	x10, x26, x10
10002e750:     	sub	x10, x10, #0x1
10002e754:     	and	x20, x10, x9
10002e758:     	mov	w9, #0x7f80             ; =32640
10002e75c:     	cmp	x20, #0x0
10002e760:     	ccmp	x2, x9, #0x2, ne
10002e764:     	add	x9, x20, x2
10002e768:     	ccmp	x27, #0x0, #0x0, ls
10002e76c:     	ccmp	x9, x11, #0x2, eq
10002e770:     	b.hi	 <L5>
10002e774:     	mov	x10, #0x7fffffffffffffff ; =9223372036854775807
10002e778:     	add	x10, x26, x10
10002e77c:     	cmn	x10, #0x20
10002e780:     	b.hs	 <L5>
10002e784:     	str	x9, [x8]
10002e788:     	adrp	x1, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002e78c:     	mov	x0, x20
10002e790:     	add	x1, x1, #0xa60
10002e794:     	bl	 <_scoop_runtime_finish_tlab_alloc>
10002e798:     	b	 <L6>
<L5>:
10002e79c:     	ldr	x8, [sp, #0x40]
10002e7a0:     	adrp	x0, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002e7a4:     	mov	w1, #0x20               ; =32
10002e7a8:     	str	x8, [sp, #0x10]
10002e7ac:     	add	x0, x0, #0xa60
10002e7b0:     	bl	 <_scoop_runtime_alloc_slow>
10002e7b4:     	ldr	x8, [sp, #0x10]
10002e7b8:     	mov	x20, x0
10002e7bc:     	str	x8, [sp, #0x40]
10002e7c0:     	str	xzr, [sp, #0x18]
<L6>:
10002e7c4:     	ldr	x8, [sp, #0x18]
10002e7c8:     	mov	x9, x20
10002e7cc:     	mov	w10, #0x7               ; =7
10002e7d0:     	str	x8, [x9, #0x18]!
10002e7d4:     	lsr	x8, x9, #52
10002e7d8:     	stur	x10, [x9, #-0x8]
10002e7dc:     	ubfx	x10, x9, #40, #12
10002e7e0:     	add	x8, x25, x8, lsl #3
10002e7e4:     	ldar	x8, [x8]
10002e7e8:     	add	x8, x8, x10, lsl #3
10002e7ec:     	ubfx	x10, x9, #28, #12
10002e7f0:     	ldar	x8, [x8]
10002e7f4:     	add	x8, x8, x10, lsl #3
10002e7f8:     	ubfx	x10, x9, #16, #12
10002e7fc:     	ldar	x8, [x8]
10002e800:     	add	x8, x8, x10, lsl #3
10002e804:     	ldar	x8, [x8]
10002e808:     	ldr	x10, [x8]
10002e80c:     	ldr	x8, [x8, #0x10]
10002e810:     	sub	x9, x9, x10
10002e814:     	add	x8, x8, x9, lsr #9
<L7>:
10002e818:     	ldxrb	w9, [x8]
10002e81c:     	orr	w9, w9, #0x1
10002e820:     	stxrb	w10, w9, [x8]
10002e824:     	cbnz	w10,  <L7>
10002e828:     	mov	x0, x19
10002e82c:     	str	x20, [sp, #0x38]
10002e830:     	str	xzr, [sp, #0x20]
10002e834:     	blr	x24
10002e838:     	ldr	x8, [x0]
10002e83c:     	neg	x9, x26
10002e840:     	add	x12, x26, #0x1f
10002e844:     	and	x2, x12, x9
10002e848:     	ldp	x10, x11, [x8]
10002e84c:     	add	x10, x26, x10
10002e850:     	sub	x10, x10, #0x1
10002e854:     	and	x20, x10, x9
10002e858:     	mov	w9, #0x7f80             ; =32640
10002e85c:     	cmp	x20, #0x0
10002e860:     	ccmp	x2, x9, #0x2, ne
10002e864:     	add	x9, x20, x2
10002e868:     	ccmp	x27, #0x0, #0x0, ls
10002e86c:     	ccmp	x9, x11, #0x2, eq
10002e870:     	b.hi	 <L8>
10002e874:     	mov	x10, #0x7fffffffffffffff ; =9223372036854775807
10002e878:     	add	x10, x26, x10
10002e87c:     	cmn	x10, #0x20
10002e880:     	b.hs	 <L8>
10002e884:     	str	x9, [x8]
10002e888:     	adrp	x1, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002e88c:     	mov	x0, x20
10002e890:     	add	x1, x1, #0xa60
10002e894:     	bl	 <_scoop_runtime_finish_tlab_alloc>
10002e898:     	b	 <L9>
<L8>:
10002e89c:     	ldp	x9, x8, [sp, #0x38]
10002e8a0:     	adrp	x0, 0x100126000 <_scoop$1$cr$4f16be18560dd81ba2e722769344b3d78076b999f9e77f696a424ea4fb34b1b0+0x20>
10002e8a4:     	mov	w1, #0x20               ; =32
10002e8a8:     	stp	x9, x8, [sp, #0x8]
10002e8ac:     	add	x0, x0, #0xa60
10002e8b0:     	bl	 <_scoop_runtime_alloc_slow>
10002e8b4:     	ldp	x9, x8, [sp, #0x8]
10002e8b8:     	mov	x20, x0
10002e8bc:     	str	x8, [sp, #0x40]
10002e8c0:     	str	x9, [sp, #0x38]
10002e8c4:     	str	xzr, [sp, #0x20]
<L9>:
10002e8c8:     	ldr	x8, [sp, #0x20]
10002e8cc:     	mov	x9, x20
10002e8d0:     	mov	w10, #0x9               ; =9
10002e8d4:     	str	x8, [x9, #0x18]!
10002e8d8:     	lsr	x8, x9, #52
10002e8dc:     	stur	x10, [x9, #-0x8]
10002e8e0:     	ubfx	x10, x9, #40, #12
10002e8e4:     	add	x8, x25, x8, lsl #3
10002e8e8:     	ldar	x8, [x8]
10002e8ec:     	add	x8, x8, x10, lsl #3
10002e8f0:     	ubfx	x10, x9, #28, #12
10002e8f4:     	ldar	x8, [x8]
10002e8f8:     	add	x8, x8, x10, lsl #3
10002e8fc:     	ubfx	x10, x9, #16, #12
10002e900:     	ldar	x8, [x8]
10002e904:     	add	x8, x8, x10, lsl #3
10002e908:     	ldar	x8, [x8]
10002e90c:     	ldr	x10, [x8]
10002e910:     	ldr	x8, [x8, #0x10]
10002e914:     	sub	x9, x9, x10
10002e918:     	add	x8, x8, x9, lsr #9
<L10>:
10002e91c:     	ldxrb	w9, [x8]
10002e920:     	orr	w9, w9, #0x1
10002e924:     	stxrb	w10, w9, [x8]
10002e928:     	cbnz	w10,  <L10>
10002e92c:     	ldp	x9, x8, [sp, #0x38]
10002e930:     	str	x20, [sp]
10002e934:     	stp	x9, x8, [sp, #0x8]
10002e938:     	bl	 <_scoop_rt_gc_collect>
10002e93c:     	ldp	x10, x8, [sp, #0x8]
10002e940:     	ldr	x9, [sp]
10002e944:     	str	x8, [sp, #0x40]
10002e948:     	str	x10, [sp, #0x38]
10002e94c:     	mov	w26, #0x1200            ; =4608
10002e950:     	mov	x20, xzr
10002e954:     	add	x27, sp, #0x30
10002e958:     	movk	w26, #0x7a, lsl #16
10002e95c:     	add	x28, sp, #0x38
10002e960:     	str	x9, [sp, #0x30]
<L11>:
10002e964:     	ldar	x8, [x21]
10002e968:     	ldar	w9, [x23]
10002e96c:     	add	x11, x22, #0x8
10002e970:     	ldar	w10, [x22]
10002e974:     	ldar	x11, [x11]
10002e978:     	cmp	w9, #0x0
10002e97c:     	ccmp	w10, #0x1, #0x0, eq
10002e980:     	ccmp	x11, x8, #0x0, eq
10002e984:     	b.eq	 <L12>
10002e988:     	ldp	x9, x8, [sp, #0x38]
10002e98c:     	ldr	x10, [sp, #0x30]
10002e990:     	str	x10, [sp]
10002e994:     	stp	x9, x8, [sp, #0x8]
10002e998:     	bl	 <_scoop_rt_safepoint>
10002e99c:     	ldp	x10, x8, [sp, #0x8]
10002e9a0:     	ldr	x9, [sp]
10002e9a4:     	str	x8, [sp, #0x40]
10002e9a8:     	str	x10, [sp, #0x38]
10002e9ac:     	str	x9, [sp, #0x30]
<L12>:
10002e9b0:     	cmp	x20, x26
10002e9b4:     	b.ge	 <L14>
10002e9b8:     	tst	x20, #0x1
10002e9bc:     	ldr	x9, [sp, #0x40]
10002e9c0:     	csel	x8, x28, x27, eq
10002e9c4:     	ldr	x8, [x8]
10002e9c8:     	str	x8, [x9, #0x18]!
10002e9cc:     	lsr	x8, x9, #52
10002e9d0:     	ubfx	x10, x9, #40, #12
10002e9d4:     	add	x8, x25, x8, lsl #3
10002e9d8:     	ldar	x8, [x8]
10002e9dc:     	add	x8, x8, x10, lsl #3
10002e9e0:     	ubfx	x10, x9, #28, #12
10002e9e4:     	ldar	x8, [x8]
10002e9e8:     	add	x8, x8, x10, lsl #3
10002e9ec:     	ubfx	x10, x9, #16, #12
10002e9f0:     	ldar	x8, [x8]
10002e9f4:     	add	x8, x8, x10, lsl #3
10002e9f8:     	ldar	x8, [x8]
10002e9fc:     	ldr	x10, [x8]
10002ea00:     	ldr	x8, [x8, #0x10]
10002ea04:     	sub	x9, x9, x10
10002ea08:     	add	x8, x8, x9, lsr #9
<L13>:
10002ea0c:     	ldxrb	w9, [x8]
10002ea10:     	orr	w9, w9, #0x1
10002ea14:     	stxrb	w10, w9, [x8]
10002ea18:     	cbnz	w10,  <L13>
10002ea1c:     	add	x20, x20, #0x1
10002ea20:     	b	 <L11>
<L14>:
10002ea24:     	ldr	x8, [sp, #0x40]
10002ea28:     	ldr	x8, [x8, #0x18]
10002ea2c:     	cbz	x8,  <L15>
10002ea30:     	ldr	x0, [x8, #0x10]
10002ea34:     	bl	 <dyld_stub_binder+0x100046210>
10002ea38:     	ldp	x29, x30, [sp, #0xa0]
10002ea3c:     	ldp	x20, x19, [sp, #0x90]
10002ea40:     	ldp	x22, x21, [sp, #0x80]
10002ea44:     	ldp	x24, x23, [sp, #0x70]
10002ea48:     	ldp	x26, x25, [sp, #0x60]
10002ea4c:     	ldp	x28, x27, [sp, #0x50]
10002ea50:     	add	sp, sp, #0xb0
10002ea54:     	ret
<L15>:
10002ea58:     	adrp	x10, 0x10007b000 <_scoop$1$td$e638df6f9e8c6a840aa29927372221a41f2ca63154c39dd998c45459b2938e46+0x40>
10002ea5c:     	mov	x0, x19
10002ea60:     	add	x10, x10, #0xa60
10002ea64:     	ldr	x9, [x10, #0x18]
10002ea68:     	blr	x24
10002ea6c:     	ldr	x8, [x0]
10002ea70:     	neg	x11, x9
10002ea74:     	ldr	x12, [x8]
10002ea78:     	add	x12, x9, x12
10002ea7c:     	sub	x12, x12, #0x1
10002ea80:     	ands	x19, x12, x11
10002ea84:     	b.eq	 <L16>
10002ea88:     	add	x12, x9, #0x17
10002ea8c:     	and	x2, x12, x11
10002ea90:     	mov	w11, #0x7f80            ; =32640
10002ea94:     	cmp	x2, x11
10002ea98:     	b.hi	 <L16>
10002ea9c:     	ldr	x10, [x10, #0x90]
10002eaa0:     	cbnz	x10,  <L16>
10002eaa4:     	ldr	x11, [x8, #0x8]
10002eaa8:     	add	x10, x19, x2
10002eaac:     	cmp	x10, x11
10002eab0:     	b.hi	 <L16>
10002eab4:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10002eab8:     	add	x9, x9, x11
10002eabc:     	cmn	x9, #0x18
10002eac0:     	b.hs	 <L16>
10002eac4:     	str	x10, [x8]
10002eac8:     	adrp	x1, 0x10007b000 <_scoop$1$td$e638df6f9e8c6a840aa29927372221a41f2ca63154c39dd998c45459b2938e46+0x40>
10002eacc:     	mov	x0, x19
10002ead0:     	add	x1, x1, #0xa60
10002ead4:     	bl	 <_scoop_runtime_finish_tlab_alloc>
10002ead8:     	b	 <L17>
<L16>:
10002eadc:     	adrp	x0, 0x10007b000 <_scoop$1$td$e638df6f9e8c6a840aa29927372221a41f2ca63154c39dd998c45459b2938e46+0x40>
10002eae0:     	mov	w1, #0x18               ; =24
10002eae4:     	add	x0, x0, #0xa60
10002eae8:     	bl	 <_scoop_runtime_alloc_slow>
10002eaec:     	mov	x19, x0
<L17>:
10002eaf0:     	mov	x0, x19
10002eaf4:     	str	x19, [sp, #0x10]
10002eaf8:     	bl	 <_scoop$1$cb$a743fee7ba2831ad1852cbb01a2d484559872cb6aa9aa7bc66d5e03510bf8243>
10002eafc:     	ldr	x0, [sp, #0x10]
10002eb00:     	str	x0, [sp, #0x28]
10002eb04:     	bl	 <_scoop_rt_throw>

0000000100035e80 <_scoop_runtime_finish_tlab_alloc>:
100035e80:     	stp	x22, x21, [sp, #-0x30]!
100035e84:     	stp	x20, x19, [sp, #0x10]
100035e88:     	stp	x29, x30, [sp, #0x20]
100035e8c:     	add	x29, sp, #0x20
100035e90:     	mov	x21, x2
100035e94:     	mov	x19, x1
100035e98:     	mov	x20, x0
100035e9c:     	bl	 <_scoop_gc_stress_move_enabled>
100035ea0:     	tbnz	w0, #0x0,  <L0>
100035ea4:     	ldr	x8, [x19, #0x90]
100035ea8:     	cbnz	x8,  <L0>
100035eac:     	bl	 <_scoop_thread_current_required>
100035eb0:     	mov	x22, x0
100035eb4:     	mov	x0, x19
100035eb8:     	mov	x1, x21
100035ebc:     	bl	 <_scoop_shape_normalize_allocation>
100035ec0:     	mov	x3, x0
100035ec4:     	mov	x0, x22
100035ec8:     	mov	x1, x20
100035ecc:     	mov	x2, x19
100035ed0:     	bl	 <_finish_small_allocation>
100035ed4:     	ldp	x29, x30, [sp, #0x20]
100035ed8:     	ldp	x20, x19, [sp, #0x10]
100035edc:     	ldp	x22, x21, [sp], #0x30
100035ee0:     	ret
<L0>:
100035ee4:     	adrp	x0, 0x100060000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0xda0>
100035ee8:     	add	x0, x0, #0xb11
100035eec:     	bl	 <_scoop_heap_fatal>

0000000100037c1c <_scoop_rt_gc_write_barrier>:
100037c1c:     	stp	x22, x21, [sp, #-0x30]!
100037c20:     	stp	x20, x19, [sp, #0x10]
100037c24:     	stp	x29, x30, [sp, #0x20]
100037c28:     	add	x29, sp, #0x20
100037c2c:     	cmn	x0, x1
100037c30:     	b.hs	 <L5>
100037c34:     	mov	x19, x1
100037c38:     	cbz	x1,  <L3>
100037c3c:     	mov	x20, x0
100037c40:     	mov	w21, #0x1               ; =1
100037c44:     	b	 <L1>
<L0>:
100037c48:     	add	x20, x8, x20
100037c4c:     	subs	x19, x19, x8
100037c50:     	b.eq	 <L3>
<L1>:
100037c54:     	mov	x0, x20
100037c58:     	bl	 <_scoop_heap_region_for_address>
100037c5c:     	cbz	x0,  <L4>
100037c60:     	ldp	x8, x9, [x0]
100037c64:     	sub	x10, x20, x8
100037c68:     	sub	x8, x9, x10
100037c6c:     	cmp	x8, x19
100037c70:     	csel	x8, x8, x19, lo
100037c74:     	lsr	x9, x10, #9
100037c78:     	add	x10, x10, x8
100037c7c:     	sub	x10, x10, #0x1
100037c80:     	lsr	x10, x10, #9
100037c84:     	cmp	x9, x10
100037c88:     	b.hi	 <L0>
100037c8c:     	add	x10, x10, #0x1
<L2>:
100037c90:     	ldr	x11, [x0, #0x10]
100037c94:     	add	x11, x11, x9
100037c98:     	ldsetb	w21, w11, [x11]
100037c9c:     	add	x9, x9, #0x1
100037ca0:     	cmp	x10, x9
100037ca4:     	b.ne	 <L2>
100037ca8:     	b	 <L0>
<L3>:
100037cac:     	ldp	x29, x30, [sp, #0x20]
100037cb0:     	ldp	x20, x19, [sp, #0x10]
100037cb4:     	ldp	x22, x21, [sp], #0x30
100037cb8:     	ret
<L4>:
100037cbc:     	adrp	x0, 0x100060000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0xda0>
100037cc0:     	add	x0, x0, #0xfca
100037cc4:     	bl	 <_scoop_heap_fatal>
<L5>:
100037cc8:     	adrp	x0, 0x100060000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0xda0>
100037ccc:     	add	x0, x0, #0xfac
100037cd0:     	bl	 <_scoop_heap_fatal>

000000010003faf8 <_scoop_heap_region_for_address>:
10003faf8:     	lsr	x8, x0, #52
10003fafc:     	adrp	x9, 0x100154000 <dyld_stub_binder+0x100154000>
10003fb00:     	add	x9, x9, #0x868
10003fb04:     	add	x8, x9, x8, lsl #3
10003fb08:     	ldapr	x8, [x8]
10003fb0c:     	cbz	x8,  <L0>
10003fb10:     	ubfx	x9, x0, #40, #12
10003fb14:     	add	x8, x8, x9, lsl #3
10003fb18:     	ldapr	x8, [x8]
10003fb1c:     	cbz	x8,  <L0>
10003fb20:     	ubfx	x9, x0, #28, #12
10003fb24:     	add	x8, x8, x9, lsl #3
10003fb28:     	ldapr	x8, [x8]
10003fb2c:     	cbz	x8,  <L0>
10003fb30:     	ubfx	x9, x0, #16, #12
10003fb34:     	add	x8, x8, x9, lsl #3
10003fb38:     	ldapr	x0, [x8]
10003fb3c:     	ret
<L0>:
10003fb40:     	mov	x0, #0x0                ; =0
10003fb44:     	ret
