
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
10002efbc:     	bl	 <dyld_stub_binder+0x10004835c>
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
100032af4:     	adrp	x0, 0x100062000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x740>
100032af8:     	add	x0, x0, #0x4cf
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
100034e9c:     	bl	 <dyld_stub_binder+0x100048164>
100034ea0:     	str	x0, [x21, #0x58]
100034ea4:     	cbnz	x0,  <L14>
100034ea8:     	adrp	x0, 0x100062000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x740>
100034eac:     	add	x0, x0, #0xa8e
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

0000000100042030 <_scoop_gc_heap_plan_moving_locked>:
100042030:     	sub	sp, sp, #0x70
100042034:     	stp	x26, x25, [sp, #0x20]
100042038:     	stp	x24, x23, [sp, #0x30]
10004203c:     	stp	x22, x21, [sp, #0x40]
100042040:     	stp	x20, x19, [sp, #0x50]
100042044:     	stp	x29, x30, [sp, #0x60]
100042048:     	add	x29, sp, #0x60
10004204c:     	adrp	x23, 0x10015c000 <dyld_stub_binder+0x10015c000>
100042050:     	add	x23, x23, #0x418
100042054:     	ldrb	w8, [x23, #0x9d]
100042058:     	tbz	w8, #0x0,  <L44>
10004205c:     	mov	x20, x0
100042060:     	bl	 <_scoop_heap_select_evacuation_sources>
100042064:     	mov	x19, x0
100042068:     	ldrb	w8, [x23, #0x99]
10004206c:     	orr	w24, w20, w8
100042070:     	tbnz	w24, #0x0,  <L0>
100042074:     	cbz	x19,  <L35>
<L0>:
100042078:     	tbnz	w24, #0x0,  <L1>
10004207c:     	bl	 <_scoop_heap_prepare_evacuation_targets>
<L1>:
100042080:     	cmp	x19, #0x1
100042084:     	stp	xzr, xzr, [sp, #0x10]
100042088:     	cset	w20, hi
10004208c:     	str	xzr, [sp, #0x8]
100042090:     	bl	 <_scoop_heap_first_block>
100042094:     	cbz	x0,  <L10>
100042098:     	mov	x21, x0
10004209c:     	eor	w25, w24, #0x1
1000420a0:     	orr	w26, w24, w20
<L2>:
1000420a4:     	ldr	w8, [x21, #0x14]
1000420a8:     	cmp	w8, #0x3
1000420ac:     	b.ne	 <L3>
1000420b0:     	ldr	w8, [x21, #0x18]
1000420b4:     	cmp	w8, #0x2
1000420b8:     	b.ne	 <L5>
1000420bc:     	add	x0, sp, #0x18
1000420c0:     	add	x1, sp, #0x10
1000420c4:     	add	x2, sp, #0x8
1000420c8:     	and	w5, w25, #0x1
1000420cc:     	and	w6, w26, #0x1
1000420d0:     	mov	x3, x21
1000420d4:     	mov	w4, #0x10               ; =16
1000420d8:     	bl	 <_append_move>
1000420dc:     	mov	x20, x0
1000420e0:     	b	 <L4>
<L3>:
1000420e4:     	mov	w20, #0x1               ; =1
<L4>:
1000420e8:     	mov	x0, x21
1000420ec:     	bl	 <_scoop_heap_next_block>
1000420f0:     	cbz	w20,  <L9>
1000420f4:     	mov	x21, x0
1000420f8:     	cbnz	x0,  <L2>
1000420fc:     	b	 <L9>
<L5>:
100042100:     	mov	w22, #0x10              ; =16
100042104:     	b	 <L8>
<L6>:
100042108:     	mov	w20, #0x1               ; =1
10004210c:     	cbz	w20,  <L4>
<L7>:
100042110:     	cmp	x22, #0xfff
100042114:     	add	x22, x22, #0x1
100042118:     	b.hs	 <L4>
<L8>:
10004211c:     	ldr	x0, [x21, #0x20]
100042120:     	mov	x1, x22
100042124:     	bl	 <_scoop_heap_bit_test>
100042128:     	cbz	w0,  <L6>
10004212c:     	ldr	x0, [x21, #0x28]
100042130:     	mov	x1, x22
100042134:     	bl	 <_scoop_heap_bit_test>
100042138:     	cbz	w0,  <L6>
10004213c:     	ldr	x0, [x21, #0x30]
100042140:     	mov	x1, x22
100042144:     	bl	 <_scoop_heap_bit_test>
100042148:     	tbnz	w0, #0x0,  <L6>
10004214c:     	add	x0, sp, #0x18
100042150:     	add	x1, sp, #0x10
100042154:     	add	x2, sp, #0x8
100042158:     	and	w5, w25, #0x1
10004215c:     	and	w6, w26, #0x1
100042160:     	mov	x3, x21
100042164:     	mov	x4, x22
100042168:     	bl	 <_append_move>
10004216c:     	mov	x20, x0
100042170:     	cbnz	w20,  <L7>
100042174:     	b	 <L4>
<L9>:
100042178:     	tbz	w24, #0x0,  <L13>
10004217c:     	cbnz	w20,  <L11>
100042180:     	b	 <L36>
<L10>:
100042184:     	tbz	w24, #0x0,  <L12>
<L11>:
100042188:     	ldr	x21, [sp, #0x10]
10004218c:     	b	 <L30>
<L12>:
100042190:     	mov	w20, #0x1               ; =1
<L13>:
100042194:     	ldr	x21, [sp, #0x10]
100042198:     	cbz	x21,  <L14>
10004219c:     	ldr	x10, [sp, #0x18]
1000421a0:     	and	x8, x21, #0x7
1000421a4:     	cmp	x21, #0x8
1000421a8:     	b.hs	 <L15>
1000421ac:     	mov	x11, #0x0               ; =0
1000421b0:     	mov	x9, #0x0                ; =0
1000421b4:     	cbnz	x8,  <L26>
1000421b8:     	b	 <L29>
<L14>:
1000421bc:     	mov	x9, #0x0                ; =0
1000421c0:     	b	 <L29>
<L15>:
1000421c4:     	mov	x11, #0x0               ; =0
1000421c8:     	mov	x9, #0x0                ; =0
1000421cc:     	and	x12, x21, #0xfffffffffffffff8
1000421d0:     	neg	x12, x12
1000421d4:     	add	x13, x10, #0xe8
1000421d8:     	mov	w14, #0x1               ; =1
1000421dc:     	b	 <L17>
<L16>:
1000421e0:     	sub	x11, x11, #0x8
1000421e4:     	add	x13, x13, #0x180
1000421e8:     	cmp	x12, x11
1000421ec:     	b.eq	 <L25>
<L17>:
1000421f0:     	ldur	x15, [x13, #-0xc0]
1000421f4:     	ldr	x15, [x15]
1000421f8:     	ldr	x16, [x15, #0x30]
1000421fc:     	cbnz	x16,  <L18>
100042200:     	ldrb	w16, [x15, #0x2c]
100042204:     	tbnz	w16, #0x0,  <L18>
100042208:     	strb	w14, [x15, #0x2c]
10004220c:     	add	x9, x9, #0x1
<L18>:
100042210:     	ldur	x15, [x13, #-0x90]
100042214:     	ldr	x15, [x15]
100042218:     	ldr	x16, [x15, #0x30]
10004221c:     	cbnz	x16,  <L19>
100042220:     	ldrb	w16, [x15, #0x2c]
100042224:     	tbnz	w16, #0x0,  <L19>
100042228:     	strb	w14, [x15, #0x2c]
10004222c:     	add	x9, x9, #0x1
<L19>:
100042230:     	ldur	x15, [x13, #-0x60]
100042234:     	ldr	x15, [x15]
100042238:     	ldr	x16, [x15, #0x30]
10004223c:     	cbnz	x16,  <L20>
100042240:     	ldrb	w16, [x15, #0x2c]
100042244:     	tbnz	w16, #0x0,  <L20>
100042248:     	strb	w14, [x15, #0x2c]
10004224c:     	add	x9, x9, #0x1
<L20>:
100042250:     	ldur	x15, [x13, #-0x30]
100042254:     	ldr	x15, [x15]
100042258:     	ldr	x16, [x15, #0x30]
10004225c:     	cbnz	x16,  <L21>
100042260:     	ldrb	w16, [x15, #0x2c]
100042264:     	tbnz	w16, #0x0,  <L21>
100042268:     	strb	w14, [x15, #0x2c]
10004226c:     	add	x9, x9, #0x1
<L21>:
100042270:     	ldr	x15, [x13]
100042274:     	ldr	x15, [x15]
100042278:     	ldr	x16, [x15, #0x30]
10004227c:     	cbnz	x16,  <L22>
100042280:     	ldrb	w16, [x15, #0x2c]
100042284:     	tbnz	w16, #0x0,  <L22>
100042288:     	strb	w14, [x15, #0x2c]
10004228c:     	add	x9, x9, #0x1
<L22>:
100042290:     	ldr	x15, [x13, #0x30]
100042294:     	ldr	x15, [x15]
100042298:     	ldr	x16, [x15, #0x30]
10004229c:     	cbnz	x16,  <L23>
1000422a0:     	ldrb	w16, [x15, #0x2c]
1000422a4:     	tbnz	w16, #0x0,  <L23>
1000422a8:     	strb	w14, [x15, #0x2c]
1000422ac:     	add	x9, x9, #0x1
<L23>:
1000422b0:     	ldr	x15, [x13, #0x60]
1000422b4:     	ldr	x15, [x15]
1000422b8:     	ldr	x16, [x15, #0x30]
1000422bc:     	cbnz	x16,  <L24>
1000422c0:     	ldrb	w16, [x15, #0x2c]
1000422c4:     	tbnz	w16, #0x0,  <L24>
1000422c8:     	strb	w14, [x15, #0x2c]
1000422cc:     	add	x9, x9, #0x1
<L24>:
1000422d0:     	ldr	x15, [x13, #0x90]
1000422d4:     	ldr	x15, [x15]
1000422d8:     	ldr	x16, [x15, #0x30]
1000422dc:     	cbnz	x16,  <L16>
1000422e0:     	ldrb	w16, [x15, #0x2c]
1000422e4:     	tbnz	w16, #0x0,  <L16>
1000422e8:     	strb	w14, [x15, #0x2c]
1000422ec:     	add	x9, x9, #0x1
1000422f0:     	b	 <L16>
<L25>:
1000422f4:     	neg	x11, x11
1000422f8:     	cbz	x8,  <L29>
<L26>:
1000422fc:     	mov	w12, #0x30              ; =48
100042300:     	madd	x10, x11, x12, x10
100042304:     	add	x10, x10, #0x28
100042308:     	mov	w11, #0x1               ; =1
10004230c:     	b	 <L28>
<L27>:
100042310:     	subs	x8, x8, #0x1
100042314:     	b.eq	 <L29>
<L28>:
100042318:     	ldr	x12, [x10], #0x30
10004231c:     	ldr	x12, [x12]
100042320:     	ldr	x13, [x12, #0x30]
100042324:     	cbnz	x13,  <L27>
100042328:     	ldrb	w13, [x12, #0x2c]
10004232c:     	tbnz	w13, #0x0,  <L27>
100042330:     	strb	w11, [x12, #0x2c]
100042334:     	add	x9, x9, #0x1
100042338:     	b	 <L27>
<L29>:
10004233c:     	cmp	x9, x19
100042340:     	eor	w8, w20, #0x1
100042344:     	csinc	w8, w8, wzr, lo
100042348:     	tbnz	w8, #0x0,  <L36>
<L30>:
10004234c:     	ldr	x19, [sp, #0x18]
100042350:     	cbz	x21,  <L34>
100042354:     	add	x20, x19, #0x18
100042358:     	mov	w22, #0x7f80            ; =32640
10004235c:     	b	 <L33>
<L31>:
100042360:     	mov	w1, #0x1                ; =1
100042364:     	bl	 <_scoop_heap_publish_large_object>
100042368:     	ldur	x8, [x20, #-0x10]
10004236c:     	str	x8, [x24, #0x78]
<L32>:
100042370:     	ldr	x8, [x23, #0x1d8]
100042374:     	add	x8, x8, #0x1
100042378:     	str	x8, [x23, #0x1d8]
10004237c:     	add	x20, x20, #0x30
100042380:     	subs	x21, x21, #0x1
100042384:     	b.eq	 <L34>
<L33>:
100042388:     	ldp	x1, x0, [x20, #-0x18]
10004238c:     	ldur	x2, [x20, #-0x8]
100042390:     	bl	 <dyld_stub_binder+0x100048230>
100042394:     	ldur	x2, [x20, #-0x8]
100042398:     	ldr	x8, [x23, #0x158]
10004239c:     	add	x8, x8, x2
1000423a0:     	str	x8, [x23, #0x158]
1000423a4:     	ldp	x24, x0, [x20, #0x8]
1000423a8:     	cmp	x2, x22
1000423ac:     	b.hi	 <L31>
1000423b0:     	ldur	x1, [x20, #-0x10]
1000423b4:     	mov	w3, #0x1                ; =1
1000423b8:     	bl	 <_scoop_heap_record_small_object>
1000423bc:     	ldur	x8, [x20, #-0x10]
1000423c0:     	ldr	x9, [x24, #0x58]
1000423c4:     	ldr	x10, [x20]
1000423c8:     	str	x8, [x9, x10, lsl #3]
1000423cc:     	b	 <L32>
<L34>:
1000423d0:     	mov	x0, x19
1000423d4:     	bl	 <dyld_stub_binder+0x1000481b8>
<L35>:
1000423d8:     	mov	w0, #0x1                ; =1
1000423dc:     	b	 <L43>
<L36>:
1000423e0:     	ldr	x0, [sp, #0x18]
1000423e4:     	bl	 <dyld_stub_binder+0x1000481b8>
1000423e8:     	bl	 <_scoop_heap_first_block>
1000423ec:     	cbz	x0,  <L40>
1000423f0:     	mov	w20, #0x2               ; =2
1000423f4:     	mov	w21, #0x5               ; =5
1000423f8:     	b	 <L39>
<L37>:
1000423fc:     	mov	x19, x0
100042400:     	bl	 <_scoop_heap_block_has_pins>
100042404:     	cmp	w0, #0x0
100042408:     	csel	w8, w21, w20, ne
10004240c:     	str	w8, [x19, #0x14]
100042410:     	ldr	x0, [x19, #0x58]
100042414:     	bl	 <dyld_stub_binder+0x1000481b8>
100042418:     	mov	x0, x19
10004241c:     	str	xzr, [x19, #0x58]
<L38>:
100042420:     	bl	 <_scoop_heap_next_block>
100042424:     	cbz	x0,  <L40>
<L39>:
100042428:     	ldr	w8, [x0, #0x14]
10004242c:     	cmp	w8, #0x3
100042430:     	b.eq	 <L37>
100042434:     	cmp	w8, #0x4
100042438:     	b.ne	 <L38>
10004243c:     	mov	x19, x0
100042440:     	bl	 <_scoop_heap_release_block>
100042444:     	mov	x0, x19
100042448:     	b	 <L38>
<L40>:
10004244c:     	ldr	x8, [x23, #0x40]
100042450:     	cbz	x8,  <L42>
<L41>:
100042454:     	sturh	wzr, [x8, #0x2b]
100042458:     	ldr	x8, [x8, #0x18]
10004245c:     	cbnz	x8,  <L41>
<L42>:
100042460:     	stp	xzr, xzr, [x23, #0x1c8]
100042464:     	str	xzr, [x23, #0x1c0]
100042468:     	ldrb	w8, [x23, #0x99]
10004246c:     	cmp	w8, #0x1
100042470:     	b.eq	 <L45>
100042474:     	mov	w0, #0x0                ; =0
<L43>:
100042478:     	ldp	x29, x30, [sp, #0x60]
10004247c:     	ldp	x20, x19, [sp, #0x50]
100042480:     	ldp	x22, x21, [sp, #0x40]
100042484:     	ldp	x24, x23, [sp, #0x30]
100042488:     	ldp	x26, x25, [sp, #0x20]
10004248c:     	add	sp, sp, #0x70
100042490:     	ret
<L44>:
100042494:     	adrp	x0, 0x100065000 <_check_release_hook.domain+0x681>
100042498:     	add	x0, x0, #0xb26
10004249c:     	bl	 <_scoop_heap_fatal>
<L45>:
1000424a0:     	adrp	x0, 0x100065000 <_check_release_hook.domain+0x681>
1000424a4:     	add	x0, x0, #0xb4c
1000424a8:     	bl	 <_scoop_heap_fatal>

000000010004362c <_darwin_discard_pages>:
10004362c:     	cbz	x2,  <L1>
100043630:     	cbz	x0,  <L0>
100043634:     	cbz	x1,  <L0>
100043638:     	stp	x20, x19, [sp, #-0x20]!
10004363c:     	stp	x29, x30, [sp, #0x10]
100043640:     	add	x29, sp, #0x10
100043644:     	mov	x19, x2
100043648:     	mov	w2, #0x4                ; =4
10004364c:     	bl	 <dyld_stub_binder+0x100048200>
100043650:     	mov	x2, x19
100043654:     	ldp	x29, x30, [sp, #0x10]
100043658:     	ldp	x20, x19, [sp], #0x20
10004365c:     	cbz	w0,  <L2>
<L0>:
100043660:     	mov	w0, #0x0                ; =0
100043664:     	mov	w8, #0x7                ; =7
100043668:     	str	w8, [x2]
10004366c:     	ret
<L1>:
100043670:     	mov	w0, #0x0                ; =0
100043674:     	ret
<L2>:
100043678:     	mov	w0, #0x1                ; =1
10004367c:     	ret

00000001000439d0 <_scoop_gc_heap_finish_collection_locked>:
1000439d0:     	sub	sp, sp, #0x60
1000439d4:     	stp	x24, x23, [sp, #0x20]
1000439d8:     	stp	x22, x21, [sp, #0x30]
1000439dc:     	stp	x20, x19, [sp, #0x40]
1000439e0:     	stp	x29, x30, [sp, #0x50]
1000439e4:     	add	x29, sp, #0x50
1000439e8:     	adrp	x22, 0x10015c000 <dyld_stub_binder+0x10015c000>
1000439ec:     	add	x22, x22, #0x418
1000439f0:     	ldrb	w8, [x22, #0x9d]
1000439f4:     	tbz	w8, #0x0,  <L11>
1000439f8:     	mov	x19, x1
1000439fc:     	mov	x20, x0
100043a00:     	mov	x8, sp
100043a04:     	bl	 <_scoop_thread_allocation_totals_locked>
100043a08:     	cbz	w19,  <L0>
100043a0c:     	ldp	x8, x9, [sp]
100043a10:     	ldp	x11, x10, [x22, #0x80]
100043a14:     	ldr	x12, [x22, #0x78]
100043a18:     	add	x8, x8, x20
100043a1c:     	sub	x8, x8, x9
100043a20:     	sub	x8, x8, x11
100043a24:     	add	x9, x10, x12
100043a28:     	add	x20, x8, x9
100043a2c:     	bl	 <_scoop_heap_first_block>
100043a30:     	cbnz	x0,  <L1>
100043a34:     	b	 <L7>
<L0>:
100043a38:     	bl	 <_scoop_heap_free_run_nodes>
100043a3c:     	bl	 <_scoop_heap_first_block>
100043a40:     	cbz	x0,  <L7>
<L1>:
100043a44:     	mov	x21, x0
100043a48:     	mov	w23, #0x1               ; =1
100043a4c:     	b	 <L5>
<L2>:
100043a50:     	cbz	w8,  <L6>
<L3>:
100043a54:     	str	w23, [x21, #0x1c]
100043a58:     	ldrb	w1, [x22, #0x99]
100043a5c:     	mov	x0, x21
100043a60:     	bl	 <_scoop_heap_finish_block>
<L4>:
100043a64:     	mov	x0, x21
100043a68:     	bl	 <_scoop_heap_next_block>
100043a6c:     	mov	x21, x0
100043a70:     	cbz	x0,  <L7>
<L5>:
100043a74:     	mov	x0, x21
100043a78:     	bl	 <_scoop_heap_active_head>
100043a7c:     	cbz	w0,  <L4>
100043a80:     	ldr	w8, [x21, #0x1c]
100043a84:     	cbz	w19,  <L2>
100043a88:     	cmp	w8, #0x1
100043a8c:     	b.ne	 <L2>
100043a90:     	ldr	w8, [x21, #0x14]
100043a94:     	cmp	w8, #0x4
100043a98:     	b.eq	 <L3>
100043a9c:     	b	 <L4>
<L6>:
100043aa0:     	ldr	x8, [x21, #0x68]
100043aa4:     	ldr	x9, [x22, #0xd0]
100043aa8:     	add	x8, x9, x8
100043aac:     	str	x8, [x22, #0xd0]
100043ab0:     	b	 <L3>
<L7>:
100043ab4:     	stp	xzr, xzr, [x22, #0x1c0]
100043ab8:     	str	xzr, [x22, #0x1d0]
100043abc:     	ldrb	w8, [x22, #0x99]
100043ac0:     	orr	w8, w19, w8
100043ac4:     	mov	w9, #0x1                ; =1
100043ac8:     	bic	w0, w9, w8
100043acc:     	bl	 <_scoop_heap_reclaim_regions>
100043ad0:     	ldr	x21, [x22, #0x40]
100043ad4:     	cbz	x21,  <L9>
<L8>:
100043ad8:     	ldp	x8, x0, [x21, #0x8]
100043adc:     	lsr	x1, x8, #9
100043ae0:     	bl	 <dyld_stub_binder+0x100048158>
100043ae4:     	sturh	wzr, [x21, #0x2b]
100043ae8:     	ldr	x21, [x21, #0x18]
100043aec:     	cbnz	x21,  <L8>
<L9>:
100043af0:     	str	xzr, [x22, #0xa0]
100043af4:     	str	x20, [x22, #0x78]
100043af8:     	ldr	q0, [sp]
100043afc:     	str	q0, [x22, #0x80]
100043b00:     	ldr	x8, [x22, #0x1d8]
100043b04:     	add	x9, x22, #0x90
100043b08:     	stlr	x8, [x9]
100043b0c:     	tbnz	w19, #0x0,  <L10>
100043b10:     	ldr	x8, [x22, #0x60]
100043b14:     	lsl	x9, x8, #1
100043b18:     	mov	w10, #0x1000000         ; =16777216
100043b1c:     	cmp	x9, x10
100043b20:     	csel	x9, x9, x10, hi
100043b24:     	cmn	x8, #0x1
100043b28:     	csinv	x8, x9, xzr, gt
100043b2c:     	str	x8, [x22, #0x68]
<L10>:
100043b30:     	strb	wzr, [x22, #0x9d]
100043b34:     	ldp	x29, x30, [sp, #0x50]
100043b38:     	ldp	x20, x19, [sp, #0x40]
100043b3c:     	ldp	x22, x21, [sp, #0x30]
100043b40:     	ldp	x24, x23, [sp, #0x20]
100043b44:     	add	sp, sp, #0x60
100043b48:     	ret
<L11>:
100043b4c:     	adrp	x0, 0x100066000 <_check_release_hook.domain+0x1681>
100043b50:     	add	x0, x0, #0x11d
100043b54:     	bl	 <_scoop_heap_fatal>

0000000100044430 <_scoop_heap_region_destroy>:
100044430:     	sub	sp, sp, #0x60
100044434:     	stp	x24, x23, [sp, #0x20]
100044438:     	stp	x22, x21, [sp, #0x30]
10004443c:     	stp	x20, x19, [sp, #0x40]
100044440:     	stp	x29, x30, [sp, #0x50]
100044444:     	add	x29, sp, #0x50
100044448:     	mov	x19, x0
10004444c:     	bl	 <_scoop_heap_page_map_remove>
100044450:     	ldp	x20, x21, [x19]
100044454:     	stp	xzr, xzr, [sp, #0x8]
100044458:     	str	xzr, [sp, #0x18]
10004445c:     	bl	 <_scoop_platform_bundle>
100044460:     	ldr	x8, [x0, #0x8]
100044464:     	ldr	x8, [x8, #0x20]
100044468:     	add	x2, sp, #0x8
10004446c:     	mov	x0, x20
100044470:     	mov	x1, x21
100044474:     	blr	x8
100044478:     	tbz	w0, #0x0,  <L2>
10004447c:     	mov	x20, #0x0               ; =0
100044480:     	mov	x21, #0x0               ; =0
100044484:     	ldr	x8, [x19, #0x8]
100044488:     	adrp	x9, 0x10015c000 <dyld_stub_binder+0x10015c000>
10004448c:     	add	x9, x9, #0x418
100044490:     	ldr	x10, [x9, #0x140]
100044494:     	add	x8, x10, x8
100044498:     	str	x8, [x9, #0x140]
10004449c:     	ldrb	w22, [x19, #0x2a]
<L0>:
1000444a0:     	ldr	x8, [x19, #0x20]
1000444a4:     	add	x23, x8, x20
1000444a8:     	ldr	x0, [x23, #0x20]
1000444ac:     	bl	 <dyld_stub_binder+0x1000481b8>
1000444b0:     	ldr	x0, [x23, #0x28]
1000444b4:     	bl	 <dyld_stub_binder+0x1000481b8>
1000444b8:     	ldr	x0, [x23, #0x30]
1000444bc:     	bl	 <dyld_stub_binder+0x1000481b8>
1000444c0:     	ldr	x0, [x23, #0x38]
1000444c4:     	bl	 <dyld_stub_binder+0x1000481b8>
1000444c8:     	ldr	x0, [x23, #0x40]
1000444cc:     	bl	 <dyld_stub_binder+0x1000481b8>
1000444d0:     	ldr	x0, [x23, #0x48]
1000444d4:     	bl	 <dyld_stub_binder+0x1000481b8>
1000444d8:     	ldr	x0, [x23, #0x50]
1000444dc:     	bl	 <dyld_stub_binder+0x1000481b8>
1000444e0:     	ldr	x0, [x23, #0x58]
1000444e4:     	bl	 <dyld_stub_binder+0x1000481b8>
1000444e8:     	tbnz	w22, #0x0,  <L1>
1000444ec:     	add	x20, x20, #0x88
1000444f0:     	cmp	x21, #0x1fe
1000444f4:     	add	x21, x21, #0x1
1000444f8:     	b.ls	 <L0>
<L1>:
1000444fc:     	ldr	x0, [x19, #0x20]
100044500:     	bl	 <dyld_stub_binder+0x1000481b8>
100044504:     	ldr	x0, [x19, #0x10]
100044508:     	bl	 <dyld_stub_binder+0x1000481b8>
10004450c:     	mov	x0, x19
100044510:     	bl	 <dyld_stub_binder+0x1000481b8>
100044514:     	ldp	x29, x30, [sp, #0x50]
100044518:     	ldp	x20, x19, [sp, #0x40]
10004451c:     	ldp	x22, x21, [sp, #0x30]
100044520:     	ldp	x24, x23, [sp, #0x20]
100044524:     	add	sp, sp, #0x60
100044528:     	ret
<L2>:
10004452c:     	ldr	w0, [sp, #0x8]
100044530:     	bl	 <_scoop_platform_error_message>
100044534:     	bl	 <_scoop_heap_fatal>

0000000100045ce8 <_scoop_heap_prepare_evacuation_targets>:
100045ce8:     	stp	x24, x23, [sp, #-0x40]!
100045cec:     	stp	x22, x21, [sp, #0x10]
100045cf0:     	stp	x20, x19, [sp, #0x20]
100045cf4:     	stp	x29, x30, [sp, #0x30]
100045cf8:     	add	x29, sp, #0x30
100045cfc:     	bl	 <_scoop_heap_first_block>
100045d00:     	cbz	x0,  <L13>
100045d04:     	mov	x19, x0
100045d08:     	adrp	x22, 0x10015c000 <dyld_stub_binder+0x10015c000>
100045d0c:     	add	x22, x22, #0x418
100045d10:     	b	 <L2>
<L0>:
100045d14:     	mov	x0, x19
100045d18:     	bl	 <_scoop_heap_release_block>
<L1>:
100045d1c:     	mov	x0, x19
100045d20:     	bl	 <_scoop_heap_next_block>
100045d24:     	mov	x19, x0
100045d28:     	cbz	x0,  <L13>
<L2>:
100045d2c:     	mov	x0, x19
100045d30:     	bl	 <_scoop_heap_active_head>
100045d34:     	cbz	w0,  <L1>
100045d38:     	ldr	w8, [x19, #0x18]
100045d3c:     	cmp	w8, #0x1
100045d40:     	b.ne	 <L1>
100045d44:     	ldr	x8, [x19]
100045d48:     	ldrb	w8, [x8, #0x2b]
100045d4c:     	tbnz	w8, #0x0,  <L1>
100045d50:     	ldr	w8, [x19, #0x1c]
100045d54:     	cbz	w8,  <L12>
<L3>:
100045d58:     	mov	w20, #0x10              ; =16
100045d5c:     	b	 <L5>
<L4>:
100045d60:     	add	x20, x20, #0x1
100045d64:     	cmp	x20, #0x1, lsl #12      ; =0x1000
100045d68:     	b.eq	 <L6>
<L5>:
100045d6c:     	ldr	x0, [x19, #0x20]
100045d70:     	mov	x1, x20
100045d74:     	bl	 <_scoop_heap_bit_test>
100045d78:     	cbz	w0,  <L4>
100045d7c:     	ldr	x0, [x19, #0x28]
100045d80:     	mov	x1, x20
100045d84:     	bl	 <_scoop_heap_bit_test>
100045d88:     	tbnz	w0, #0x0,  <L4>
100045d8c:     	mov	x0, x19
100045d90:     	mov	x1, x20
100045d94:     	mov	w2, #0x0                ; =0
100045d98:     	mov	w3, #0x0                ; =0
100045d9c:     	bl	 <_retire_small_object>
100045da0:     	b	 <L4>
<L6>:
100045da4:     	ldr	x8, [x19, #0x68]
100045da8:     	cbz	x8,  <L0>
100045dac:     	mov	x23, #0x0               ; =0
100045db0:     	ldr	x20, [x19, #0x48]
100045db4:     	mov	w21, #0x1               ; =1
100045db8:     	b	 <L9>
<L7>:
100045dbc:     	mov	x1, x23
<L8>:
100045dc0:     	add	x21, x21, #0x1
100045dc4:     	mov	x23, x1
100045dc8:     	cmp	x21, #0x101
100045dcc:     	b.eq	 <L1>
<L9>:
100045dd0:     	cmp	x21, #0x100
100045dd4:     	b.ne	 <L11>
100045dd8:     	cmp	x23, #0x0
100045ddc:     	cset	w8, ne
100045de0:     	mov	w0, #0x1                ; =1
<L10>:
100045de4:     	cbz	w0,  <L7>
100045de8:     	cbz	w8,  <L7>
100045dec:     	subs	x24, x21, x23
100045df0:     	b.eq	 <L14>
100045df4:     	mov	w0, #0x18               ; =24
100045df8:     	bl	 <dyld_stub_binder+0x10004820c>
100045dfc:     	cbz	x0,  <L15>
100045e00:     	mov	x1, #0x0                ; =0
100045e04:     	ldr	x8, [x22, #0x58]
100045e08:     	stp	x8, x19, [x0]
100045e0c:     	strh	w23, [x0, #0x10]
100045e10:     	strh	w24, [x0, #0x12]
100045e14:     	str	wzr, [x0, #0x14]
100045e18:     	str	x0, [x22, #0x58]
100045e1c:     	b	 <L8>
<L11>:
100045e20:     	mov	x0, x20
100045e24:     	mov	x1, x21
100045e28:     	bl	 <_scoop_heap_bit_test>
100045e2c:     	cmp	x23, #0x0
100045e30:     	cset	w8, ne
100045e34:     	tbnz	w0, #0x0,  <L10>
100045e38:     	mov	x1, x21
100045e3c:     	cbz	x23,  <L8>
100045e40:     	b	 <L10>
<L12>:
100045e44:     	ldr	x8, [x19, #0x68]
100045e48:     	cbnz	x8,  <L1>
100045e4c:     	b	 <L3>
<L13>:
100045e50:     	ldp	x29, x30, [sp, #0x30]
100045e54:     	ldp	x20, x19, [sp, #0x20]
100045e58:     	ldp	x22, x21, [sp, #0x10]
100045e5c:     	ldp	x24, x23, [sp], #0x40
100045e60:     	ret
<L14>:
100045e64:     	adrp	x0, 0x100066000 <_check_release_hook.domain+0x1681>
100045e68:     	add	x0, x0, #0x494
100045e6c:     	bl	 <_scoop_heap_fatal>
<L15>:
100045e70:     	adrp	x0, 0x100066000 <_check_release_hook.domain+0x1681>
100045e74:     	add	x0, x0, #0x4aa
100045e78:     	bl	 <_scoop_heap_fatal>
