
/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/reclamation-on-darwin/region-churn:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002ee54 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
10002ee54:     	sub	sp, sp, #0x100
10002ee58:     	stp	x26, x25, [sp, #0xb0]
10002ee5c:     	stp	x24, x23, [sp, #0xc0]
10002ee60:     	stp	x22, x21, [sp, #0xd0]
10002ee64:     	stp	x20, x19, [sp, #0xe0]
10002ee68:     	stp	x29, x30, [sp, #0xf0]
10002ee6c:     	add	x29, sp, #0xf0
10002ee70:     	adrp	x0, 0x10015c000 <dyld_stub_binder+0x10015c000>
10002ee74:     	add	x0, x0, #0x648
10002ee78:     	ldr	x8, [x0]
10002ee7c:     	blr	x8
10002ee80:     	adrp	x23, 0x10015c000 <dyld_stub_binder+0x10015c000>
10002ee84:     	adrp	x25, 0x10015c000 <dyld_stub_binder+0x10015c000>
10002ee88:     	add	x23, x23, #0x898
10002ee8c:     	ldr	x24, [x0]
10002ee90:     	add	x25, x25, #0x884
10002ee94:     	add	x10, x24, #0x8
10002ee98:     	ldar	x8, [x23]
10002ee9c:     	ldar	w11, [x25]
10002eea0:     	ldar	w9, [x24]
10002eea4:     	ldar	x10, [x10]
10002eea8:     	cbnz	w11,  <L0>
10002eeac:     	cmp	w9, #0x1
10002eeb0:     	ccmp	x10, x8, #0x0, eq
10002eeb4:     	b.eq	 <L1>
<L0>:
10002eeb8:     	bl	 <_scoop_rt_safepoint>
<L1>:
10002eebc:     	movi.2d	v0, #0000000000000000
10002eec0:     	mov	x0, sp
10002eec4:     	mov	x1, xzr
10002eec8:     	mov	x2, xzr
10002eecc:     	str	xzr, [sp, #0x10]
10002eed0:     	str	q0, [sp]
10002eed4:     	bl	 <_scoop_rt_push_caller_roots>
10002eed8:     	movi.2d	v0, #0000000000000000
10002eedc:     	add	x0, sp, #0x18
10002eee0:     	add	x1, sp, #0x18
10002eee4:     	stur	q0, [sp, #0x18]
10002eee8:     	stur	q0, [sp, #0x28]
10002eeec:     	stur	q0, [sp, #0x38]
10002eef0:     	stur	q0, [sp, #0x48]
10002eef4:     	bl	 <_scoop_rt_enter_native_safe>
10002eef8:     	bl	 <_m34_memory_pins>
10002eefc:     	mov	w19, w0
10002ef00:     	add	x0, sp, #0x18
10002ef04:     	bl	 <_scoop_rt_leave_native_safe>
10002ef08:     	mov	x0, sp
10002ef0c:     	bl	 <_scoop_rt_pop_caller_roots>
10002ef10:     	mov	w21, wzr
10002ef14:     	mov	x20, xzr
10002ef18:     	mov	w22, #0x2               ; =2
<L2>:
10002ef1c:     	ldar	x8, [x23]
10002ef20:     	ldar	w9, [x25]
10002ef24:     	add	x11, x24, #0x8
10002ef28:     	ldar	w10, [x24]
10002ef2c:     	ldar	x11, [x11]
10002ef30:     	cmp	w9, #0x0
10002ef34:     	ccmp	w10, #0x1, #0x0, eq
10002ef38:     	ccmp	x11, x8, #0x0, eq
10002ef3c:     	b.eq	 <L3>
10002ef40:     	bl	 <_scoop_rt_safepoint>
<L3>:
10002ef44:     	cmp	w21, #0x2
10002ef48:     	b.ge	 <L4>
10002ef4c:     	mov	w0, w21
10002ef50:     	mov	w1, w19
10002ef54:     	bl	 <_scoop$1$cb$9fcafbb211d8f976a0bd7b23d8482b48f61bd5b743ce18a7cd5abcb7f9166e4d>
10002ef58:     	add	x20, x20, x0
10002ef5c:     	bl	 <_scoop_rt_gc_collect>
10002ef60:     	movi.2d	v0, #0000000000000000
10002ef64:     	add	x0, sp, #0x58
10002ef68:     	mov	x1, xzr
10002ef6c:     	mov	x2, xzr
10002ef70:     	str	xzr, [sp, #0x68]
10002ef74:     	stur	q0, [sp, #0x58]
10002ef78:     	bl	 <_scoop_rt_push_caller_roots>
10002ef7c:     	movi.2d	v0, #0000000000000000
10002ef80:     	add	x0, sp, #0x70
10002ef84:     	add	x1, sp, #0x70
10002ef88:     	stp	q0, q0, [sp, #0x70]
10002ef8c:     	stp	q0, q0, [sp, #0x90]
10002ef90:     	bl	 <_scoop_rt_enter_native_safe>
10002ef94:     	mov	w0, w22
10002ef98:     	bl	 <_m34_memory_sample>
10002ef9c:     	add	x0, sp, #0x70
10002efa0:     	bl	 <_scoop_rt_leave_native_safe>
10002efa4:     	add	x0, sp, #0x58
10002efa8:     	bl	 <_scoop_rt_pop_caller_roots>
10002efac:     	add	w21, w21, #0x1
10002efb0:     	add	w22, w22, #0x3
10002efb4:     	b	 <L2>
<L4>:
10002efb8:     	mov	x0, x20
10002efbc:     	bl	 <dyld_stub_binder+0x10004839c>
10002efc0:     	ldp	x29, x30, [sp, #0xf0]
10002efc4:     	ldp	x20, x19, [sp, #0xe0]
10002efc8:     	ldp	x22, x21, [sp, #0xd0]
10002efcc:     	ldp	x24, x23, [sp, #0xc0]
10002efd0:     	ldp	x26, x25, [sp, #0xb0]
10002efd4:     	add	sp, sp, #0x100
10002efd8:     	ret

00000001000325b4 <_scoop_heap_reclaim_regions>:
1000325b4:     	sub	sp, sp, #0xb0
1000325b8:     	stp	x28, x27, [sp, #0x50]
1000325bc:     	stp	x26, x25, [sp, #0x60]
1000325c0:     	stp	x24, x23, [sp, #0x70]
1000325c4:     	stp	x22, x21, [sp, #0x80]
1000325c8:     	stp	x20, x19, [sp, #0x90]
1000325cc:     	stp	x29, x30, [sp, #0xa0]
1000325d0:     	add	x29, sp, #0xa0
1000325d4:     	mov	x21, x0
1000325d8:     	bl	 <_scoop_platform_bundle>
1000325dc:     	ldr	x26, [x0, #0x8]
1000325e0:     	cbz	w21,  <L0>
1000325e4:     	ldr	x8, [x26, #0x10]
1000325e8:     	blr	x8
1000325ec:     	mov	x15, x0
1000325f0:     	b	 <L1>
<L0>:
1000325f4:     	mov	w15, #0x8000            ; =32768
<L1>:
1000325f8:     	cbz	x15,  <L53>
1000325fc:     	and	x8, x15, #0x7f
100032600:     	cbnz	x8,  <L53>
100032604:     	mov	w8, #0x8000             ; =32768
100032608:     	udiv	x9, x8, x15
10003260c:     	msub	x8, x9, x15, x8
100032610:     	cbnz	x8,  <L53>
100032614:     	adrp	x22, 0x10015c000 <dyld_stub_binder+0x10015c000>
100032618:     	add	x22, x22, #0x418
10003261c:     	cbz	w21,  <L2>
100032620:     	stp	xzr, xzr, [x22, #0x48]
<L2>:
100032624:     	ldr	x17, [x22, #0x40]
100032628:     	cbz	x17,  <L49>
10003262c:     	mov	x19, #0x0               ; =0
100032630:     	mov	w16, #0x0               ; =0
100032634:     	add	x20, x22, #0x40
100032638:     	str	x15, [sp, #0x30]
10003263c:     	str	w21, [sp, #0x4]
100032640:     	str	x26, [sp, #0x20]
100032644:     	b	 <L4>
<L3>:
100032648:     	add	x20, x17, #0x18
10003264c:     	ldr	x17, [x20]
100032650:     	cbz	x17,  <L50>
<L4>:
100032654:     	ldrb	w8, [x17, #0x2a]
100032658:     	cmp	w8, #0x1
10003265c:     	b.ne	 <L6>
100032660:     	ldr	x8, [x17, #0x20]
100032664:     	ldr	w8, [x8, #0x14]
100032668:     	cmp	w8, #0x1
10003266c:     	b.ne	 <L3>
<L5>:
100032670:     	ldr	x8, [x17, #0x18]
100032674:     	str	x8, [x20]
100032678:     	mov	x0, x17
10003267c:     	bl	 <_scoop_heap_region_destroy>
100032680:     	ldr	x15, [sp, #0x30]
100032684:     	mov	w16, #0x1               ; =1
100032688:     	ldr	x17, [x20]
10003268c:     	cbnz	x17,  <L4>
100032690:     	b	 <L50>
<L6>:
100032694:     	cbz	w21,  <L3>
100032698:     	ldrh	w8, [x17, #0x28]
10003269c:     	cbz	x8,  <L8>
1000326a0:     	ldr	x9, [x17, #0x20]
1000326a4:     	add	x9, x9, #0x14
<L7>:
1000326a8:     	ldr	w10, [x9], #0x88
1000326ac:     	cmp	w10, #0x2
1000326b0:     	b.hs	 <L9>
1000326b4:     	subs	x8, x8, #0x1
1000326b8:     	b.ne	 <L7>
1000326bc:     	mov	x8, x17
1000326c0:     	cbnz	x19,  <L5>
1000326c4:     	b	 <L10>
<L8>:
1000326c8:     	cbnz	x19,  <L5>
1000326cc:     	mov	x8, #0x0                ; =0
1000326d0:     	mov	w27, #0x1               ; =1
1000326d4:     	mov	x19, x17
1000326d8:     	mov	w0, #0x1                ; =1
1000326dc:     	tbnz	w27, #0x0,  <L28>
1000326e0:     	b	 <L29>
<L9>:
1000326e4:     	mov	x8, x19
<L10>:
1000326e8:     	str	x8, [sp, #0x8]
1000326ec:     	str	w16, [sp, #0x14]
1000326f0:     	mov	x10, #0x0               ; =0
1000326f4:     	ldr	x19, [x17]
1000326f8:     	mov	w27, #0x1               ; =1
1000326fc:     	mov	x22, x19
100032700:     	str	x17, [sp, #0x28]
100032704:     	b	 <L12>
<L11>:
100032708:     	ldr	x10, [sp, #0x18]
10003270c:     	add	x10, x10, #0x1
100032710:     	ldrh	w8, [x17, #0x28]
100032714:     	cmp	x10, x8
100032718:     	b.hs	 <L24>
<L12>:
10003271c:     	mov	x28, #0x0               ; =0
100032720:     	ldr	x8, [x17, #0x20]
100032724:     	mov	w9, #0x88               ; =136
100032728:     	madd	x23, x10, x9, x8
10003272c:     	str	x10, [sp, #0x18]
100032730:     	lsl	x25, x10, #15
100032734:     	mov	x20, x15
100032738:     	b	 <L16>
<L13>:
10003273c:     	mov	w0, #0x1                ; =1
<L14>:
100032740:     	and	w27, w27, w0
100032744:     	add	x22, x21, x15
100032748:     	mov	x19, x22
<L15>:
10003274c:     	add	x28, x28, x15
100032750:     	add	x20, x20, x15
100032754:     	cmp	x28, #0x8, lsl #12      ; =0x8000
100032758:     	b.hs	 <L11>
<L16>:
10003275c:     	ldr	x8, [x17]
100032760:     	add	x9, x28, x25
100032764:     	add	x21, x9, x8
100032768:     	ldrb	w8, [x23, #0x84]
10003276c:     	cmp	w8, #0x1
100032770:     	b.ne	 <L17>
100032774:     	ldr	w8, [x23, #0x14]
100032778:     	cmp	w8, #0x6
10003277c:     	b.ne	 <L18>
<L17>:
100032780:     	subs	x24, x22, x19
100032784:     	b.eq	 <L13>
100032788:     	stp	xzr, xzr, [sp, #0x38]
10003278c:     	str	xzr, [sp, #0x48]
100032790:     	ldr	x8, [x26, #0x28]
100032794:     	add	x2, sp, #0x38
100032798:     	mov	x0, x19
10003279c:     	mov	x1, x24
1000327a0:     	blr	x8
1000327a4:     	adrp	x9, 0x10015c000 <dyld_stub_binder+0x10015c000>
1000327a8:     	add	x9, x9, #0x418
1000327ac:     	tbz	w0, #0x0,  <L22>
1000327b0:     	ldr	x8, [x9, #0x128]
1000327b4:     	add	x8, x8, #0x1
1000327b8:     	str	x8, [x9, #0x128]
1000327bc:     	ldr	x8, [x9, #0x138]
1000327c0:     	add	x8, x8, x24
1000327c4:     	str	x8, [x9, #0x138]
1000327c8:     	b	 <L23>
<L18>:
1000327cc:     	mov	x0, x23
1000327d0:     	bl	 <_scoop_heap_active_head>
1000327d4:     	ldp	x17, x15, [sp, #0x28]
1000327d8:     	cbz	w0,  <L21>
1000327dc:     	lsr	x24, x28, #7
1000327e0:     	add	x8, x28, x15
1000327e4:     	cmp	x24, x8, lsr #7
1000327e8:     	b.hs	 <L21>
1000327ec:     	lsr	x8, x20, #7
1000327f0:     	sub	x26, x8, #0x1
<L19>:
1000327f4:     	ldr	x0, [x23, #0x40]
1000327f8:     	mov	x1, x24
1000327fc:     	bl	 <_scoop_heap_bit_test>
100032800:     	tbnz	w0, #0x0,  <L20>
100032804:     	cmp	x26, x24
100032808:     	add	x24, x24, #0x1
10003280c:     	b.ne	 <L19>
<L20>:
100032810:     	ldp	x26, x17, [sp, #0x20]
100032814:     	ldr	x15, [sp, #0x30]
100032818:     	tbnz	w0, #0x0,  <L17>
<L21>:
10003281c:     	cmp	x22, x19
100032820:     	csel	x19, x21, x19, eq
100032824:     	add	x22, x21, x15
100032828:     	b	 <L15>
<L22>:
10003282c:     	ldr	x8, [x9, #0x130]
100032830:     	add	x8, x8, #0x1
100032834:     	str	x8, [x9, #0x130]
<L23>:
100032838:     	ldp	x17, x15, [sp, #0x28]
10003283c:     	b	 <L14>
<L24>:
100032840:     	subs	x23, x22, x19
100032844:     	b.ne	 <L25>
100032848:     	mov	w0, #0x1                ; =1
10003284c:     	ldr	w21, [sp, #0x4]
100032850:     	adrp	x22, 0x10015c000 <dyld_stub_binder+0x10015c000>
100032854:     	add	x22, x22, #0x418
100032858:     	ldr	w16, [sp, #0x14]
10003285c:     	ldr	x19, [sp, #0x8]
100032860:     	tbnz	w27, #0x0,  <L28>
100032864:     	b	 <L29>
<L25>:
100032868:     	stp	xzr, xzr, [sp, #0x38]
10003286c:     	str	xzr, [sp, #0x48]
100032870:     	ldr	x8, [x26, #0x28]
100032874:     	add	x2, sp, #0x38
100032878:     	mov	x0, x19
10003287c:     	mov	x1, x23
100032880:     	blr	x8
100032884:     	ldr	w21, [sp, #0x4]
100032888:     	adrp	x22, 0x10015c000 <dyld_stub_binder+0x10015c000>
10003288c:     	add	x22, x22, #0x418
100032890:     	tbz	w0, #0x0,  <L26>
100032894:     	ldr	x8, [x22, #0x128]
100032898:     	add	x8, x8, #0x1
10003289c:     	str	x8, [x22, #0x128]
1000328a0:     	ldr	x8, [x22, #0x138]
1000328a4:     	add	x8, x8, x23
1000328a8:     	str	x8, [x22, #0x138]
1000328ac:     	b	 <L27>
<L26>:
1000328b0:     	ldr	x8, [x22, #0x130]
1000328b4:     	add	x8, x8, #0x1
1000328b8:     	str	x8, [x22, #0x130]
<L27>:
1000328bc:     	ldr	w16, [sp, #0x14]
1000328c0:     	ldp	x17, x15, [sp, #0x28]
1000328c4:     	ldr	x19, [sp, #0x8]
1000328c8:     	ldrh	w8, [x17, #0x28]
1000328cc:     	tbz	w27, #0x0,  <L29>
<L28>:
1000328d0:     	cbz	w0,  <L29>
1000328d4:     	cbz	x8,  <L3>
1000328d8:     	ldr	x9, [x17, #0x20]
1000328dc:     	cmp	x8, #0x4
1000328e0:     	b.hs	 <L30>
1000328e4:     	mov	x10, #0x0               ; =0
1000328e8:     	b	 <L32>
<L29>:
1000328ec:     	cbnz	x8,  <L34>
1000328f0:     	b	 <L3>
<L30>:
1000328f4:     	and	x10, x8, #0xfffc
1000328f8:     	mov	x11, x10
1000328fc:     	mov	x12, x9
<L31>:
100032900:     	strb	wzr, [x12, #0x84]
100032904:     	strb	wzr, [x12, #0x10c]
100032908:     	strb	wzr, [x12, #0x194]
10003290c:     	strb	wzr, [x12, #0x21c]
100032910:     	add	x12, x12, #0x220
100032914:     	subs	x11, x11, #0x4
100032918:     	b.ne	 <L31>
10003291c:     	cmp	x10, x8
100032920:     	b.eq	 <L34>
<L32>:
100032924:     	sub	x11, x8, x10
100032928:     	mov	w12, #0x88              ; =136
10003292c:     	umaddl	x9, w10, w12, x9
100032930:     	add	x9, x9, #0x84
<L33>:
100032934:     	strb	wzr, [x9], #0x88
100032938:     	subs	x11, x11, #0x1
10003293c:     	b.ne	 <L33>
<L34>:
100032940:     	ldr	x9, [x22, #0x50]
100032944:     	ldr	x10, [x17, #0x20]
100032948:     	sub	x11, x8, #0x1
10003294c:     	cmp	x11, #0x7
100032950:     	b.hs	 <L35>
100032954:     	mov	x11, #0x0               ; =0
100032958:     	and	x8, x8, #0x7
10003295c:     	cbnz	x8,  <L46>
100032960:     	b	 <L3>
<L35>:
100032964:     	mov	x11, #0x0               ; =0
100032968:     	add	x12, x10, #0x3cc
10003296c:     	and	x13, x8, #0xfff8
100032970:     	neg	x13, x13
100032974:     	b	 <L37>
<L36>:
100032978:     	add	x12, x12, #0x440
10003297c:     	sub	x11, x11, #0x8
100032980:     	cmp	x13, x11
100032984:     	b.eq	 <L45>
<L37>:
100032988:     	sub	x14, x12, #0x3b8
10003298c:     	ldr	w14, [x14]
100032990:     	cmp	w14, #0x1
100032994:     	b.ne	 <L38>
100032998:     	sub	x14, x12, #0x3c4
10003299c:     	str	x9, [x14]
1000329a0:     	sub	x9, x12, #0x3cc
1000329a4:     	str	x9, [x22, #0x50]
<L38>:
1000329a8:     	sub	x14, x12, #0x330
1000329ac:     	ldr	w14, [x14]
1000329b0:     	cmp	w14, #0x1
1000329b4:     	b.ne	 <L39>
1000329b8:     	sub	x14, x12, #0x33c
1000329bc:     	str	x9, [x14]
1000329c0:     	sub	x9, x12, #0x344
1000329c4:     	str	x9, [x22, #0x50]
<L39>:
1000329c8:     	sub	x14, x12, #0x2a8
1000329cc:     	ldr	w14, [x14]
1000329d0:     	cmp	w14, #0x1
1000329d4:     	b.ne	 <L40>
1000329d8:     	sub	x14, x12, #0x2b4
1000329dc:     	str	x9, [x14]
1000329e0:     	sub	x9, x12, #0x2bc
1000329e4:     	str	x9, [x22, #0x50]
<L40>:
1000329e8:     	sub	x14, x12, #0x220
1000329ec:     	ldr	w14, [x14]
1000329f0:     	cmp	w14, #0x1
1000329f4:     	b.ne	 <L41>
1000329f8:     	sub	x14, x12, #0x22c
1000329fc:     	str	x9, [x14]
100032a00:     	sub	x9, x12, #0x234
100032a04:     	str	x9, [x22, #0x50]
<L41>:
100032a08:     	sub	x14, x12, #0x198
100032a0c:     	ldr	w14, [x14]
100032a10:     	cmp	w14, #0x1
100032a14:     	b.ne	 <L42>
100032a18:     	sub	x14, x12, #0x1a4
100032a1c:     	str	x9, [x14]
100032a20:     	sub	x9, x12, #0x1ac
100032a24:     	str	x9, [x22, #0x50]
<L42>:
100032a28:     	sub	x14, x12, #0x110
100032a2c:     	ldr	w14, [x14]
100032a30:     	cmp	w14, #0x1
100032a34:     	b.ne	 <L43>
100032a38:     	sub	x14, x12, #0x11c
100032a3c:     	str	x9, [x14]
100032a40:     	sub	x9, x12, #0x124
100032a44:     	str	x9, [x22, #0x50]
<L43>:
100032a48:     	ldur	w14, [x12, #-0x88]
100032a4c:     	cmp	w14, #0x1
100032a50:     	b.ne	 <L44>
100032a54:     	stur	x9, [x12, #-0x94]
100032a58:     	sub	x9, x12, #0x9c
100032a5c:     	str	x9, [x22, #0x50]
<L44>:
100032a60:     	ldr	w14, [x12]
100032a64:     	cmp	w14, #0x1
100032a68:     	b.ne	 <L36>
100032a6c:     	stur	x9, [x12, #-0xc]
100032a70:     	sub	x9, x12, #0x14
100032a74:     	str	x9, [x22, #0x50]
100032a78:     	b	 <L36>
<L45>:
100032a7c:     	neg	x11, x11
100032a80:     	and	x8, x8, #0x7
100032a84:     	cbz	x8,  <L3>
<L46>:
100032a88:     	mov	w12, #0x88              ; =136
100032a8c:     	madd	x10, x11, x12, x10
100032a90:     	b	 <L48>
<L47>:
100032a94:     	add	x10, x10, #0x88
100032a98:     	subs	x8, x8, #0x1
100032a9c:     	b.eq	 <L3>
<L48>:
100032aa0:     	ldr	w11, [x10, #0x14]
100032aa4:     	cmp	w11, #0x1
100032aa8:     	b.ne	 <L47>
100032aac:     	str	x9, [x10, #0x8]
100032ab0:     	str	x10, [x22, #0x50]
100032ab4:     	mov	x9, x10
100032ab8:     	b	 <L47>
<L49>:
100032abc:     	mov	w16, #0x0               ; =0
100032ac0:     	mov	x19, #0x0               ; =0
<L50>:
100032ac4:     	cbz	w21,  <L51>
100032ac8:     	str	x19, [x22, #0x48]
<L51>:
100032acc:     	tbz	w16, #0x0,  <L52>
100032ad0:     	bl	 <_scoop_heap_page_map_prune>
<L52>:
100032ad4:     	ldp	x29, x30, [sp, #0xa0]
100032ad8:     	ldp	x20, x19, [sp, #0x90]
100032adc:     	ldp	x22, x21, [sp, #0x80]
100032ae0:     	ldp	x24, x23, [sp, #0x70]
100032ae4:     	ldp	x26, x25, [sp, #0x60]
100032ae8:     	ldp	x28, x27, [sp, #0x50]
100032aec:     	add	sp, sp, #0xb0
100032af0:     	ret
<L53>:
100032af4:     	adrp	x0, 0x100062000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x700>
100032af8:     	add	x0, x0, #0x50f
100032afc:     	bl	 <_scoop_heap_fatal>

0000000100034cb4 <_scoop_heap_select_evacuation_sources>:
100034cb4:     	stp	x28, x27, [sp, #-0x60]!
100034cb8:     	stp	x26, x25, [sp, #0x10]
100034cbc:     	stp	x24, x23, [sp, #0x20]
100034cc0:     	stp	x22, x21, [sp, #0x30]
100034cc4:     	stp	x20, x19, [sp, #0x40]
100034cc8:     	stp	x29, x30, [sp, #0x50]
100034ccc:     	add	x29, sp, #0x50
100034cd0:     	mov	x19, x0
100034cd4:     	mov	x20, #0x0               ; =0
100034cd8:     	adrp	x22, 0x10015c000 <dyld_stub_binder+0x10015c000>
100034cdc:     	add	x22, x22, #0x418
100034ce0:     	tbnz	w0, #0x0,  <L11>
100034ce4:     	ldrb	w8, [x22, #0x99]
100034ce8:     	tbnz	w8, #0x0,  <L11>
100034cec:     	ldr	x24, [x22, #0x40]
100034cf0:     	cbz	x24,  <L11>
100034cf4:     	mov	x23, #0x0               ; =0
100034cf8:     	mov	x20, #0x0               ; =0
100034cfc:     	mov	x25, #0x0               ; =0
100034d00:     	mov	w26, #0x88              ; =136
100034d04:     	b	 <L3>
<L0>:
100034d08:     	eor	w8, w28, #0x1
<L1>:
100034d0c:     	cmp	x27, x25
100034d10:     	csel	x25, x27, x25, hi
100034d14:     	csel	x23, x24, x23, hi
100034d18:     	sub	x9, x27, #0x1
100034d1c:     	cmp	x9, #0x400, lsl #12     ; =0x400000
100034d20:     	cset	w9, lo
100034d24:     	and	w8, w9, w8
100034d28:     	strb	w8, [x24, #0x2b]
100034d2c:     	str	x27, [x24, #0x30]
100034d30:     	add	x20, x20, x8
<L2>:
100034d34:     	ldr	x24, [x24, #0x18]
100034d38:     	cbz	x24,  <L10>
<L3>:
100034d3c:     	ldrb	w8, [x24, #0x2a]
100034d40:     	tbnz	w8, #0x0,  <L2>
100034d44:     	ldrh	w8, [x24, #0x28]
100034d48:     	cbz	w8,  <L9>
100034d4c:     	mov	x22, #0x0               ; =0
100034d50:     	mov	w28, #0x0               ; =0
100034d54:     	mov	x27, #0x0               ; =0
100034d58:     	b	 <L7>
<L4>:
100034d5c:     	ldrb	w8, [x21, #0x82]
<L5>:
100034d60:     	ldr	x9, [x21, #0x68]
100034d64:     	add	x27, x9, x27
100034d68:     	orr	w28, w28, w8
<L6>:
100034d6c:     	add	x22, x22, #0x1
100034d70:     	ldrh	w8, [x24, #0x28]
100034d74:     	cmp	x22, x8
100034d78:     	b.hs	 <L0>
<L7>:
100034d7c:     	ldr	x8, [x24, #0x20]
100034d80:     	madd	x21, x22, x26, x8
100034d84:     	mov	x0, x21
100034d88:     	bl	 <_scoop_heap_active_head>
100034d8c:     	cbz	w0,  <L6>
100034d90:     	ldr	w8, [x21, #0x18]
100034d94:     	cmp	w8, #0x2
100034d98:     	b.eq	 <L4>
100034d9c:     	mov	x10, #0x0               ; =0
100034da0:     	ldr	x9, [x21, #0x30]
<L8>:
100034da4:     	ldr	x11, [x9, x10]
100034da8:     	cmp	x11, #0x0
100034dac:     	cset	w8, ne
100034db0:     	cbnz	x11,  <L5>
100034db4:     	cmp	x10, #0x1f8
100034db8:     	add	x10, x10, #0x8
100034dbc:     	b.ne	 <L8>
100034dc0:     	b	 <L5>
<L9>:
100034dc4:     	mov	x27, #0x0               ; =0
100034dc8:     	mov	w8, #0x1                ; =1
100034dcc:     	b	 <L1>
<L10>:
100034dd0:     	cbz	x23,  <L20>
100034dd4:     	ldrb	w8, [x23, #0x2b]
100034dd8:     	cmp	w8, #0x1
100034ddc:     	adrp	x22, 0x10015c000 <dyld_stub_binder+0x10015c000>
100034de0:     	add	x22, x22, #0x418
100034de4:     	b.ne	 <L11>
100034de8:     	strb	wzr, [x23, #0x2b]
100034dec:     	sub	x20, x20, #0x1
<L11>:
100034df0:     	bl	 <_scoop_heap_first_block>
100034df4:     	cbz	x0,  <L21>
<L12>:
100034df8:     	mov	x21, x0
100034dfc:     	mov	w23, #0x3               ; =3
100034e00:     	b	 <L15>
<L13>:
100034e04:     	ldr	w8, [x21, #0x1c]
100034e08:     	cbz	w8,  <L16>
<L14>:
100034e0c:     	mov	x0, x21
100034e10:     	bl	 <_scoop_heap_next_block>
100034e14:     	mov	x21, x0
100034e18:     	cbz	x0,  <L21>
<L15>:
100034e1c:     	mov	x0, x21
100034e20:     	bl	 <_scoop_heap_active_head>
100034e24:     	cbz	w0,  <L14>
100034e28:     	ldr	x8, [x21, #0x70]
100034e2c:     	cbz	x8,  <L14>
100034e30:     	cbnz	w19,  <L13>
100034e34:     	ldrb	w8, [x22, #0x99]
100034e38:     	tbnz	w8, #0x0,  <L19>
100034e3c:     	ldr	x8, [x21]
100034e40:     	ldrb	w8, [x8, #0x2b]
100034e44:     	cbnz	w8,  <L19>
100034e48:     	b	 <L14>
<L16>:
100034e4c:     	ldr	w8, [x21, #0x18]
100034e50:     	cmp	w8, #0x2
100034e54:     	b.ne	 <L17>
100034e58:     	ldrb	w8, [x21, #0x82]
100034e5c:     	eor	w8, w8, #0x1
100034e60:     	cbnz	w8,  <L19>
100034e64:     	b	 <L14>
<L17>:
100034e68:     	mov	x8, #0x0                ; =0
100034e6c:     	ldr	x9, [x21, #0x30]
<L18>:
100034e70:     	ldr	x10, [x9, x8]
100034e74:     	cbnz	x10,  <L14>
100034e78:     	add	x8, x8, #0x8
100034e7c:     	cmp	x8, #0x200
100034e80:     	b.ne	 <L18>
<L19>:
100034e84:     	str	w23, [x21, #0x14]
100034e88:     	ldr	w8, [x21, #0x18]
100034e8c:     	cmp	w8, #0x1
100034e90:     	b.ne	 <L14>
100034e94:     	mov	w0, #0x1000             ; =4096
100034e98:     	mov	w1, #0x8                ; =8
100034e9c:     	bl	 <dyld_stub_binder+0x1000481a4>
100034ea0:     	str	x0, [x21, #0x58]
100034ea4:     	cbnz	x0,  <L14>
100034ea8:     	adrp	x0, 0x100062000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x700>
100034eac:     	add	x0, x0, #0xace
100034eb0:     	bl	 <_scoop_heap_fatal>
<L20>:
100034eb4:     	adrp	x22, 0x10015c000 <dyld_stub_binder+0x10015c000>
100034eb8:     	add	x22, x22, #0x418
100034ebc:     	bl	 <_scoop_heap_first_block>
100034ec0:     	cbnz	x0,  <L12>
<L21>:
100034ec4:     	mov	x0, x20
100034ec8:     	ldp	x29, x30, [sp, #0x50]
100034ecc:     	ldp	x20, x19, [sp, #0x40]
100034ed0:     	ldp	x22, x21, [sp, #0x30]
100034ed4:     	ldp	x24, x23, [sp, #0x20]
100034ed8:     	ldp	x26, x25, [sp, #0x10]
100034edc:     	ldp	x28, x27, [sp], #0x60
100034ee0:     	ret

000000010003a0ac <_scoop_heap_prepare_evacuation_targets>:
10003a0ac:     	stp	x26, x25, [sp, #-0x50]!
10003a0b0:     	stp	x24, x23, [sp, #0x10]
10003a0b4:     	stp	x22, x21, [sp, #0x20]
10003a0b8:     	stp	x20, x19, [sp, #0x30]
10003a0bc:     	stp	x29, x30, [sp, #0x40]
10003a0c0:     	add	x29, sp, #0x40
10003a0c4:     	bl	 <_scoop_heap_first_block>
10003a0c8:     	cbz	x0,  <L15>
10003a0cc:     	mov	x19, x0
10003a0d0:     	mov	w22, #0x1               ; =1
10003a0d4:     	mov	w23, #0x4               ; =4
10003a0d8:     	adrp	x24, 0x10015c000 <dyld_stub_binder+0x10015c000>
10003a0dc:     	add	x24, x24, #0x418
10003a0e0:     	b	 <L2>
<L0>:
10003a0e4:     	mov	x0, x19
10003a0e8:     	bl	 <_scoop_heap_release_block>
<L1>:
10003a0ec:     	mov	x0, x19
10003a0f0:     	bl	 <_scoop_heap_next_block>
10003a0f4:     	mov	x19, x0
10003a0f8:     	cbz	x0,  <L15>
<L2>:
10003a0fc:     	mov	x0, x19
10003a100:     	bl	 <_scoop_heap_active_head>
10003a104:     	cbz	w0,  <L1>
10003a108:     	ldr	w8, [x19, #0x18]
10003a10c:     	cmp	w8, #0x1
10003a110:     	b.ne	 <L1>
10003a114:     	ldr	x8, [x19]
10003a118:     	ldrb	w8, [x8, #0x2b]
10003a11c:     	tbnz	w8, #0x0,  <L1>
10003a120:     	ldr	w8, [x19, #0x1c]
10003a124:     	cbz	w8,  <L14>
<L3>:
10003a128:     	mov	x20, #0x0               ; =0
10003a12c:     	mov	w21, #0x80              ; =128
10003a130:     	b	 <L7>
<L4>:
10003a134:     	add	x8, x9, #0x88
10003a138:     	ldapr	x8, [x8]
10003a13c:     	tbnz	w8, #0x2,  <L19>
<L5>:
10003a140:     	ldr	x0, [x19, #0x20]
10003a144:     	add	x1, x20, #0x10
10003a148:     	bl	 <_scoop_heap_bit_clear>
10003a14c:     	ldr	x0, [x19, #0x30]
10003a150:     	add	x1, x20, #0x10
10003a154:     	bl	 <_scoop_heap_bit_clear>
10003a158:     	ldr	x8, [x19, #0x50]
10003a15c:     	add	x8, x8, x20, lsl #1
10003a160:     	strh	wzr, [x8, #0x20]
10003a164:     	strb	w22, [x19, #0x84]
<L6>:
10003a168:     	add	x20, x20, #0x1
10003a16c:     	add	x21, x21, #0x8
10003a170:     	cmp	x20, #0xff0
10003a174:     	b.eq	 <L8>
<L7>:
10003a178:     	ldr	x0, [x19, #0x20]
10003a17c:     	add	x1, x20, #0x10
10003a180:     	bl	 <_scoop_heap_bit_test>
10003a184:     	cbz	w0,  <L6>
10003a188:     	ldr	x0, [x19, #0x28]
10003a18c:     	add	x1, x20, #0x10
10003a190:     	bl	 <_scoop_heap_bit_test>
10003a194:     	tbnz	w0, #0x0,  <L6>
10003a198:     	ldr	x8, [x19, #0x50]
10003a19c:     	add	x8, x8, x20, lsl #1
10003a1a0:     	ldrh	w8, [x8, #0x20]
10003a1a4:     	cbz	w8,  <L16>
10003a1a8:     	mov	x0, x19
10003a1ac:     	bl	 <_scoop_heap_block_base>
10003a1b0:     	add	x9, x0, x20, lsl #3
10003a1b4:     	ldr	x8, [x9, #0x80]
10003a1b8:     	ldr	x8, [x8, #0x90]
10003a1bc:     	cbz	x8,  <L4>
10003a1c0:     	add	x9, x9, #0x88
10003a1c4:     	ldclral	x23, x9, [x9]
10003a1c8:     	tbz	w9, #0x2,  <L5>
10003a1cc:     	add	x0, x0, x21
10003a1d0:     	blr	x8
10003a1d4:     	b	 <L5>
<L8>:
10003a1d8:     	ldr	x8, [x19, #0x68]
10003a1dc:     	cbz	x8,  <L0>
10003a1e0:     	mov	x25, #0x0               ; =0
10003a1e4:     	ldr	x20, [x19, #0x48]
10003a1e8:     	mov	w21, #0x1               ; =1
10003a1ec:     	b	 <L11>
<L9>:
10003a1f0:     	mov	x1, x25
<L10>:
10003a1f4:     	add	x21, x21, #0x1
10003a1f8:     	mov	x25, x1
10003a1fc:     	cmp	x21, #0x101
10003a200:     	b.eq	 <L1>
<L11>:
10003a204:     	cmp	x21, #0x100
10003a208:     	b.ne	 <L13>
10003a20c:     	cmp	x25, #0x0
10003a210:     	cset	w8, ne
10003a214:     	mov	w0, #0x1                ; =1
<L12>:
10003a218:     	cbz	w0,  <L9>
10003a21c:     	cbz	w8,  <L9>
10003a220:     	subs	x26, x21, x25
10003a224:     	b.eq	 <L18>
10003a228:     	mov	w0, #0x18               ; =24
10003a22c:     	bl	 <dyld_stub_binder+0x10004824c>
10003a230:     	cbz	x0,  <L17>
10003a234:     	mov	x1, #0x0                ; =0
10003a238:     	ldr	x8, [x24, #0x58]
10003a23c:     	stp	x8, x19, [x0]
10003a240:     	strh	w25, [x0, #0x10]
10003a244:     	strh	w26, [x0, #0x12]
10003a248:     	str	wzr, [x0, #0x14]
10003a24c:     	str	x0, [x24, #0x58]
10003a250:     	b	 <L10>
<L13>:
10003a254:     	mov	x0, x20
10003a258:     	mov	x1, x21
10003a25c:     	bl	 <_scoop_heap_bit_test>
10003a260:     	cmp	x25, #0x0
10003a264:     	cset	w8, ne
10003a268:     	tbnz	w0, #0x0,  <L12>
10003a26c:     	mov	x1, x21
10003a270:     	cbz	x25,  <L10>
10003a274:     	b	 <L12>
<L14>:
10003a278:     	ldr	x8, [x19, #0x68]
10003a27c:     	cbnz	x8,  <L1>
10003a280:     	b	 <L3>
<L15>:
10003a284:     	ldp	x29, x30, [sp, #0x40]
10003a288:     	ldp	x20, x19, [sp, #0x30]
10003a28c:     	ldp	x22, x21, [sp, #0x20]
10003a290:     	ldp	x24, x23, [sp, #0x10]
10003a294:     	ldp	x26, x25, [sp], #0x50
10003a298:     	ret
<L16>:
10003a29c:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a2a0:     	add	x0, x0, #0xedf
10003a2a4:     	bl	 <_scoop_heap_fatal>
<L17>:
10003a2a8:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a2ac:     	add	x0, x0, #0xf43
10003a2b0:     	bl	 <_scoop_heap_fatal>
<L18>:
10003a2b4:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a2b8:     	add	x0, x0, #0xf2d
10003a2bc:     	bl	 <_scoop_heap_fatal>
<L19>:
10003a2c0:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a2c4:     	add	x0, x0, #0xf04
10003a2c8:     	bl	 <_scoop_heap_fatal>

000000010003a2cc <_scoop_heap_finish_block>:
10003a2cc:     	stp	x28, x27, [sp, #-0x60]!
10003a2d0:     	stp	x26, x25, [sp, #0x10]
10003a2d4:     	stp	x24, x23, [sp, #0x20]
10003a2d8:     	stp	x22, x21, [sp, #0x30]
10003a2dc:     	stp	x20, x19, [sp, #0x40]
10003a2e0:     	stp	x29, x30, [sp, #0x50]
10003a2e4:     	add	x29, sp, #0x50
10003a2e8:     	mov	x20, x1
10003a2ec:     	mov	x19, x0
10003a2f0:     	ldr	w8, [x0, #0x18]
10003a2f4:     	cmp	w8, #0x2
10003a2f8:     	b.eq	 <L9>
10003a2fc:     	cmp	w8, #0x1
10003a300:     	b.ne	 <L31>
10003a304:     	mov	w25, #0x0               ; =0
10003a308:     	mov	w24, #0x0               ; =0
10003a30c:     	ldr	x8, [x19, #0x40]
10003a310:     	movi.2d	v0, #0000000000000000
10003a314:     	stp	q0, q0, [x8]
10003a318:     	mov	w21, #0x10              ; =16
10003a31c:     	mov	w26, #0x1               ; =1
10003a320:     	mov	w27, #0x4               ; =4
10003a324:     	b	 <L2>
<L0>:
10003a328:     	orr	w24, w24, w22
10003a32c:     	mov	w25, #0x1               ; =1
<L1>:
10003a330:     	add	x21, x21, #0x1
10003a334:     	cmp	x21, #0x1, lsl #12      ; =0x1000
10003a338:     	b.eq	 <L11>
<L2>:
10003a33c:     	ldr	x0, [x19, #0x20]
10003a340:     	mov	x1, x21
10003a344:     	bl	 <_scoop_heap_bit_test>
10003a348:     	cbz	w0,  <L1>
10003a34c:     	ldr	x0, [x19, #0x30]
10003a350:     	mov	x1, x21
10003a354:     	bl	 <_scoop_heap_bit_test>
10003a358:     	mov	x22, x0
10003a35c:     	ldr	x0, [x19, #0x28]
10003a360:     	mov	x1, x21
10003a364:     	bl	 <_scoop_heap_bit_test>
10003a368:     	mov	x23, x0
10003a36c:     	ldr	w8, [x19, #0x14]
10003a370:     	cmp	w8, #0x3
10003a374:     	cset	w8, eq
10003a378:     	eor	w9, w22, #0x1
10003a37c:     	and	w8, w8, w9
10003a380:     	cmp	w8, #0x1
10003a384:     	ccmp	w0, #0x0, #0x4, eq
10003a388:     	b.eq	 <L3>
10003a38c:     	ldr	x9, [x19, #0x58]
10003a390:     	ldr	x9, [x9, x21, lsl #3]
10003a394:     	cbz	x9,  <L25>
<L3>:
10003a398:     	eor	w8, w8, #0x1
10003a39c:     	and	w8, w23, w8
10003a3a0:     	ldr	x9, [x19, #0x50]
10003a3a4:     	ldrh	w28, [x9, x21, lsl #1]
10003a3a8:     	tbz	w8, #0x0,  <L5>
10003a3ac:     	cbz	w28,  <L27>
10003a3b0:     	add	x8, x21, x28
10003a3b4:     	lsl	x8, x8, #3
10003a3b8:     	sub	x8, x8, #0x1
10003a3bc:     	lsr	x23, x21, #4
10003a3c0:     	cmp	x23, x8, lsr #7
10003a3c4:     	b.hi	 <L0>
10003a3c8:     	lsr	x8, x8, #7
10003a3cc:     	add	x25, x8, #0x1
<L4>:
10003a3d0:     	ldr	x0, [x19, #0x40]
10003a3d4:     	mov	x1, x23
10003a3d8:     	bl	 <_scoop_heap_bit_set>
10003a3dc:     	add	x23, x23, #0x1
10003a3e0:     	cmp	x25, x23
10003a3e4:     	b.ne	 <L4>
10003a3e8:     	b	 <L0>
<L5>:
10003a3ec:     	cbz	w28,  <L26>
10003a3f0:     	mov	x0, x19
10003a3f4:     	bl	 <_scoop_heap_block_base>
10003a3f8:     	add	x0, x0, x21, lsl #3
10003a3fc:     	tbnz	w23, #0x0,  <L7>
10003a400:     	ldr	x8, [x0]
10003a404:     	ldr	x8, [x8, #0x90]
10003a408:     	cbz	x8,  <L6>
10003a40c:     	add	x9, x0, #0x8
10003a410:     	ldclral	x27, x9, [x9]
10003a414:     	tbz	w9, #0x2,  <L7>
10003a418:     	mov	x22, x0
10003a41c:     	blr	x8
10003a420:     	mov	x0, x22
10003a424:     	b	 <L7>
<L6>:
10003a428:     	add	x8, x0, #0x8
10003a42c:     	ldapr	x8, [x8]
10003a430:     	tbnz	w8, #0x2,  <L30>
<L7>:
10003a434:     	cbz	w20,  <L8>
10003a438:     	lsl	x2, x28, #3
10003a43c:     	mov	w1, #0xa5               ; =165
10003a440:     	bl	 <dyld_stub_binder+0x10004827c>
<L8>:
10003a444:     	ldr	x0, [x19, #0x20]
10003a448:     	mov	x1, x21
10003a44c:     	bl	 <_scoop_heap_bit_clear>
10003a450:     	ldr	x0, [x19, #0x30]
10003a454:     	mov	x1, x21
10003a458:     	bl	 <_scoop_heap_bit_clear>
10003a45c:     	ldr	x8, [x19, #0x50]
10003a460:     	strh	wzr, [x8, x21, lsl #1]
10003a464:     	strb	w26, [x19, #0x84]
10003a468:     	b	 <L1>
<L9>:
10003a46c:     	ldr	w8, [x19, #0x14]
10003a470:     	cmp	w8, #0x3
10003a474:     	b.ne	 <L10>
10003a478:     	ldrb	w8, [x19, #0x82]
10003a47c:     	tbz	w8, #0x0,  <L18>
<L10>:
10003a480:     	ldrb	w8, [x19, #0x81]
10003a484:     	tbz	w8, #0x0,  <L19>
10003a488:     	ldrb	w8, [x19, #0x82]
10003a48c:     	mov	w9, #0x2                ; =2
10003a490:     	mov	w10, #0x5               ; =5
10003a494:     	cmp	w8, #0x0
10003a498:     	csel	w8, w10, w9, ne
10003a49c:     	str	w8, [x19, #0x14]
10003a4a0:     	stp	xzr, xzr, [x19, #0x68]
10003a4a4:     	str	xzr, [x19, #0x78]
10003a4a8:     	strb	wzr, [x19, #0x81]
10003a4ac:     	strb	wzr, [x19, #0x83]
10003a4b0:     	b	 <L24>
<L11>:
10003a4b4:     	ldr	x8, [x19, #0x28]
10003a4b8:     	movi.2d	v0, #0000000000000000
10003a4bc:     	stp	q0, q0, [x8, #0x1e0]
10003a4c0:     	stp	q0, q0, [x8, #0x1c0]
10003a4c4:     	stp	q0, q0, [x8, #0x1a0]
10003a4c8:     	stp	q0, q0, [x8, #0x180]
10003a4cc:     	stp	q0, q0, [x8, #0x160]
10003a4d0:     	stp	q0, q0, [x8, #0x140]
10003a4d4:     	stp	q0, q0, [x8, #0x120]
10003a4d8:     	stp	q0, q0, [x8, #0x100]
10003a4dc:     	stp	q0, q0, [x8, #0xe0]
10003a4e0:     	stp	q0, q0, [x8, #0xc0]
10003a4e4:     	stp	q0, q0, [x8, #0xa0]
10003a4e8:     	stp	q0, q0, [x8, #0x80]
10003a4ec:     	stp	q0, q0, [x8, #0x60]
10003a4f0:     	stp	q0, q0, [x8, #0x40]
10003a4f4:     	stp	q0, q0, [x8, #0x20]
10003a4f8:     	stp	q0, q0, [x8]
10003a4fc:     	ldr	x8, [x19, #0x38]
10003a500:     	stp	q0, q0, [x8, #0x1e0]
10003a504:     	stp	q0, q0, [x8, #0x1c0]
10003a508:     	stp	q0, q0, [x8, #0x1a0]
10003a50c:     	stp	q0, q0, [x8, #0x180]
10003a510:     	stp	q0, q0, [x8, #0x160]
10003a514:     	stp	q0, q0, [x8, #0x140]
10003a518:     	stp	q0, q0, [x8, #0x120]
10003a51c:     	stp	q0, q0, [x8, #0x100]
10003a520:     	stp	q0, q0, [x8, #0xe0]
10003a524:     	stp	q0, q0, [x8, #0xc0]
10003a528:     	stp	q0, q0, [x8, #0xa0]
10003a52c:     	stp	q0, q0, [x8, #0x80]
10003a530:     	stp	q0, q0, [x8, #0x60]
10003a534:     	stp	q0, q0, [x8, #0x40]
10003a538:     	stp	q0, q0, [x8, #0x20]
10003a53c:     	stp	q0, q0, [x8]
10003a540:     	ldr	x8, [x19, #0x48]
10003a544:     	stp	q0, q0, [x8]
10003a548:     	ldr	x0, [x19, #0x58]
10003a54c:     	bl	 <dyld_stub_binder+0x1000481f8>
10003a550:     	str	xzr, [x19, #0x58]
10003a554:     	stp	xzr, xzr, [x19, #0x68]
10003a558:     	tbz	w25, #0x0,  <L17>
10003a55c:     	tst	w24, #0x1
10003a560:     	mov	w8, #0x2                ; =2
10003a564:     	mov	w9, #0x5                ; =5
10003a568:     	csel	w8, w9, w8, ne
10003a56c:     	str	w8, [x19, #0x14]
10003a570:     	tbnz	w20, #0x0,  <L24>
10003a574:     	mov	x23, #0x0               ; =0
10003a578:     	ldr	x20, [x19, #0x40]
10003a57c:     	mov	w21, #0x1               ; =1
10003a580:     	adrp	x22, 0x10015c000 <dyld_stub_binder+0x10015c000>
10003a584:     	add	x22, x22, #0x418
10003a588:     	b	 <L14>
<L12>:
10003a58c:     	mov	x1, x23
<L13>:
10003a590:     	add	x21, x21, #0x1
10003a594:     	mov	x23, x1
10003a598:     	cmp	x21, #0x101
10003a59c:     	b.eq	 <L24>
<L14>:
10003a5a0:     	cmp	x21, #0x100
10003a5a4:     	b.ne	 <L16>
10003a5a8:     	cmp	x23, #0x0
10003a5ac:     	cset	w8, ne
10003a5b0:     	mov	w0, #0x1                ; =1
<L15>:
10003a5b4:     	cbz	w0,  <L12>
10003a5b8:     	cbz	w8,  <L12>
10003a5bc:     	subs	x24, x21, x23
10003a5c0:     	b.eq	 <L28>
10003a5c4:     	mov	w0, #0x18               ; =24
10003a5c8:     	bl	 <dyld_stub_binder+0x10004824c>
10003a5cc:     	cbz	x0,  <L29>
10003a5d0:     	mov	x1, #0x0                ; =0
10003a5d4:     	ldr	x8, [x22, #0x58]
10003a5d8:     	stp	x8, x19, [x0]
10003a5dc:     	strh	w23, [x0, #0x10]
10003a5e0:     	strh	w24, [x0, #0x12]
10003a5e4:     	str	wzr, [x0, #0x14]
10003a5e8:     	str	x0, [x22, #0x58]
10003a5ec:     	b	 <L13>
<L16>:
10003a5f0:     	mov	x0, x20
10003a5f4:     	mov	x1, x21
10003a5f8:     	bl	 <_scoop_heap_bit_test>
10003a5fc:     	cmp	x23, #0x0
10003a600:     	cset	w8, ne
10003a604:     	tbnz	w0, #0x0,  <L15>
10003a608:     	mov	x1, x21
10003a60c:     	cbz	x23,  <L13>
10003a610:     	b	 <L15>
<L17>:
10003a614:     	cbnz	w20,  <L22>
10003a618:     	b	 <L23>
<L18>:
10003a61c:     	ldrb	w8, [x19, #0x81]
10003a620:     	cmp	w8, #0x1
10003a624:     	b.ne	 <L19>
10003a628:     	ldr	x8, [x19, #0x78]
10003a62c:     	cbnz	x8,  <L21>
10003a630:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a634:     	add	x0, x0, #0xfba
10003a638:     	bl	 <_scoop_heap_fatal>
<L19>:
10003a63c:     	mov	x0, x19
10003a640:     	bl	 <_scoop_heap_block_base>
10003a644:     	mov	x8, x0
10003a648:     	ldr	x9, [x0, #0x80]!
10003a64c:     	ldr	x9, [x9, #0x90]
10003a650:     	cbz	x9,  <L20>
10003a654:     	add	x8, x8, #0x88
10003a658:     	mov	w10, #0x4               ; =4
10003a65c:     	ldclral	x10, x8, [x8]
10003a660:     	tbz	w8, #0x2,  <L21>
10003a664:     	blr	x9
10003a668:     	b	 <L21>
<L20>:
10003a66c:     	add	x8, x8, #0x88
10003a670:     	ldapr	x8, [x8]
10003a674:     	tbnz	w8, #0x2,  <L30>
<L21>:
10003a678:     	cbz	w20,  <L23>
10003a67c:     	mov	x0, x19
10003a680:     	bl	 <_scoop_heap_block_base>
10003a684:     	ldr	x2, [x19, #0x60]
10003a688:     	add	x0, x0, #0x80
10003a68c:     	mov	w1, #0xa5               ; =165
10003a690:     	bl	 <dyld_stub_binder+0x10004827c>
<L22>:
10003a694:     	mov	x0, x19
10003a698:     	bl	 <_scoop_heap_quarantine_block>
10003a69c:     	b	 <L24>
<L23>:
10003a6a0:     	mov	x0, x19
10003a6a4:     	bl	 <_scoop_heap_release_block>
<L24>:
10003a6a8:     	ldp	x29, x30, [sp, #0x50]
10003a6ac:     	ldp	x20, x19, [sp, #0x40]
10003a6b0:     	ldp	x22, x21, [sp, #0x30]
10003a6b4:     	ldp	x24, x23, [sp, #0x20]
10003a6b8:     	ldp	x26, x25, [sp, #0x10]
10003a6bc:     	ldp	x28, x27, [sp], #0x60
10003a6c0:     	ret
<L25>:
10003a6c4:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a6c8:     	add	x0, x0, #0xf6b
10003a6cc:     	bl	 <_scoop_heap_fatal>
<L26>:
10003a6d0:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a6d4:     	add	x0, x0, #0xedf
10003a6d8:     	bl	 <_scoop_heap_fatal>
<L27>:
10003a6dc:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a6e0:     	add	x0, x0, #0xf9a
10003a6e4:     	bl	 <_scoop_heap_fatal>
<L28>:
10003a6e8:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a6ec:     	add	x0, x0, #0xf2d
10003a6f0:     	bl	 <_scoop_heap_fatal>
<L29>:
10003a6f4:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a6f8:     	add	x0, x0, #0xf43
10003a6fc:     	bl	 <_scoop_heap_fatal>
<L30>:
10003a700:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a704:     	add	x0, x0, #0xf04
10003a708:     	bl	 <_scoop_heap_fatal>
<L31>:
10003a70c:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1700>
10003a710:     	add	x0, x0, #0xebf
10003a714:     	bl	 <_scoop_heap_fatal>

000000010004269c <_scoop_gc_heap_plan_moving_locked>:
10004269c:     	sub	sp, sp, #0x70
1000426a0:     	stp	x26, x25, [sp, #0x20]
1000426a4:     	stp	x24, x23, [sp, #0x30]
1000426a8:     	stp	x22, x21, [sp, #0x40]
1000426ac:     	stp	x20, x19, [sp, #0x50]
1000426b0:     	stp	x29, x30, [sp, #0x60]
1000426b4:     	add	x29, sp, #0x60
1000426b8:     	adrp	x23, 0x10015c000 <dyld_stub_binder+0x10015c000>
1000426bc:     	add	x23, x23, #0x418
1000426c0:     	ldrb	w8, [x23, #0x9d]
1000426c4:     	tbz	w8, #0x0,  <L44>
1000426c8:     	mov	x20, x0
1000426cc:     	bl	 <_scoop_heap_select_evacuation_sources>
1000426d0:     	mov	x19, x0
1000426d4:     	ldrb	w8, [x23, #0x99]
1000426d8:     	orr	w24, w20, w8
1000426dc:     	tbnz	w24, #0x0,  <L0>
1000426e0:     	cbz	x19,  <L35>
<L0>:
1000426e4:     	tbnz	w24, #0x0,  <L1>
1000426e8:     	bl	 <_scoop_heap_prepare_evacuation_targets>
<L1>:
1000426ec:     	cmp	x19, #0x1
1000426f0:     	stp	xzr, xzr, [sp, #0x10]
1000426f4:     	cset	w20, hi
1000426f8:     	str	xzr, [sp, #0x8]
1000426fc:     	bl	 <_scoop_heap_first_block>
100042700:     	cbz	x0,  <L10>
100042704:     	mov	x21, x0
100042708:     	eor	w25, w24, #0x1
10004270c:     	orr	w26, w24, w20
<L2>:
100042710:     	ldr	w8, [x21, #0x14]
100042714:     	cmp	w8, #0x3
100042718:     	b.ne	 <L3>
10004271c:     	ldr	w8, [x21, #0x18]
100042720:     	cmp	w8, #0x2
100042724:     	b.ne	 <L5>
100042728:     	add	x0, sp, #0x18
10004272c:     	add	x1, sp, #0x10
100042730:     	add	x2, sp, #0x8
100042734:     	and	w5, w25, #0x1
100042738:     	and	w6, w26, #0x1
10004273c:     	mov	x3, x21
100042740:     	mov	w4, #0x10               ; =16
100042744:     	bl	 <_append_move>
100042748:     	mov	x20, x0
10004274c:     	b	 <L4>
<L3>:
100042750:     	mov	w20, #0x1               ; =1
<L4>:
100042754:     	mov	x0, x21
100042758:     	bl	 <_scoop_heap_next_block>
10004275c:     	cbz	w20,  <L9>
100042760:     	mov	x21, x0
100042764:     	cbnz	x0,  <L2>
100042768:     	b	 <L9>
<L5>:
10004276c:     	mov	w22, #0x10              ; =16
100042770:     	b	 <L8>
<L6>:
100042774:     	mov	w20, #0x1               ; =1
100042778:     	cbz	w20,  <L4>
<L7>:
10004277c:     	cmp	x22, #0xfff
100042780:     	add	x22, x22, #0x1
100042784:     	b.hs	 <L4>
<L8>:
100042788:     	ldr	x0, [x21, #0x20]
10004278c:     	mov	x1, x22
100042790:     	bl	 <_scoop_heap_bit_test>
100042794:     	cbz	w0,  <L6>
100042798:     	ldr	x0, [x21, #0x28]
10004279c:     	mov	x1, x22
1000427a0:     	bl	 <_scoop_heap_bit_test>
1000427a4:     	cbz	w0,  <L6>
1000427a8:     	ldr	x0, [x21, #0x30]
1000427ac:     	mov	x1, x22
1000427b0:     	bl	 <_scoop_heap_bit_test>
1000427b4:     	tbnz	w0, #0x0,  <L6>
1000427b8:     	add	x0, sp, #0x18
1000427bc:     	add	x1, sp, #0x10
1000427c0:     	add	x2, sp, #0x8
1000427c4:     	and	w5, w25, #0x1
1000427c8:     	and	w6, w26, #0x1
1000427cc:     	mov	x3, x21
1000427d0:     	mov	x4, x22
1000427d4:     	bl	 <_append_move>
1000427d8:     	mov	x20, x0
1000427dc:     	cbnz	w20,  <L7>
1000427e0:     	b	 <L4>
<L9>:
1000427e4:     	tbz	w24, #0x0,  <L13>
1000427e8:     	cbnz	w20,  <L11>
1000427ec:     	b	 <L36>
<L10>:
1000427f0:     	tbz	w24, #0x0,  <L12>
<L11>:
1000427f4:     	ldr	x21, [sp, #0x10]
1000427f8:     	b	 <L30>
<L12>:
1000427fc:     	mov	w20, #0x1               ; =1
<L13>:
100042800:     	ldr	x21, [sp, #0x10]
100042804:     	cbz	x21,  <L14>
100042808:     	ldr	x10, [sp, #0x18]
10004280c:     	and	x8, x21, #0x7
100042810:     	cmp	x21, #0x8
100042814:     	b.hs	 <L15>
100042818:     	mov	x11, #0x0               ; =0
10004281c:     	mov	x9, #0x0                ; =0
100042820:     	cbnz	x8,  <L26>
100042824:     	b	 <L29>
<L14>:
100042828:     	mov	x9, #0x0                ; =0
10004282c:     	b	 <L29>
<L15>:
100042830:     	mov	x11, #0x0               ; =0
100042834:     	mov	x9, #0x0                ; =0
100042838:     	and	x12, x21, #0xfffffffffffffff8
10004283c:     	neg	x12, x12
100042840:     	add	x13, x10, #0xe8
100042844:     	mov	w14, #0x1               ; =1
100042848:     	b	 <L17>
<L16>:
10004284c:     	sub	x11, x11, #0x8
100042850:     	add	x13, x13, #0x180
100042854:     	cmp	x12, x11
100042858:     	b.eq	 <L25>
<L17>:
10004285c:     	ldur	x15, [x13, #-0xc0]
100042860:     	ldr	x15, [x15]
100042864:     	ldr	x16, [x15, #0x30]
100042868:     	cbnz	x16,  <L18>
10004286c:     	ldrb	w16, [x15, #0x2c]
100042870:     	tbnz	w16, #0x0,  <L18>
100042874:     	strb	w14, [x15, #0x2c]
100042878:     	add	x9, x9, #0x1
<L18>:
10004287c:     	ldur	x15, [x13, #-0x90]
100042880:     	ldr	x15, [x15]
100042884:     	ldr	x16, [x15, #0x30]
100042888:     	cbnz	x16,  <L19>
10004288c:     	ldrb	w16, [x15, #0x2c]
100042890:     	tbnz	w16, #0x0,  <L19>
100042894:     	strb	w14, [x15, #0x2c]
100042898:     	add	x9, x9, #0x1
<L19>:
10004289c:     	ldur	x15, [x13, #-0x60]
1000428a0:     	ldr	x15, [x15]
1000428a4:     	ldr	x16, [x15, #0x30]
1000428a8:     	cbnz	x16,  <L20>
1000428ac:     	ldrb	w16, [x15, #0x2c]
1000428b0:     	tbnz	w16, #0x0,  <L20>
1000428b4:     	strb	w14, [x15, #0x2c]
1000428b8:     	add	x9, x9, #0x1
<L20>:
1000428bc:     	ldur	x15, [x13, #-0x30]
1000428c0:     	ldr	x15, [x15]
1000428c4:     	ldr	x16, [x15, #0x30]
1000428c8:     	cbnz	x16,  <L21>
1000428cc:     	ldrb	w16, [x15, #0x2c]
1000428d0:     	tbnz	w16, #0x0,  <L21>
1000428d4:     	strb	w14, [x15, #0x2c]
1000428d8:     	add	x9, x9, #0x1
<L21>:
1000428dc:     	ldr	x15, [x13]
1000428e0:     	ldr	x15, [x15]
1000428e4:     	ldr	x16, [x15, #0x30]
1000428e8:     	cbnz	x16,  <L22>
1000428ec:     	ldrb	w16, [x15, #0x2c]
1000428f0:     	tbnz	w16, #0x0,  <L22>
1000428f4:     	strb	w14, [x15, #0x2c]
1000428f8:     	add	x9, x9, #0x1
<L22>:
1000428fc:     	ldr	x15, [x13, #0x30]
100042900:     	ldr	x15, [x15]
100042904:     	ldr	x16, [x15, #0x30]
100042908:     	cbnz	x16,  <L23>
10004290c:     	ldrb	w16, [x15, #0x2c]
100042910:     	tbnz	w16, #0x0,  <L23>
100042914:     	strb	w14, [x15, #0x2c]
100042918:     	add	x9, x9, #0x1
<L23>:
10004291c:     	ldr	x15, [x13, #0x60]
100042920:     	ldr	x15, [x15]
100042924:     	ldr	x16, [x15, #0x30]
100042928:     	cbnz	x16,  <L24>
10004292c:     	ldrb	w16, [x15, #0x2c]
100042930:     	tbnz	w16, #0x0,  <L24>
100042934:     	strb	w14, [x15, #0x2c]
100042938:     	add	x9, x9, #0x1
<L24>:
10004293c:     	ldr	x15, [x13, #0x90]
100042940:     	ldr	x15, [x15]
100042944:     	ldr	x16, [x15, #0x30]
100042948:     	cbnz	x16,  <L16>
10004294c:     	ldrb	w16, [x15, #0x2c]
100042950:     	tbnz	w16, #0x0,  <L16>
100042954:     	strb	w14, [x15, #0x2c]
100042958:     	add	x9, x9, #0x1
10004295c:     	b	 <L16>
<L25>:
100042960:     	neg	x11, x11
100042964:     	cbz	x8,  <L29>
<L26>:
100042968:     	mov	w12, #0x30              ; =48
10004296c:     	madd	x10, x11, x12, x10
100042970:     	add	x10, x10, #0x28
100042974:     	mov	w11, #0x1               ; =1
100042978:     	b	 <L28>
<L27>:
10004297c:     	subs	x8, x8, #0x1
100042980:     	b.eq	 <L29>
<L28>:
100042984:     	ldr	x12, [x10], #0x30
100042988:     	ldr	x12, [x12]
10004298c:     	ldr	x13, [x12, #0x30]
100042990:     	cbnz	x13,  <L27>
100042994:     	ldrb	w13, [x12, #0x2c]
100042998:     	tbnz	w13, #0x0,  <L27>
10004299c:     	strb	w11, [x12, #0x2c]
1000429a0:     	add	x9, x9, #0x1
1000429a4:     	b	 <L27>
<L29>:
1000429a8:     	cmp	x9, x19
1000429ac:     	eor	w8, w20, #0x1
1000429b0:     	csinc	w8, w8, wzr, lo
1000429b4:     	tbnz	w8, #0x0,  <L36>
<L30>:
1000429b8:     	ldr	x19, [sp, #0x18]
1000429bc:     	cbz	x21,  <L34>
1000429c0:     	add	x20, x19, #0x18
1000429c4:     	mov	w22, #0x7f80            ; =32640
1000429c8:     	b	 <L33>
<L31>:
1000429cc:     	mov	w1, #0x1                ; =1
1000429d0:     	bl	 <_scoop_heap_publish_large_object>
1000429d4:     	ldur	x8, [x20, #-0x10]
1000429d8:     	str	x8, [x24, #0x78]
<L32>:
1000429dc:     	ldr	x8, [x23, #0x1d8]
1000429e0:     	add	x8, x8, #0x1
1000429e4:     	str	x8, [x23, #0x1d8]
1000429e8:     	add	x20, x20, #0x30
1000429ec:     	subs	x21, x21, #0x1
1000429f0:     	b.eq	 <L34>
<L33>:
1000429f4:     	ldp	x1, x0, [x20, #-0x18]
1000429f8:     	ldur	x2, [x20, #-0x8]
1000429fc:     	bl	 <dyld_stub_binder+0x100048270>
100042a00:     	ldur	x2, [x20, #-0x8]
100042a04:     	ldr	x8, [x23, #0x158]
100042a08:     	add	x8, x8, x2
100042a0c:     	str	x8, [x23, #0x158]
100042a10:     	ldp	x24, x0, [x20, #0x8]
100042a14:     	cmp	x2, x22
100042a18:     	b.hi	 <L31>
100042a1c:     	ldur	x1, [x20, #-0x10]
100042a20:     	mov	w3, #0x1                ; =1
100042a24:     	bl	 <_scoop_heap_record_small_object>
100042a28:     	ldur	x8, [x20, #-0x10]
100042a2c:     	ldr	x9, [x24, #0x58]
100042a30:     	ldr	x10, [x20]
100042a34:     	str	x8, [x9, x10, lsl #3]
100042a38:     	b	 <L32>
<L34>:
100042a3c:     	mov	x0, x19
100042a40:     	bl	 <dyld_stub_binder+0x1000481f8>
<L35>:
100042a44:     	mov	w0, #0x1                ; =1
100042a48:     	b	 <L43>
<L36>:
100042a4c:     	ldr	x0, [sp, #0x18]
100042a50:     	bl	 <dyld_stub_binder+0x1000481f8>
100042a54:     	bl	 <_scoop_heap_first_block>
100042a58:     	cbz	x0,  <L40>
100042a5c:     	mov	w20, #0x2               ; =2
100042a60:     	mov	w21, #0x5               ; =5
100042a64:     	b	 <L39>
<L37>:
100042a68:     	mov	x19, x0
100042a6c:     	bl	 <_scoop_heap_block_has_pins>
100042a70:     	cmp	w0, #0x0
100042a74:     	csel	w8, w21, w20, ne
100042a78:     	str	w8, [x19, #0x14]
100042a7c:     	ldr	x0, [x19, #0x58]
100042a80:     	bl	 <dyld_stub_binder+0x1000481f8>
100042a84:     	mov	x0, x19
100042a88:     	str	xzr, [x19, #0x58]
<L38>:
100042a8c:     	bl	 <_scoop_heap_next_block>
100042a90:     	cbz	x0,  <L40>
<L39>:
100042a94:     	ldr	w8, [x0, #0x14]
100042a98:     	cmp	w8, #0x3
100042a9c:     	b.eq	 <L37>
100042aa0:     	cmp	w8, #0x4
100042aa4:     	b.ne	 <L38>
100042aa8:     	mov	x19, x0
100042aac:     	bl	 <_scoop_heap_release_block>
100042ab0:     	mov	x0, x19
100042ab4:     	b	 <L38>
<L40>:
100042ab8:     	ldr	x8, [x23, #0x40]
100042abc:     	cbz	x8,  <L42>
<L41>:
100042ac0:     	sturh	wzr, [x8, #0x2b]
100042ac4:     	ldr	x8, [x8, #0x18]
100042ac8:     	cbnz	x8,  <L41>
<L42>:
100042acc:     	stp	xzr, xzr, [x23, #0x1c8]
100042ad0:     	str	xzr, [x23, #0x1c0]
100042ad4:     	ldrb	w8, [x23, #0x99]
100042ad8:     	cmp	w8, #0x1
100042adc:     	b.eq	 <L45>
100042ae0:     	mov	w0, #0x0                ; =0
<L43>:
100042ae4:     	ldp	x29, x30, [sp, #0x60]
100042ae8:     	ldp	x20, x19, [sp, #0x50]
100042aec:     	ldp	x22, x21, [sp, #0x40]
100042af0:     	ldp	x24, x23, [sp, #0x30]
100042af4:     	ldp	x26, x25, [sp, #0x20]
100042af8:     	add	sp, sp, #0x70
100042afc:     	ret
<L44>:
100042b00:     	adrp	x0, 0x100065000 <_check_release_hook.domain+0x511>
100042b04:     	add	x0, x0, #0xc96
100042b08:     	bl	 <_scoop_heap_fatal>
<L45>:
100042b0c:     	adrp	x0, 0x100065000 <_check_release_hook.domain+0x511>
100042b10:     	add	x0, x0, #0xcbc
100042b14:     	bl	 <_scoop_heap_fatal>

0000000100043c98 <_darwin_discard_pages>:
100043c98:     	cbz	x2,  <L1>
100043c9c:     	cbz	x0,  <L0>
100043ca0:     	cbz	x1,  <L0>
100043ca4:     	stp	x20, x19, [sp, #-0x20]!
100043ca8:     	stp	x29, x30, [sp, #0x10]
100043cac:     	add	x29, sp, #0x10
100043cb0:     	mov	x19, x2
100043cb4:     	mov	w2, #0x4                ; =4
100043cb8:     	bl	 <dyld_stub_binder+0x100048240>
100043cbc:     	mov	x2, x19
100043cc0:     	ldp	x29, x30, [sp, #0x10]
100043cc4:     	ldp	x20, x19, [sp], #0x20
100043cc8:     	cbz	w0,  <L2>
<L0>:
100043ccc:     	mov	w0, #0x0                ; =0
100043cd0:     	mov	w8, #0x7                ; =7
100043cd4:     	str	w8, [x2]
100043cd8:     	ret
<L1>:
100043cdc:     	mov	w0, #0x0                ; =0
100043ce0:     	ret
<L2>:
100043ce4:     	mov	w0, #0x1                ; =1
100043ce8:     	ret

000000010004403c <_scoop_gc_heap_finish_collection_locked>:
10004403c:     	sub	sp, sp, #0x60
100044040:     	stp	x24, x23, [sp, #0x20]
100044044:     	stp	x22, x21, [sp, #0x30]
100044048:     	stp	x20, x19, [sp, #0x40]
10004404c:     	stp	x29, x30, [sp, #0x50]
100044050:     	add	x29, sp, #0x50
100044054:     	adrp	x22, 0x10015c000 <dyld_stub_binder+0x10015c000>
100044058:     	add	x22, x22, #0x418
10004405c:     	ldrb	w8, [x22, #0x9d]
100044060:     	tbz	w8, #0x0,  <L11>
100044064:     	mov	x19, x1
100044068:     	mov	x20, x0
10004406c:     	mov	x8, sp
100044070:     	bl	 <_scoop_thread_allocation_totals_locked>
100044074:     	cbz	w19,  <L0>
100044078:     	ldp	x8, x9, [sp]
10004407c:     	ldp	x11, x10, [x22, #0x80]
100044080:     	ldr	x12, [x22, #0x78]
100044084:     	add	x8, x8, x20
100044088:     	sub	x8, x8, x9
10004408c:     	sub	x8, x8, x11
100044090:     	add	x9, x10, x12
100044094:     	add	x20, x8, x9
100044098:     	bl	 <_scoop_heap_first_block>
10004409c:     	cbnz	x0,  <L1>
1000440a0:     	b	 <L7>
<L0>:
1000440a4:     	bl	 <_scoop_heap_free_run_nodes>
1000440a8:     	bl	 <_scoop_heap_first_block>
1000440ac:     	cbz	x0,  <L7>
<L1>:
1000440b0:     	mov	x21, x0
1000440b4:     	mov	w23, #0x1               ; =1
1000440b8:     	b	 <L5>
<L2>:
1000440bc:     	cbz	w8,  <L6>
<L3>:
1000440c0:     	str	w23, [x21, #0x1c]
1000440c4:     	ldrb	w1, [x22, #0x99]
1000440c8:     	mov	x0, x21
1000440cc:     	bl	 <_scoop_heap_finish_block>
<L4>:
1000440d0:     	mov	x0, x21
1000440d4:     	bl	 <_scoop_heap_next_block>
1000440d8:     	mov	x21, x0
1000440dc:     	cbz	x0,  <L7>
<L5>:
1000440e0:     	mov	x0, x21
1000440e4:     	bl	 <_scoop_heap_active_head>
1000440e8:     	cbz	w0,  <L4>
1000440ec:     	ldr	w8, [x21, #0x1c]
1000440f0:     	cbz	w19,  <L2>
1000440f4:     	cmp	w8, #0x1
1000440f8:     	b.ne	 <L2>
1000440fc:     	ldr	w8, [x21, #0x14]
100044100:     	cmp	w8, #0x4
100044104:     	b.eq	 <L3>
100044108:     	b	 <L4>
<L6>:
10004410c:     	ldr	x8, [x21, #0x68]
100044110:     	ldr	x9, [x22, #0xd0]
100044114:     	add	x8, x9, x8
100044118:     	str	x8, [x22, #0xd0]
10004411c:     	b	 <L3>
<L7>:
100044120:     	stp	xzr, xzr, [x22, #0x1c0]
100044124:     	str	xzr, [x22, #0x1d0]
100044128:     	ldrb	w8, [x22, #0x99]
10004412c:     	orr	w8, w19, w8
100044130:     	mov	w9, #0x1                ; =1
100044134:     	bic	w0, w9, w8
100044138:     	bl	 <_scoop_heap_reclaim_regions>
10004413c:     	ldr	x21, [x22, #0x40]
100044140:     	cbz	x21,  <L9>
<L8>:
100044144:     	ldp	x8, x0, [x21, #0x8]
100044148:     	lsr	x1, x8, #9
10004414c:     	bl	 <dyld_stub_binder+0x100048198>
100044150:     	sturh	wzr, [x21, #0x2b]
100044154:     	ldr	x21, [x21, #0x18]
100044158:     	cbnz	x21,  <L8>
<L9>:
10004415c:     	str	xzr, [x22, #0xa0]
100044160:     	str	x20, [x22, #0x78]
100044164:     	ldr	q0, [sp]
100044168:     	str	q0, [x22, #0x80]
10004416c:     	ldr	x8, [x22, #0x1d8]
100044170:     	add	x9, x22, #0x90
100044174:     	stlr	x8, [x9]
100044178:     	tbnz	w19, #0x0,  <L10>
10004417c:     	ldr	x8, [x22, #0x60]
100044180:     	lsl	x9, x8, #1
100044184:     	mov	w10, #0x1000000         ; =16777216
100044188:     	cmp	x9, x10
10004418c:     	csel	x9, x9, x10, hi
100044190:     	cmn	x8, #0x1
100044194:     	csinv	x8, x9, xzr, gt
100044198:     	str	x8, [x22, #0x68]
<L10>:
10004419c:     	strb	wzr, [x22, #0x9d]
1000441a0:     	ldp	x29, x30, [sp, #0x50]
1000441a4:     	ldp	x20, x19, [sp, #0x40]
1000441a8:     	ldp	x22, x21, [sp, #0x30]
1000441ac:     	ldp	x24, x23, [sp, #0x20]
1000441b0:     	add	sp, sp, #0x60
1000441b4:     	ret
<L11>:
1000441b8:     	adrp	x0, 0x100066000 <_check_release_hook.domain+0x1511>
1000441bc:     	add	x0, x0, #0x28d
1000441c0:     	bl	 <_scoop_heap_fatal>

0000000100044a9c <_scoop_heap_region_destroy>:
100044a9c:     	sub	sp, sp, #0x60
100044aa0:     	stp	x24, x23, [sp, #0x20]
100044aa4:     	stp	x22, x21, [sp, #0x30]
100044aa8:     	stp	x20, x19, [sp, #0x40]
100044aac:     	stp	x29, x30, [sp, #0x50]
100044ab0:     	add	x29, sp, #0x50
100044ab4:     	mov	x19, x0
100044ab8:     	bl	 <_scoop_heap_page_map_remove>
100044abc:     	ldp	x20, x21, [x19]
100044ac0:     	stp	xzr, xzr, [sp, #0x8]
100044ac4:     	str	xzr, [sp, #0x18]
100044ac8:     	bl	 <_scoop_platform_bundle>
100044acc:     	ldr	x8, [x0, #0x8]
100044ad0:     	ldr	x8, [x8, #0x20]
100044ad4:     	add	x2, sp, #0x8
100044ad8:     	mov	x0, x20
100044adc:     	mov	x1, x21
100044ae0:     	blr	x8
100044ae4:     	tbz	w0, #0x0,  <L2>
100044ae8:     	mov	x20, #0x0               ; =0
100044aec:     	mov	x21, #0x0               ; =0
100044af0:     	ldr	x8, [x19, #0x8]
100044af4:     	adrp	x9, 0x10015c000 <dyld_stub_binder+0x10015c000>
100044af8:     	add	x9, x9, #0x418
100044afc:     	ldr	x10, [x9, #0x140]
100044b00:     	add	x8, x10, x8
100044b04:     	str	x8, [x9, #0x140]
100044b08:     	ldrb	w22, [x19, #0x2a]
<L0>:
100044b0c:     	ldr	x8, [x19, #0x20]
100044b10:     	add	x23, x8, x20
100044b14:     	ldr	x0, [x23, #0x20]
100044b18:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b1c:     	ldr	x0, [x23, #0x28]
100044b20:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b24:     	ldr	x0, [x23, #0x30]
100044b28:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b2c:     	ldr	x0, [x23, #0x38]
100044b30:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b34:     	ldr	x0, [x23, #0x40]
100044b38:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b3c:     	ldr	x0, [x23, #0x48]
100044b40:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b44:     	ldr	x0, [x23, #0x50]
100044b48:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b4c:     	ldr	x0, [x23, #0x58]
100044b50:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b54:     	tbnz	w22, #0x0,  <L1>
100044b58:     	add	x20, x20, #0x88
100044b5c:     	cmp	x21, #0x1fe
100044b60:     	add	x21, x21, #0x1
100044b64:     	b.ls	 <L0>
<L1>:
100044b68:     	ldr	x0, [x19, #0x20]
100044b6c:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b70:     	ldr	x0, [x19, #0x10]
100044b74:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b78:     	mov	x0, x19
100044b7c:     	bl	 <dyld_stub_binder+0x1000481f8>
100044b80:     	ldp	x29, x30, [sp, #0x50]
100044b84:     	ldp	x20, x19, [sp, #0x40]
100044b88:     	ldp	x22, x21, [sp, #0x30]
100044b8c:     	ldp	x24, x23, [sp, #0x20]
100044b90:     	add	sp, sp, #0x60
100044b94:     	ret
<L2>:
100044b98:     	ldr	w0, [sp, #0x8]
100044b9c:     	bl	 <_scoop_platform_error_message>
100044ba0:     	bl	 <_scoop_heap_fatal>
