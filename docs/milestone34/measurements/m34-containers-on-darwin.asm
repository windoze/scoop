; lowOccupancy, MIR fn1, LIR scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-on-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100046944 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2>:
100046944:     	stp	x28, x27, [sp, #-0x60]!
100046948:     	stp	x26, x25, [sp, #0x10]
10004694c:     	stp	x24, x23, [sp, #0x20]
100046950:     	stp	x22, x21, [sp, #0x30]
100046954:     	stp	x20, x19, [sp, #0x40]
100046958:     	stp	x29, x30, [sp, #0x50]
10004695c:     	add	x29, sp, #0x50
100046960:     	sub	sp, sp, #0x270
100046964:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
100046968:     	mov	x20, x0
10004696c:     	add	x22, sp, #0x78
100046970:     	add	x8, x8, #0x5d0
100046974:     	ldr	x9, [x8]
100046978:     	mov	x0, x8
10004697c:     	blr	x9
100046980:     	adrp	x23, 0x10020d000 <_scoop_gc_marker+0x800>
100046984:     	adrp	x25, 0x10020d000 <_scoop_gc_marker+0x800>
100046988:     	add	x23, x23, #0xa10
10004698c:     	ldr	x24, [x0]
100046990:     	add	x25, x25, #0x9fc
100046994:     	add	x10, x24, #0x8
100046998:     	ldar	x8, [x23]
10004699c:     	ldar	w11, [x25]
1000469a0:     	ldar	w9, [x24]
1000469a4:     	ldar	x10, [x10]
1000469a8:     	cbnz	w11, 0x1000469b8 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x74>
1000469ac:     	cmp	w9, #0x1
1000469b0:     	ccmp	x10, x8, #0x0, eq
1000469b4:     	b.eq	0x1000469bc <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x78>
1000469b8:     	bl	0x100058378 <_scoop_rt_safepoint>
1000469bc:     	movi.2d	v0, #0000000000000000
1000469c0:     	add	x0, sp, #0x60
1000469c4:     	mov	x1, xzr
1000469c8:     	mov	x2, xzr
1000469cc:     	str	xzr, [sp, #0x70]
1000469d0:     	str	q0, [sp, #0x60]
1000469d4:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
1000469d8:     	movi.2d	v0, #0000000000000000
1000469dc:     	add	x0, sp, #0x78
1000469e0:     	add	x1, sp, #0x78
1000469e4:     	stur	q0, [sp, #0x78]
1000469e8:     	stur	q0, [sp, #0x88]
1000469ec:     	stp	q0, q0, [x22, #0x20]
1000469f0:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
1000469f4:     	bl	0x100069138 <_m34_container_begin>
1000469f8:     	add	x0, sp, #0x78
1000469fc:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100046a00:     	add	x0, sp, #0x60
100046a04:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100046a08:     	adrp	x26, 0x10020d000 <_scoop_gc_marker+0x800>
100046a0c:     	adrp	x10, 0x100208000 <dyld_stub_binder+0x100208000>
100046a10:     	add	x26, x26, #0x5b8
100046a14:     	ldr	x10, [x10, #0x88]
100046a18:     	ldr	x27, [x26]
100046a1c:     	ldr	x9, [x10, #0x18]
100046a20:     	mov	x0, x26
100046a24:     	blr	x27
100046a28:     	ldr	x8, [x0]
100046a2c:     	neg	x11, x9
100046a30:     	ldr	x12, [x8]
100046a34:     	add	x12, x9, x12
100046a38:     	sub	x12, x12, #0x1
100046a3c:     	ands	x21, x12, x11
100046a40:     	b.eq	0x100046a9c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x158>
100046a44:     	add	x12, x9, #0x1f
100046a48:     	and	x2, x12, x11
100046a4c:     	mov	w11, #0x7f80            ; =32640
100046a50:     	cmp	x2, x11
100046a54:     	b.hi	0x100046a9c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x158>
100046a58:     	ldr	x10, [x10, #0x90]
100046a5c:     	cbnz	x10, 0x100046a9c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x158>
100046a60:     	ldr	x11, [x8, #0x8]
100046a64:     	add	x10, x21, x2
100046a68:     	cmp	x10, x11
100046a6c:     	b.hi	0x100046a9c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x158>
100046a70:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
100046a74:     	add	x9, x9, x11
100046a78:     	cmn	x9, #0x21
100046a7c:     	b.hi	0x100046a9c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x158>
100046a80:     	str	x10, [x8]
100046a84:     	adrp	x1, 0x100208000 <dyld_stub_binder+0x100208000>
100046a88:     	mov	x0, x21
100046a8c:     	ldr	x1, [x1, #0x88]
100046a90:     	bl	0x100053da0 <_scoop_runtime_finish_tlab_alloc>
100046a94:     	mov	x0, x21
100046a98:     	b	0x100046aac <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x168>
100046a9c:     	adrp	x0, 0x100208000 <dyld_stub_binder+0x100208000>
100046aa0:     	mov	w1, #0x20               ; =32
100046aa4:     	ldr	x0, [x0, #0x88]
100046aa8:     	bl	0x100058388 <_scoop_runtime_alloc_slow>
100046aac:     	mov	x1, x20
100046ab0:     	str	x0, [sp, #0x20]
100046ab4:     	bl	0x10006abd8 <dyld_stub_binder+0x10006abd8>
100046ab8:     	ldr	x8, [sp, #0x20]
100046abc:     	movi.2d	v0, #0000000000000000
100046ac0:     	str	x8, [sp, #0x58]
100046ac4:     	adrp	x9, 0x10019e000 <_scoop$1$tr$a9467c0f11fc88d2e81930126eedc9e16609b6cf581081945137bd56ba6e5100+0x60>
100046ac8:     	add	x9, x9, #0x9f0
100046acc:     	str	x8, [sp, #0x50]
100046ad0:     	add	x8, sp, #0x50
100046ad4:     	add	x0, sp, #0xc8
100046ad8:     	add	x1, sp, #0xb8
100046adc:     	mov	w2, #0x1                ; =1
100046ae0:     	stp	x8, x9, [sp, #0xb8]
100046ae4:     	str	q0, [x22, #0x50]
100046ae8:     	str	xzr, [sp, #0xd8]
100046aec:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100046af0:     	movi.2d	v0, #0000000000000000
100046af4:     	add	x0, sp, #0xe0
100046af8:     	add	x1, sp, #0xe0
100046afc:     	stp	q0, q0, [sp, #0xe0]
100046b00:     	stp	q0, q0, [sp, #0x100]
100046b04:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100046b08:     	mov	w0, wzr
100046b0c:     	bl	0x1000691b0 <_m34_container_end>
100046b10:     	add	x0, sp, #0xe0
100046b14:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100046b18:     	ldr	x19, [sp, #0x50]
100046b1c:     	add	x0, sp, #0xc8
100046b20:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100046b24:     	adrp	x10, 0x10017f000 <_scoop$1$sr$c85fe09b5bacd3d7860263c6dd1ef05984e8fbc7f0cf96d0b4e70ee19ac755fb+0x90>
100046b28:     	mov	x0, x26
100046b2c:     	add	x10, x10, #0x500
100046b30:     	str	x19, [sp, #0x50]
100046b34:     	str	x19, [sp, #0x48]
100046b38:     	ldr	x9, [x10, #0x18]
100046b3c:     	blr	x27
100046b40:     	ldr	x8, [x0]
100046b44:     	neg	x11, x9
100046b48:     	str	x27, [sp, #0x8]
100046b4c:     	ldr	x12, [x8]
100046b50:     	add	x12, x9, x12
100046b54:     	sub	x12, x12, #0x1
100046b58:     	ands	x20, x12, x11
100046b5c:     	b.eq	0x100046bb4 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x270>
100046b60:     	add	x12, x9, #0x17
100046b64:     	and	x2, x12, x11
100046b68:     	mov	w11, #0x7f80            ; =32640
100046b6c:     	cmp	x2, x11
100046b70:     	b.hi	0x100046bb4 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x270>
100046b74:     	ldr	x10, [x10, #0x90]
100046b78:     	cbnz	x10, 0x100046bb4 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x270>
100046b7c:     	ldr	x11, [x8, #0x8]
100046b80:     	add	x10, x20, x2
100046b84:     	cmp	x10, x11
100046b88:     	b.hi	0x100046bb4 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x270>
100046b8c:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
100046b90:     	add	x9, x9, x11
100046b94:     	cmn	x9, #0x19
100046b98:     	b.hi	0x100046bb4 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x270>
100046b9c:     	str	x10, [x8]
100046ba0:     	adrp	x1, 0x10017f000 <_scoop$1$sr$c85fe09b5bacd3d7860263c6dd1ef05984e8fbc7f0cf96d0b4e70ee19ac755fb+0x90>
100046ba4:     	mov	x0, x20
100046ba8:     	add	x1, x1, #0x500
100046bac:     	bl	0x100053da0 <_scoop_runtime_finish_tlab_alloc>
100046bb0:     	b	0x100046bdc <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x298>
100046bb4:     	ldr	x8, [sp, #0x50]
100046bb8:     	adrp	x0, 0x10017f000 <_scoop$1$sr$c85fe09b5bacd3d7860263c6dd1ef05984e8fbc7f0cf96d0b4e70ee19ac755fb+0x90>
100046bbc:     	mov	w1, #0x18               ; =24
100046bc0:     	stp	x19, x8, [sp, #0x18]
100046bc4:     	add	x0, x0, #0x500
100046bc8:     	bl	0x100058388 <_scoop_runtime_alloc_slow>
100046bcc:     	ldp	x9, x8, [sp, #0x18]
100046bd0:     	mov	x20, x0
100046bd4:     	str	x8, [sp, #0x50]
100046bd8:     	str	x9, [sp, #0x48]
100046bdc:     	mov	w8, #0x7                ; =7
100046be0:     	mov	x1, x20
100046be4:     	str	w8, [x20, #0x10]
100046be8:     	ldp	x0, x8, [sp, #0x48]
100046bec:     	stp	x20, x0, [sp, #0x18]
100046bf0:     	str	x8, [sp, #0x10]
100046bf4:     	bl	0x10006aab8 <dyld_stub_binder+0x10006aab8>
100046bf8:     	ldp	x8, x9, [sp, #0x10]
100046bfc:     	ldr	x10, [sp, #0x20]
100046c00:     	str	x8, [sp, #0x50]
100046c04:     	str	x10, [sp, #0x48]
100046c08:     	movi.2d	v0, #0000000000000000
100046c0c:     	str	x9, [sp, #0x40]
100046c10:     	add	x21, sp, #0x50
100046c14:     	str	x8, [sp, #0x38]
100046c18:     	adrp	x8, 0x10019e000 <_scoop$1$tr$a9467c0f11fc88d2e81930126eedc9e16609b6cf581081945137bd56ba6e5100+0x60>
100046c1c:     	add	x8, x8, #0xa20
100046c20:     	stp	x21, x8, [x29, #-0xd0]
100046c24:     	add	x8, sp, #0x38
100046c28:     	adrp	x9, 0x10019e000 <_scoop$1$tr$a9467c0f11fc88d2e81930126eedc9e16609b6cf581081945137bd56ba6e5100+0x60>
100046c2c:     	add	x9, x9, #0xa30
100046c30:     	sub	x0, x29, #0xb0
100046c34:     	sub	x1, x29, #0xd0
100046c38:     	mov	w2, #0x2                ; =2
100046c3c:     	stp	x8, x9, [x29, #-0xc0]
100046c40:     	stur	q0, [x29, #-0xb0]
100046c44:     	stur	xzr, [x29, #-0xa0]
100046c48:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100046c4c:     	movi.2d	v0, #0000000000000000
100046c50:     	sub	x0, x29, #0x98
100046c54:     	sub	x1, x29, #0x98
100046c58:     	stp	q0, q0, [x22, #0x1b0]
100046c5c:     	stp	q0, q0, [x22, #0x1d0]
100046c60:     	bl	0x100058488 <_scoop_rt_enter_native_borrowed>
100046c64:     	ldr	x0, [sp, #0x38]
100046c68:     	bl	0x1000692ac <_m34_container_layout>
100046c6c:     	sub	x0, x29, #0x98
100046c70:     	bl	0x100063ad0 <_scoop_rt_leave_native_borrowed>
100046c74:     	ldr	x19, [sp, #0x50]
100046c78:     	ldr	x20, [sp, #0x38]
100046c7c:     	sub	x0, x29, #0xb0
100046c80:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100046c84:     	str	x19, [sp, #0x50]
100046c88:     	str	x20, [sp, #0x38]
100046c8c:     	str	x19, [sp, #0x20]
100046c90:     	bl	0x100058398 <_scoop_rt_gc_collect>
100046c94:     	ldr	x8, [sp, #0x20]
100046c98:     	mov	x20, xzr
100046c9c:     	mov	w27, wzr
100046ca0:     	adrp	x28, 0x10019e000 <_scoop$1$tr$a9467c0f11fc88d2e81930126eedc9e16609b6cf581081945137bd56ba6e5100+0x60>
100046ca4:     	add	x28, x28, #0xa00
100046ca8:     	adrp	x19, 0x10019e000 <_scoop$1$tr$a9467c0f11fc88d2e81930126eedc9e16609b6cf581081945137bd56ba6e5100+0x60>
100046cac:     	add	x19, x19, #0xa10
100046cb0:     	str	x8, [sp, #0x50]
100046cb4:     	ldar	x8, [x23]
100046cb8:     	ldar	w9, [x25]
100046cbc:     	add	x11, x24, #0x8
100046cc0:     	ldar	w10, [x24]
100046cc4:     	ldar	x11, [x11]
100046cc8:     	cmp	w9, #0x0
100046ccc:     	ccmp	w10, #0x1, #0x0, eq
100046cd0:     	ccmp	x11, x8, #0x0, eq
100046cd4:     	b.eq	0x100046cec <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x3a8>
100046cd8:     	ldr	x8, [sp, #0x50]
100046cdc:     	str	x8, [sp, #0x20]
100046ce0:     	bl	0x100058378 <_scoop_rt_safepoint>
100046ce4:     	ldr	x8, [sp, #0x20]
100046ce8:     	str	x8, [sp, #0x50]
100046cec:     	cmp	w27, #0x5
100046cf0:     	b.ge	0x100046de0 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x49c>
100046cf4:     	movi.2d	v0, #0000000000000000
100046cf8:     	add	x0, sp, #0x130
100046cfc:     	add	x1, sp, #0x120
100046d00:     	mov	w2, #0x1                ; =1
100046d04:     	stp	x21, x28, [sp, #0x120]
100046d08:     	str	xzr, [sp, #0x140]
100046d0c:     	str	q0, [sp, #0x130]
100046d10:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100046d14:     	movi.2d	v0, #0000000000000000
100046d18:     	add	x0, sp, #0x148
100046d1c:     	add	x1, sp, #0x148
100046d20:     	stp	q0, q0, [x22, #0xd0]
100046d24:     	stp	q0, q0, [x22, #0xf0]
100046d28:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100046d2c:     	bl	0x100069138 <_m34_container_begin>
100046d30:     	add	x0, sp, #0x148
100046d34:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100046d38:     	ldr	x26, [sp, #0x50]
100046d3c:     	add	x0, sp, #0x130
100046d40:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100046d44:     	str	x26, [sp, #0x50]
100046d48:     	str	x26, [sp, #0x20]
100046d4c:     	bl	0x100058398 <_scoop_rt_gc_collect>
100046d50:     	ldr	x8, [sp, #0x20]
100046d54:     	movi.2d	v0, #0000000000000000
100046d58:     	add	x0, sp, #0x198
100046d5c:     	add	x1, sp, #0x188
100046d60:     	mov	w2, #0x1                ; =1
100046d64:     	str	x8, [sp, #0x50]
100046d68:     	stp	x21, x19, [sp, #0x188]
100046d6c:     	str	xzr, [sp, #0x1a8]
100046d70:     	str	q0, [x22, #0x120]
100046d74:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100046d78:     	movi.2d	v0, #0000000000000000
100046d7c:     	add	x0, sp, #0x1b0
100046d80:     	add	x1, sp, #0x1b0
100046d84:     	stp	q0, q0, [sp, #0x1b0]
100046d88:     	stp	q0, q0, [sp, #0x1d0]
100046d8c:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100046d90:     	mov	w0, #0x4                ; =4
100046d94:     	bl	0x1000691b0 <_m34_container_end>
100046d98:     	add	x0, sp, #0x1b0
100046d9c:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100046da0:     	ldr	x26, [sp, #0x50]
100046da4:     	add	x0, sp, #0x198
100046da8:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100046dac:     	str	x26, [sp, #0x50]
100046db0:     	ldr	x8, [x26, #0x18]
100046db4:     	cmp	x8, #0x1
100046db8:     	b.lt	0x100046e10 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x4cc>
100046dbc:     	ldr	x8, [x26, #0x10]
100046dc0:     	ldr	x9, [x8, #0x10]
100046dc4:     	cmp	x9, #0x1
100046dc8:     	b.lt	0x100046e9c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x558>
100046dcc:     	ldr	x8, [x8, #0x18]
100046dd0:     	add	w27, w27, #0x1
100046dd4:     	ldrsw	x8, [x8, #0x10]
100046dd8:     	add	x20, x20, x8
100046ddc:     	b	0x100046cb4 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x370>
100046de0:     	cmp	x20, #0x23
100046de4:     	cset	w0, eq
100046de8:     	bl	0x10003ec6c <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
100046dec:     	mov	x0, x20
100046df0:     	add	sp, sp, #0x270
100046df4:     	ldp	x29, x30, [sp, #0x50]
100046df8:     	ldp	x20, x19, [sp, #0x40]
100046dfc:     	ldp	x22, x21, [sp, #0x30]
100046e00:     	ldp	x24, x23, [sp, #0x20]
100046e04:     	ldp	x26, x25, [sp, #0x10]
100046e08:     	ldp	x28, x27, [sp], #0x60
100046e0c:     	ret
100046e10:     	adrp	x10, 0x1000ab000 <_scoop$1$td$e1ee3b33d5a813f6151bf36485008d7f25b8c343eaf05ed9b3478985011481ed+0x70>
100046e14:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
100046e18:     	add	x10, x10, #0x490
100046e1c:     	ldr	x9, [x10, #0x18]
100046e20:     	add	x0, x0, #0x5b8
100046e24:     	ldr	x8, [sp, #0x8]
100046e28:     	blr	x8
100046e2c:     	ldr	x8, [x0]
100046e30:     	neg	x11, x9
100046e34:     	ldr	x12, [x8]
100046e38:     	add	x12, x9, x12
100046e3c:     	sub	x12, x12, #0x1
100046e40:     	ands	x19, x12, x11
100046e44:     	b.eq	0x100046f28 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5e4>
100046e48:     	add	x12, x9, #0x17
100046e4c:     	and	x2, x12, x11
100046e50:     	mov	w11, #0x7f80            ; =32640
100046e54:     	cmp	x2, x11
100046e58:     	b.hi	0x100046f28 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5e4>
100046e5c:     	ldr	x10, [x10, #0x90]
100046e60:     	cbnz	x10, 0x100046f28 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5e4>
100046e64:     	ldr	x11, [x8, #0x8]
100046e68:     	add	x10, x19, x2
100046e6c:     	cmp	x10, x11
100046e70:     	b.hi	0x100046f28 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5e4>
100046e74:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
100046e78:     	add	x9, x9, x11
100046e7c:     	cmn	x9, #0x18
100046e80:     	b.hs	0x100046f28 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5e4>
100046e84:     	str	x10, [x8]
100046e88:     	adrp	x1, 0x1000ab000 <_scoop$1$td$e1ee3b33d5a813f6151bf36485008d7f25b8c343eaf05ed9b3478985011481ed+0x70>
100046e8c:     	mov	x0, x19
100046e90:     	add	x1, x1, #0x490
100046e94:     	bl	0x100053da0 <_scoop_runtime_finish_tlab_alloc>
100046e98:     	b	0x100046f3c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5f8>
100046e9c:     	adrp	x10, 0x1000ab000 <_scoop$1$td$e1ee3b33d5a813f6151bf36485008d7f25b8c343eaf05ed9b3478985011481ed+0x70>
100046ea0:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
100046ea4:     	add	x10, x10, #0x490
100046ea8:     	ldr	x9, [x10, #0x18]
100046eac:     	add	x0, x0, #0x5b8
100046eb0:     	ldr	x8, [sp, #0x8]
100046eb4:     	blr	x8
100046eb8:     	ldr	x8, [x0]
100046ebc:     	neg	x11, x9
100046ec0:     	ldr	x12, [x8]
100046ec4:     	add	x12, x9, x12
100046ec8:     	sub	x12, x12, #0x1
100046ecc:     	ands	x19, x12, x11
100046ed0:     	b.eq	0x100046f54 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x610>
100046ed4:     	add	x12, x9, #0x17
100046ed8:     	and	x2, x12, x11
100046edc:     	mov	w11, #0x7f80            ; =32640
100046ee0:     	cmp	x2, x11
100046ee4:     	b.hi	0x100046f54 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x610>
100046ee8:     	ldr	x10, [x10, #0x90]
100046eec:     	cbnz	x10, 0x100046f54 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x610>
100046ef0:     	ldr	x11, [x8, #0x8]
100046ef4:     	add	x10, x19, x2
100046ef8:     	cmp	x10, x11
100046efc:     	b.hi	0x100046f54 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x610>
100046f00:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
100046f04:     	add	x9, x9, x11
100046f08:     	cmn	x9, #0x18
100046f0c:     	b.hs	0x100046f54 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x610>
100046f10:     	str	x10, [x8]
100046f14:     	adrp	x1, 0x1000ab000 <_scoop$1$td$e1ee3b33d5a813f6151bf36485008d7f25b8c343eaf05ed9b3478985011481ed+0x70>
100046f18:     	mov	x0, x19
100046f1c:     	add	x1, x1, #0x490
100046f20:     	bl	0x100053da0 <_scoop_runtime_finish_tlab_alloc>
100046f24:     	b	0x100046f68 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x624>
100046f28:     	adrp	x0, 0x1000ab000 <_scoop$1$td$e1ee3b33d5a813f6151bf36485008d7f25b8c343eaf05ed9b3478985011481ed+0x70>
100046f2c:     	mov	w1, #0x18               ; =24
100046f30:     	add	x0, x0, #0x490
100046f34:     	bl	0x100058388 <_scoop_runtime_alloc_slow>
100046f38:     	mov	x19, x0
100046f3c:     	mov	x0, x19
100046f40:     	str	x19, [sp, #0x20]
100046f44:     	bl	0x10002a018 <_scoop$1$cb$d88a9e35cab652ce2c186f206c545e4974be02e613922f4df03e263678dbd5aa>
100046f48:     	ldr	x0, [sp, #0x20]
100046f4c:     	str	x0, [sp, #0x28]
100046f50:     	bl	0x10005996c <_scoop_rt_throw>
100046f54:     	adrp	x0, 0x1000ab000 <_scoop$1$td$e1ee3b33d5a813f6151bf36485008d7f25b8c343eaf05ed9b3478985011481ed+0x70>
100046f58:     	mov	w1, #0x18               ; =24
100046f5c:     	add	x0, x0, #0x490
100046f60:     	bl	0x100058388 <_scoop_runtime_alloc_slow>
100046f64:     	mov	x19, x0
100046f68:     	mov	x0, x19
100046f6c:     	str	x19, [sp, #0x20]
100046f70:     	bl	0x10002a018 <_scoop$1$cb$d88a9e35cab652ce2c186f206c545e4974be02e613922f4df03e263678dbd5aa>
100046f74:     	ldr	x0, [sp, #0x20]
100046f78:     	str	x0, [sp, #0x30]
100046f7c:     	bl	0x10005996c <_scoop_rt_throw>

; buildStrings, MIR fn2, LIR scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-on-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010003a664 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc>:
10003a664:     	stp	x28, x27, [sp, #-0x60]!
10003a668:     	stp	x26, x25, [sp, #0x10]
10003a66c:     	stp	x24, x23, [sp, #0x20]
10003a670:     	stp	x22, x21, [sp, #0x30]
10003a674:     	stp	x20, x19, [sp, #0x40]
10003a678:     	stp	x29, x30, [sp, #0x50]
10003a67c:     	add	x29, sp, #0x50
10003a680:     	sub	sp, sp, #0x1f0
10003a684:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
10003a688:     	mov	x19, x0
10003a68c:     	add	x8, x8, #0x5d0
10003a690:     	ldr	x9, [x8]
10003a694:     	mov	x0, x8
10003a698:     	blr	x9
10003a69c:     	adrp	x22, 0x10020d000 <_scoop_gc_marker+0x800>
10003a6a0:     	adrp	x24, 0x10020d000 <_scoop_gc_marker+0x800>
10003a6a4:     	add	x22, x22, #0xa10
10003a6a8:     	ldr	x23, [x0]
10003a6ac:     	add	x24, x24, #0x9fc
10003a6b0:     	add	x10, x23, #0x8
10003a6b4:     	ldar	x8, [x22]
10003a6b8:     	ldar	w11, [x24]
10003a6bc:     	ldar	w9, [x23]
10003a6c0:     	ldar	x10, [x10]
10003a6c4:     	cbnz	w11, 0x10003a6d4 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x70>
10003a6c8:     	cmp	w9, #0x1
10003a6cc:     	ccmp	x10, x8, #0x0, eq
10003a6d0:     	b.eq	0x10003a6d8 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x74>
10003a6d4:     	bl	0x100058378 <_scoop_rt_safepoint>
10003a6d8:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
10003a6dc:     	adrp	x10, 0x1000a9000 <_scoop$1$td$2a8ab1be17d59e6ca9acf2df8e9c41925845047ee28b7bc6f84dfd9f616d60e1+0x50>
10003a6e0:     	add	x0, x0, #0x5b8
10003a6e4:     	add	x10, x10, #0xc30
10003a6e8:     	ldr	x8, [x0]
10003a6ec:     	ldr	x9, [x10, #0x18]
10003a6f0:     	blr	x8
10003a6f4:     	ldr	x8, [x0]
10003a6f8:     	neg	x11, x9
10003a6fc:     	ldr	x12, [x8]
10003a700:     	add	x12, x9, x12
10003a704:     	sub	x12, x12, #0x1
10003a708:     	ands	x20, x12, x11
10003a70c:     	b.eq	0x10003a768 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
10003a710:     	add	x12, x9, #0x17
10003a714:     	and	x2, x12, x11
10003a718:     	mov	w11, #0x7f80            ; =32640
10003a71c:     	cmp	x2, x11
10003a720:     	b.hi	0x10003a768 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
10003a724:     	ldr	x10, [x10, #0x90]
10003a728:     	cbnz	x10, 0x10003a768 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
10003a72c:     	ldr	x11, [x8, #0x8]
10003a730:     	add	x10, x20, x2
10003a734:     	cmp	x10, x11
10003a738:     	b.hi	0x10003a768 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
10003a73c:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10003a740:     	add	x9, x9, x11
10003a744:     	cmn	x9, #0x19
10003a748:     	b.hi	0x10003a768 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
10003a74c:     	str	x10, [x8]
10003a750:     	adrp	x1, 0x1000a9000 <_scoop$1$td$2a8ab1be17d59e6ca9acf2df8e9c41925845047ee28b7bc6f84dfd9f616d60e1+0x50>
10003a754:     	mov	x0, x20
10003a758:     	add	x1, x1, #0xc30
10003a75c:     	bl	0x100053da0 <_scoop_runtime_finish_tlab_alloc>
10003a760:     	mov	x0, x20
10003a764:     	b	0x10003a778 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x114>
10003a768:     	adrp	x0, 0x1000a9000 <_scoop$1$td$2a8ab1be17d59e6ca9acf2df8e9c41925845047ee28b7bc6f84dfd9f616d60e1+0x50>
10003a76c:     	mov	w1, #0x18               ; =24
10003a770:     	add	x0, x0, #0xc30
10003a774:     	bl	0x100058388 <_scoop_runtime_alloc_slow>
10003a778:     	add	x21, sp, #0xe0
10003a77c:     	str	x0, [sp, #0x10]
10003a780:     	bl	0x1000174ec <_scoop$1$cb$7fd0de94188adc1500f086a2b0e914cb1973f941c791324bcba453f6b7bbfd25>
10003a784:     	ldr	x8, [sp, #0x10]
10003a788:     	movi.2d	v0, #0000000000000000
10003a78c:     	str	x8, [sp, #0x48]
10003a790:     	adrp	x9, 0x10017a000 <_scoop$1$sr$4ce64baecb9311acd0135324a99db4c91e42d0c79fbc0e12f49445c5e243001c+0xc0>
10003a794:     	add	x9, x9, #0xf40
10003a798:     	str	x8, [sp, #0x40]
10003a79c:     	add	x8, sp, #0x40
10003a7a0:     	add	x0, sp, #0x60
10003a7a4:     	add	x1, sp, #0x50
10003a7a8:     	mov	w2, #0x1                ; =1
10003a7ac:     	stp	x8, x9, [sp, #0x50]
10003a7b0:     	str	q0, [sp, #0x60]
10003a7b4:     	str	xzr, [sp, #0x70]
10003a7b8:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003a7bc:     	movi.2d	v0, #0000000000000000
10003a7c0:     	add	x0, sp, #0x78
10003a7c4:     	add	x1, sp, #0x78
10003a7c8:     	stur	q0, [sp, #0x78]
10003a7cc:     	stur	q0, [sp, #0x88]
10003a7d0:     	stur	q0, [sp, #0x98]
10003a7d4:     	stur	q0, [sp, #0xa8]
10003a7d8:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10003a7dc:     	bl	0x100069138 <_m34_container_begin>
10003a7e0:     	add	x0, sp, #0x78
10003a7e4:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10003a7e8:     	ldr	x26, [sp, #0x40]
10003a7ec:     	add	x0, sp, #0x60
10003a7f0:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003a7f4:     	mov	x25, xzr
10003a7f8:     	mov	x20, xzr
10003a7fc:     	str	x26, [sp, #0x40]
10003a800:     	ldar	x8, [x22]
10003a804:     	ldar	w9, [x24]
10003a808:     	add	x11, x23, #0x8
10003a80c:     	ldar	w10, [x23]
10003a810:     	ldar	x11, [x11]
10003a814:     	cmp	w9, #0x0
10003a818:     	ccmp	w10, #0x1, #0x0, eq
10003a81c:     	ccmp	x11, x8, #0x0, eq
10003a820:     	b.eq	0x10003a838 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x1d4>
10003a824:     	ldr	x8, [sp, #0x40]
10003a828:     	str	x8, [sp, #0x10]
10003a82c:     	bl	0x100058378 <_scoop_rt_safepoint>
10003a830:     	ldr	x8, [sp, #0x10]
10003a834:     	str	x8, [sp, #0x40]
10003a838:     	cmp	x20, x19
10003a83c:     	b.ge	0x10003a8a4 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x240>
10003a840:     	ldr	x8, [sp, #0x40]
10003a844:     	mov	x0, x20
10003a848:     	str	x8, [sp, #0x10]
10003a84c:     	bl	0x10000a700 <_scoop$1$cb$ef730a437b587319fa729136c1c496c97d793c073731fe4cbfd67870ec8e2cc0>
10003a850:     	ldr	x8, [sp, #0x10]
10003a854:     	mov	x1, x0
10003a858:     	str	x8, [sp, #0x40]
10003a85c:     	stp	x8, x0, [sp, #0x20]
10003a860:     	str	x0, [sp, #0x18]
10003a864:     	str	x0, [sp, #0x8]
10003a868:     	mov	x0, x8
10003a86c:     	bl	0x100009540 <_scoop$1$cb$bf2a768a0cb1375f89a63ed498891d7c57dab520218a10fddd14a0197c926150>
10003a870:     	ldp	x0, x8, [sp, #0x8]
10003a874:     	str	x8, [sp, #0x40]
10003a878:     	str	x0, [sp, #0x28]
10003a87c:     	str	x8, [sp, #0x20]
10003a880:     	str	x0, [sp, #0x18]
10003a884:     	str	x8, [sp, #0x10]
10003a888:     	bl	0x100017e48 <_scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
10003a88c:     	ldp	x9, x8, [sp, #0x8]
10003a890:     	str	x8, [sp, #0x40]
10003a894:     	str	x9, [sp, #0x28]
10003a898:     	add	x25, x25, x0
10003a89c:     	add	x20, x20, #0x1
10003a8a0:     	b	0x10003a800 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x19c>
10003a8a4:     	movi.2d	v0, #0000000000000000
10003a8a8:     	add	x19, sp, #0x40
10003a8ac:     	adrp	x8, 0x10017a000 <_scoop$1$sr$4ce64baecb9311acd0135324a99db4c91e42d0c79fbc0e12f49445c5e243001c+0xc0>
10003a8b0:     	add	x8, x8, #0xf50
10003a8b4:     	add	x0, sp, #0xc8
10003a8b8:     	add	x1, sp, #0xb8
10003a8bc:     	mov	w2, #0x1                ; =1
10003a8c0:     	stp	x19, x8, [sp, #0xb8]
10003a8c4:     	str	xzr, [sp, #0xd8]
10003a8c8:     	stur	q0, [sp, #0xc8]
10003a8cc:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003a8d0:     	movi.2d	v0, #0000000000000000
10003a8d4:     	add	x0, sp, #0xe0
10003a8d8:     	add	x1, sp, #0xe0
10003a8dc:     	stp	q0, q0, [x21]
10003a8e0:     	stp	q0, q0, [x21, #0x20]
10003a8e4:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10003a8e8:     	mov	w0, #0x1                ; =1
10003a8ec:     	bl	0x1000691b0 <_m34_container_end>
10003a8f0:     	add	x0, sp, #0xe0
10003a8f4:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10003a8f8:     	ldr	x20, [sp, #0x40]
10003a8fc:     	add	x0, sp, #0xc8
10003a900:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003a904:     	movi.2d	v0, #0000000000000000
10003a908:     	adrp	x8, 0x10017a000 <_scoop$1$sr$4ce64baecb9311acd0135324a99db4c91e42d0c79fbc0e12f49445c5e243001c+0xc0>
10003a90c:     	add	x8, x8, #0xf60
10003a910:     	add	x0, sp, #0x130
10003a914:     	add	x1, sp, #0x120
10003a918:     	mov	w2, #0x1                ; =1
10003a91c:     	str	x20, [sp, #0x40]
10003a920:     	stp	x19, x8, [sp, #0x120]
10003a924:     	str	q0, [x21, #0x50]
10003a928:     	str	xzr, [sp, #0x140]
10003a92c:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003a930:     	movi.2d	v0, #0000000000000000
10003a934:     	sub	x0, x29, #0xf8
10003a938:     	sub	x1, x29, #0xf8
10003a93c:     	stur	q0, [x29, #-0xf8]
10003a940:     	stur	q0, [x29, #-0xe8]
10003a944:     	stur	q0, [x29, #-0xd8]
10003a948:     	stur	q0, [x29, #-0xc8]
10003a94c:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10003a950:     	bl	0x100069138 <_m34_container_begin>
10003a954:     	sub	x0, x29, #0xf8
10003a958:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10003a95c:     	ldr	x19, [sp, #0x40]
10003a960:     	add	x0, sp, #0x130
10003a964:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003a968:     	mov	x0, x19
10003a96c:     	str	x19, [sp, #0x40]
10003a970:     	str	x19, [sp, #0x10]
10003a974:     	bl	0x100003b40 <_scoop$1$cb$22ffc90119bb7e713d02ab7beca1a040fd100be545734e1a344db51e6165d4ce>
10003a978:     	ldr	x8, [sp, #0x10]
10003a97c:     	movi.2d	v0, #0000000000000000
10003a980:     	str	x8, [sp, #0x38]
10003a984:     	add	x8, sp, #0x30
10003a988:     	str	x0, [sp, #0x30]
10003a98c:     	adrp	x9, 0x10017a000 <_scoop$1$sr$4ce64baecb9311acd0135324a99db4c91e42d0c79fbc0e12f49445c5e243001c+0xc0>
10003a990:     	add	x9, x9, #0xf70
10003a994:     	sub	x0, x29, #0xa8
10003a998:     	sub	x1, x29, #0xb8
10003a99c:     	mov	w2, #0x1                ; =1
10003a9a0:     	stp	x8, x9, [x29, #-0xb8]
10003a9a4:     	stur	q0, [x29, #-0xa8]
10003a9a8:     	stur	xzr, [x29, #-0x98]
10003a9ac:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003a9b0:     	movi.2d	v0, #0000000000000000
10003a9b4:     	sub	x0, x29, #0x90
10003a9b8:     	sub	x1, x29, #0x90
10003a9bc:     	stp	q0, q0, [x21, #0xd0]
10003a9c0:     	stp	q0, q0, [x21, #0xf0]
10003a9c4:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10003a9c8:     	mov	w0, #0x2                ; =2
10003a9cc:     	bl	0x1000691b0 <_m34_container_end>
10003a9d0:     	sub	x0, x29, #0x90
10003a9d4:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10003a9d8:     	ldr	x19, [sp, #0x30]
10003a9dc:     	sub	x0, x29, #0xa8
10003a9e0:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003a9e4:     	mov	x0, x19
10003a9e8:     	str	x19, [sp, #0x30]
10003a9ec:     	str	x19, [sp, #0x10]
10003a9f0:     	bl	0x100017e48 <_scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
10003a9f4:     	ldr	x8, [sp, #0x10]
10003a9f8:     	cmp	x0, x25
10003a9fc:     	str	x8, [sp, #0x30]
10003aa00:     	cset	w0, eq
10003aa04:     	bl	0x10003ec6c <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
10003aa08:     	ldr	x0, [sp, #0x10]
10003aa0c:     	str	x0, [sp, #0x30]
10003aa10:     	bl	0x100017e48 <_scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
10003aa14:     	ldr	x8, [sp, #0x10]
10003aa18:     	str	x8, [sp, #0x30]
10003aa1c:     	add	sp, sp, #0x1f0
10003aa20:     	ldp	x29, x30, [sp, #0x50]
10003aa24:     	ldp	x20, x19, [sp, #0x40]
10003aa28:     	ldp	x22, x21, [sp, #0x30]
10003aa2c:     	ldp	x24, x23, [sp, #0x20]
10003aa30:     	ldp	x26, x25, [sp, #0x10]
10003aa34:     	ldp	x28, x27, [sp], #0x60
10003aa38:     	ret

; jsonRoundTrip, MIR fn3, LIR scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-on-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000417d0 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364>:
1000417d0:     	stp	x28, x27, [sp, #-0x60]!
1000417d4:     	stp	x26, x25, [sp, #0x10]
1000417d8:     	stp	x24, x23, [sp, #0x20]
1000417dc:     	stp	x22, x21, [sp, #0x30]
1000417e0:     	stp	x20, x19, [sp, #0x40]
1000417e4:     	stp	x29, x30, [sp, #0x50]
1000417e8:     	add	x29, sp, #0x50
1000417ec:     	sub	sp, sp, #0x3e0
1000417f0:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
1000417f4:     	mov	x19, x0
1000417f8:     	add	x8, x8, #0x5d0
1000417fc:     	ldr	x9, [x8]
100041800:     	mov	x0, x8
100041804:     	blr	x9
100041808:     	adrp	x22, 0x10020d000 <_scoop_gc_marker+0x800>
10004180c:     	adrp	x24, 0x10020d000 <_scoop_gc_marker+0x800>
100041810:     	add	x22, x22, #0xa10
100041814:     	ldr	x23, [x0]
100041818:     	add	x24, x24, #0x9fc
10004181c:     	add	x10, x23, #0x8
100041820:     	ldar	x8, [x22]
100041824:     	ldar	w11, [x24]
100041828:     	ldar	w9, [x23]
10004182c:     	ldar	x10, [x10]
100041830:     	cbnz	w11, 0x100041840 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x70>
100041834:     	cmp	w9, #0x1
100041838:     	ccmp	x10, x8, #0x0, eq
10004183c:     	b.eq	0x100041844 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x74>
100041840:     	bl	0x100058378 <_scoop_rt_safepoint>
100041844:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
100041848:     	adrp	x10, 0x100207000 <_scoop$1$bs$c50a1dc0e8b9295adba1796616b5ff27699caf40e937bbb0ef79ecdb0d61b685+0x50>
10004184c:     	add	x21, sp, #0x120
100041850:     	add	x0, x0, #0x5b8
100041854:     	ldr	x10, [x10, #0xfe8]
100041858:     	ldr	x8, [x0]
10004185c:     	ldr	x9, [x10, #0x18]
100041860:     	blr	x8
100041864:     	ldr	x8, [x0]
100041868:     	neg	x11, x9
10004186c:     	ldr	x12, [x8]
100041870:     	add	x12, x9, x12
100041874:     	sub	x12, x12, #0x1
100041878:     	ands	x20, x12, x11
10004187c:     	b.eq	0x1000418d8 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
100041880:     	add	x12, x9, #0x1f
100041884:     	and	x2, x12, x11
100041888:     	mov	w11, #0x7f80            ; =32640
10004188c:     	cmp	x2, x11
100041890:     	b.hi	0x1000418d8 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
100041894:     	ldr	x10, [x10, #0x90]
100041898:     	cbnz	x10, 0x1000418d8 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
10004189c:     	ldr	x11, [x8, #0x8]
1000418a0:     	add	x10, x20, x2
1000418a4:     	cmp	x10, x11
1000418a8:     	b.hi	0x1000418d8 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
1000418ac:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
1000418b0:     	add	x9, x9, x11
1000418b4:     	cmn	x9, #0x21
1000418b8:     	b.hi	0x1000418d8 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
1000418bc:     	str	x10, [x8]
1000418c0:     	adrp	x1, 0x100207000 <_scoop$1$bs$c50a1dc0e8b9295adba1796616b5ff27699caf40e937bbb0ef79ecdb0d61b685+0x50>
1000418c4:     	mov	x0, x20
1000418c8:     	ldr	x1, [x1, #0xfe8]
1000418cc:     	bl	0x100053da0 <_scoop_runtime_finish_tlab_alloc>
1000418d0:     	mov	x0, x20
1000418d4:     	b	0x1000418e8 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x118>
1000418d8:     	adrp	x0, 0x100207000 <_scoop$1$bs$c50a1dc0e8b9295adba1796616b5ff27699caf40e937bbb0ef79ecdb0d61b685+0x50>
1000418dc:     	mov	w1, #0x20               ; =32
1000418e0:     	ldr	x0, [x0, #0xfe8]
1000418e4:     	bl	0x100058388 <_scoop_runtime_alloc_slow>
1000418e8:     	mov	x1, x19
1000418ec:     	str	x0, [sp, #0x28]
1000418f0:     	bl	0x10006ad88 <dyld_stub_binder+0x10006ad88>
1000418f4:     	ldr	x8, [sp, #0x28]
1000418f8:     	movi.2d	v0, #0000000000000000
1000418fc:     	str	x8, [sp, #0xe0]
100041900:     	add	x20, sp, #0xd8
100041904:     	stp	x8, x8, [sp, #0xd0]
100041908:     	adrp	x8, 0x100190000 <_scoop$1$sr$2503881459123331efc0a3dadae77f63c3de117828ec8f825ee55c13454ad883+0x10>
10004190c:     	add	x8, x8, #0xd00
100041910:     	stp	x20, x8, [sp, #0xe8]
100041914:     	add	x8, sp, #0xd0
100041918:     	adrp	x9, 0x100190000 <_scoop$1$sr$2503881459123331efc0a3dadae77f63c3de117828ec8f825ee55c13454ad883+0x10>
10004191c:     	add	x9, x9, #0xd10
100041920:     	add	x0, sp, #0x108
100041924:     	add	x1, sp, #0xe8
100041928:     	stp	x8, x9, [sp, #0xf8]
10004192c:     	add	x8, sp, #0x9
100041930:     	mov	w2, #0x2                ; =2
100041934:     	stur	q0, [x8, #0xff]
100041938:     	str	xzr, [sp, #0x118]
10004193c:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100041940:     	movi.2d	v0, #0000000000000000
100041944:     	add	x0, sp, #0x120
100041948:     	add	x1, sp, #0x120
10004194c:     	stp	q0, q0, [x21]
100041950:     	stp	q0, q0, [x21, #0x20]
100041954:     	bl	0x100058488 <_scoop_rt_enter_native_borrowed>
100041958:     	ldr	x0, [sp, #0xd0]
10004195c:     	bl	0x1000692ac <_m34_container_layout>
100041960:     	add	x0, sp, #0x120
100041964:     	bl	0x100063ad0 <_scoop_rt_leave_native_borrowed>
100041968:     	ldr	x25, [sp, #0xd8]
10004196c:     	ldr	x26, [sp, #0xd0]
100041970:     	add	x0, sp, #0x108
100041974:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100041978:     	movi.2d	v0, #0000000000000000
10004197c:     	adrp	x8, 0x100190000 <_scoop$1$sr$2503881459123331efc0a3dadae77f63c3de117828ec8f825ee55c13454ad883+0x10>
100041980:     	add	x8, x8, #0xd20
100041984:     	add	x0, sp, #0x170
100041988:     	add	x1, sp, #0x160
10004198c:     	mov	w2, #0x1                ; =1
100041990:     	str	x25, [sp, #0xd8]
100041994:     	str	x26, [sp, #0xd0]
100041998:     	stp	x20, x8, [sp, #0x160]
10004199c:     	str	q0, [x21, #0x50]
1000419a0:     	str	xzr, [sp, #0x180]
1000419a4:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
1000419a8:     	movi.2d	v0, #0000000000000000
1000419ac:     	add	x8, sp, #0x89
1000419b0:     	add	x0, sp, #0x188
1000419b4:     	add	x1, sp, #0x188
1000419b8:     	stur	q0, [x8, #0xff]
1000419bc:     	add	x8, sp, #0x99
1000419c0:     	stur	q0, [x8, #0xff]
1000419c4:     	add	x8, sp, #0xa9
1000419c8:     	stur	q0, [x8, #0xff]
1000419cc:     	add	x8, sp, #0xb9
1000419d0:     	stur	q0, [x8, #0xff]
1000419d4:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
1000419d8:     	bl	0x100069138 <_m34_container_begin>
1000419dc:     	add	x0, sp, #0x188
1000419e0:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
1000419e4:     	ldr	x25, [sp, #0xd8]
1000419e8:     	add	x0, sp, #0x170
1000419ec:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
1000419f0:     	mov	x20, xzr
1000419f4:     	str	x25, [sp, #0xd8]
1000419f8:     	ldar	x8, [x22]
1000419fc:     	ldar	w9, [x24]
100041a00:     	add	x11, x23, #0x8
100041a04:     	ldar	w10, [x23]
100041a08:     	ldar	x11, [x11]
100041a0c:     	cmp	w9, #0x0
100041a10:     	ccmp	w10, #0x1, #0x0, eq
100041a14:     	ccmp	x11, x8, #0x0, eq
100041a18:     	b.eq	0x100041a30 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x260>
100041a1c:     	ldr	x8, [sp, #0xd8]
100041a20:     	str	x8, [sp, #0x28]
100041a24:     	bl	0x100058378 <_scoop_rt_safepoint>
100041a28:     	ldr	x8, [sp, #0x28]
100041a2c:     	str	x8, [sp, #0xd8]
100041a30:     	cmp	x20, x19
100041a34:     	b.ge	0x100041a60 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x290>
100041a38:     	ldr	x0, [sp, #0xd8]
100041a3c:     	mov	w1, w20
100041a40:     	str	x0, [sp, #0x38]
100041a44:     	str	x0, [sp, #0x28]
100041a48:     	bl	0x10006ab78 <dyld_stub_binder+0x10006ab78>
100041a4c:     	ldr	x8, [sp, #0x28]
100041a50:     	str	x8, [sp, #0xd8]
100041a54:     	str	x8, [sp, #0x38]
100041a58:     	add	x20, x20, #0x1
100041a5c:     	b	0x1000419f8 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x228>
100041a60:     	movi.2d	v0, #0000000000000000
100041a64:     	add	x20, sp, #0xd8
100041a68:     	adrp	x8, 0x100190000 <_scoop$1$sr$2503881459123331efc0a3dadae77f63c3de117828ec8f825ee55c13454ad883+0x10>
100041a6c:     	add	x8, x8, #0xd30
100041a70:     	add	x0, sp, #0x1d8
100041a74:     	add	x1, sp, #0x1c8
100041a78:     	stp	x20, x8, [sp, #0x1c8]
100041a7c:     	add	x8, sp, #0xd9
100041a80:     	mov	w2, #0x1                ; =1
100041a84:     	str	xzr, [sp, #0x1e8]
100041a88:     	stur	q0, [x8, #0xff]
100041a8c:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100041a90:     	movi.2d	v0, #0000000000000000
100041a94:     	add	x0, sp, #0x1f0
100041a98:     	add	x1, sp, #0x1f0
100041a9c:     	stp	q0, q0, [x21, #0xd0]
100041aa0:     	stp	q0, q0, [x21, #0xf0]
100041aa4:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100041aa8:     	mov	w0, #0x1                ; =1
100041aac:     	bl	0x1000691b0 <_m34_container_end>
100041ab0:     	add	x0, sp, #0x1f0
100041ab4:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100041ab8:     	ldr	x22, [sp, #0xd8]
100041abc:     	add	x0, sp, #0x1d8
100041ac0:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100041ac4:     	movi.2d	v0, #0000000000000000
100041ac8:     	adrp	x8, 0x100190000 <_scoop$1$sr$2503881459123331efc0a3dadae77f63c3de117828ec8f825ee55c13454ad883+0x10>
100041acc:     	add	x8, x8, #0xd40
100041ad0:     	add	x0, sp, #0x240
100041ad4:     	add	x1, sp, #0x230
100041ad8:     	mov	w2, #0x1                ; =1
100041adc:     	str	x22, [sp, #0xd8]
100041ae0:     	str	x20, [sp, #0x230]
100041ae4:     	str	x8, [sp, #0x238]
100041ae8:     	str	q0, [x21, #0x120]
100041aec:     	str	xzr, [sp, #0x250]
100041af0:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100041af4:     	movi.2d	v0, #0000000000000000
100041af8:     	add	x8, sp, #0x159
100041afc:     	add	x0, sp, #0x258
100041b00:     	add	x1, sp, #0x258
100041b04:     	stur	q0, [x8, #0xff]
100041b08:     	add	x8, sp, #0x169
100041b0c:     	stur	q0, [x8, #0xff]
100041b10:     	add	x8, sp, #0x179
100041b14:     	stur	q0, [x8, #0xff]
100041b18:     	add	x8, sp, #0x189
100041b1c:     	stur	q0, [x8, #0xff]
100041b20:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100041b24:     	bl	0x100069138 <_m34_container_begin>
100041b28:     	add	x0, sp, #0x258
100041b2c:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100041b30:     	ldr	x20, [sp, #0xd8]
100041b34:     	add	x0, sp, #0x240
100041b38:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100041b3c:     	str	x20, [sp, #0xd8]
100041b40:     	str	x20, [sp, #0x28]
100041b44:     	bl	0x1000384b8 <_scoop$1$cb$0bded537c1012e7dc164ae08cf779ffaa1fbf40bc9bfe0e762282e5e75f642ac>
100041b48:     	ldr	x8, [sp, #0x28]
100041b4c:     	adrp	x25, 0x10020d000 <_scoop_gc_marker+0x800>
100041b50:     	add	x25, x25, #0x6f0
100041b54:     	str	x8, [sp, #0xd8]
100041b58:     	ldr	x8, [x25]
100041b5c:     	str	x8, [sp, #0x20]
100041b60:     	bl	0x10006aae8 <dyld_stub_binder+0x10006aae8>
100041b64:     	ldp	x8, x9, [sp, #0x20]
100041b68:     	str	x8, [sp, #0xc8]
100041b6c:     	adrp	x24, 0x100207000 <_scoop$1$bs$c50a1dc0e8b9295adba1796616b5ff27699caf40e937bbb0ef79ecdb0d61b685+0x50>
100041b70:     	ldr	x24, [x24, #0xf90]
100041b74:     	str	x9, [sp, #0xc0]
100041b78:     	ldr	x8, [x24]
100041b7c:     	str	x8, [sp, #0x18]
100041b80:     	bl	0x10000e5e0 <_scoop$1$cb$5e012350ddc7cb238741da77cd1768afd61bf09fc92c1eb080f58c2c066df7f3>
100041b84:     	ldp	x8, x10, [sp, #0x18]
100041b88:     	ldr	x9, [sp, #0x28]
100041b8c:     	str	x8, [sp, #0xb8]
100041b90:     	str	x10, [sp, #0xc8]
100041b94:     	adrp	x22, 0x10020d000 <_scoop_gc_marker+0x800>
100041b98:     	adrp	x23, 0x1000a9000 <_scoop$1$td$2a8ab1be17d59e6ca9acf2df8e9c41925845047ee28b7bc6f84dfd9f616d60e1+0x50>
100041b9c:     	add	x22, x22, #0x628
100041ba0:     	add	x23, x23, #0xf50
100041ba4:     	str	x9, [sp, #0xc0]
100041ba8:     	ldr	x8, [x22]
100041bac:     	ldr	x9, [x23, #0x60]
100041bb0:     	ldr	x2, [x9, #0x18]
100041bb4:     	str	x8, [sp, #0xb0]
100041bb8:     	mov	x8, x10
100041bbc:     	ldr	x1, [sp, #0xb0]
100041bc0:     	ldp	x0, x9, [sp, #0xb8]
100041bc4:     	stp	x1, x0, [sp, #0x20]
100041bc8:     	stp	x9, x8, [sp, #0x10]
100041bcc:     	bl	0x10006acd4 <dyld_stub_binder+0x10006acd4>
100041bd0:     	ldp	x11, x9, [sp, #0x20]
100041bd4:     	mov	x3, x1
100041bd8:     	ldp	x8, x10, [sp, #0x10]
100041bdc:     	str	x9, [sp, #0xb8]
100041be0:     	str	x11, [sp, #0xa8]
100041be4:     	str	x10, [sp, #0xc8]
100041be8:     	str	x8, [sp, #0xc0]
100041bec:     	mov	x1, x8
100041bf0:     	str	x0, [sp, #0xa0]
100041bf4:     	mov	x0, x10
100041bf8:     	ldr	x2, [sp, #0xa0]
100041bfc:     	stp	x2, x0, [sp, #0x20]
100041c00:     	bl	0x10006ad10 <dyld_stub_binder+0x10006ad10>
100041c04:     	ldp	x9, x8, [sp, #0x20]
100041c08:     	ldr	x10, [sp, #0x10]
100041c0c:     	str	x8, [sp, #0xc8]
100041c10:     	str	x10, [sp, #0x98]
100041c14:     	movi.2d	v0, #0000000000000000
100041c18:     	adrp	x8, 0x100190000 <_scoop$1$sr$2503881459123331efc0a3dadae77f63c3de117828ec8f825ee55c13454ad883+0x10>
100041c1c:     	add	x8, x8, #0xd50
100041c20:     	str	x9, [sp, #0x90]
100041c24:     	add	x20, sp, #0x88
100041c28:     	add	x1, sp, #0x298
100041c2c:     	str	x0, [sp, #0x88]
100041c30:     	add	x0, sp, #0x2a8
100041c34:     	mov	w2, #0x1                ; =1
100041c38:     	str	x8, [sp, #0x2a0]
100041c3c:     	add	x8, sp, #0x1a9
100041c40:     	str	x20, [sp, #0x298]
100041c44:     	stur	q0, [x8, #0xff]
100041c48:     	str	xzr, [sp, #0x2b8]
100041c4c:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100041c50:     	movi.2d	v0, #0000000000000000
100041c54:     	add	x0, sp, #0x2c0
100041c58:     	add	x1, sp, #0x2c0
100041c5c:     	stp	q0, q0, [x21, #0x1a0]
100041c60:     	stp	q0, q0, [x21, #0x1c0]
100041c64:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100041c68:     	mov	w0, #0x2                ; =2
100041c6c:     	bl	0x1000691b0 <_m34_container_end>
100041c70:     	add	x0, sp, #0x2c0
100041c74:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100041c78:     	ldr	x26, [sp, #0x88]
100041c7c:     	add	x0, sp, #0x2a8
100041c80:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100041c84:     	movi.2d	v0, #0000000000000000
100041c88:     	adrp	x8, 0x100190000 <_scoop$1$sr$2503881459123331efc0a3dadae77f63c3de117828ec8f825ee55c13454ad883+0x10>
100041c8c:     	add	x8, x8, #0xd60
100041c90:     	add	x0, sp, #0x310
100041c94:     	add	x1, sp, #0x300
100041c98:     	mov	w2, #0x1                ; =1
100041c9c:     	str	x26, [sp, #0x88]
100041ca0:     	str	x20, [sp, #0x300]
100041ca4:     	str	x8, [sp, #0x308]
100041ca8:     	str	q0, [x21, #0x1f0]
100041cac:     	str	xzr, [sp, #0x320]
100041cb0:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100041cb4:     	movi.2d	v0, #0000000000000000
100041cb8:     	add	x8, sp, #0x229
100041cbc:     	add	x0, sp, #0x328
100041cc0:     	add	x1, sp, #0x328
100041cc4:     	stur	q0, [x8, #0xff]
100041cc8:     	add	x8, sp, #0x239
100041ccc:     	stur	q0, [x8, #0xff]
100041cd0:     	add	x8, sp, #0x249
100041cd4:     	stur	q0, [x8, #0xff]
100041cd8:     	add	x8, sp, #0x259
100041cdc:     	stur	q0, [x8, #0xff]
100041ce0:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100041ce4:     	bl	0x100069138 <_m34_container_begin>
100041ce8:     	add	x0, sp, #0x328
100041cec:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100041cf0:     	ldr	x26, [sp, #0x88]
100041cf4:     	add	x0, sp, #0x310
100041cf8:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100041cfc:     	str	x26, [sp, #0x88]
100041d00:     	str	x26, [sp, #0x28]
100041d04:     	bl	0x1000384b8 <_scoop$1$cb$0bded537c1012e7dc164ae08cf779ffaa1fbf40bc9bfe0e762282e5e75f642ac>
100041d08:     	ldr	x8, [sp, #0x28]
100041d0c:     	str	x8, [sp, #0x88]
100041d10:     	ldr	x8, [x25]
100041d14:     	str	x8, [sp, #0x20]
100041d18:     	bl	0x10006aae8 <dyld_stub_binder+0x10006aae8>
100041d1c:     	ldp	x9, x8, [sp, #0x20]
100041d20:     	str	x8, [sp, #0x88]
100041d24:     	str	x9, [sp, #0x80]
100041d28:     	str	x8, [sp, #0x78]
100041d2c:     	ldr	x9, [x24]
100041d30:     	stp	x9, x8, [sp, #0x10]
100041d34:     	bl	0x10000e5e0 <_scoop$1$cb$5e012350ddc7cb238741da77cd1768afd61bf09fc92c1eb080f58c2c066df7f3>
100041d38:     	ldp	x8, x10, [sp, #0x18]
100041d3c:     	ldr	x9, [sp, #0x28]
100041d40:     	ldr	x11, [sp, #0x10]
100041d44:     	str	x8, [sp, #0x88]
100041d48:     	str	x11, [sp, #0x70]
100041d4c:     	str	x10, [sp, #0x80]
100041d50:     	str	x9, [sp, #0x78]
100041d54:     	mov	x0, x11
100041d58:     	ldr	x8, [x22]
100041d5c:     	ldr	x9, [x23, #0x60]
100041d60:     	ldr	x2, [x9, #0x8]
100041d64:     	str	x8, [sp, #0x68]
100041d68:     	mov	x9, x10
100041d6c:     	ldr	x1, [sp, #0x68]
100041d70:     	ldr	x8, [sp, #0x88]
100041d74:     	ldr	x10, [sp, #0x78]
100041d78:     	stp	x0, x8, [sp, #0x20]
100041d7c:     	stp	x10, x9, [sp, #0x8]
100041d80:     	str	x1, [sp, #0x18]
100041d84:     	bl	0x10006acbc <dyld_stub_binder+0x10006acbc>
100041d88:     	ldp	x12, x9, [sp, #0x20]
100041d8c:     	mov	x3, x1
100041d90:     	ldp	x8, x10, [sp, #0x8]
100041d94:     	ldr	x11, [sp, #0x18]
100041d98:     	str	x9, [sp, #0x88]
100041d9c:     	str	x12, [sp, #0x70]
100041da0:     	str	x11, [sp, #0x60]
100041da4:     	str	x10, [sp, #0x80]
100041da8:     	str	x8, [sp, #0x78]
100041dac:     	mov	x1, x8
100041db0:     	str	x0, [sp, #0x58]
100041db4:     	mov	x0, x10
100041db8:     	ldr	x2, [sp, #0x58]
100041dbc:     	stp	x0, x9, [sp, #0x20]
100041dc0:     	str	x2, [sp, #0x18]
100041dc4:     	bl	0x10006aaac <dyld_stub_binder+0x10006aaac>
100041dc8:     	ldp	x11, x8, [sp, #0x20]
100041dcc:     	ldr	x9, [sp, #0x18]
100041dd0:     	ldr	x10, [sp, #0x8]
100041dd4:     	str	x8, [sp, #0x88]
100041dd8:     	str	x11, [sp, #0x80]
100041ddc:     	str	x10, [sp, #0x50]
100041de0:     	movi.2d	v0, #0000000000000000
100041de4:     	adrp	x8, 0x100190000 <_scoop$1$sr$2503881459123331efc0a3dadae77f63c3de117828ec8f825ee55c13454ad883+0x10>
100041de8:     	add	x8, x8, #0xd70
100041dec:     	str	x9, [sp, #0x48]
100041df0:     	adrp	x9, 0x100190000 <_scoop$1$sr$2503881459123331efc0a3dadae77f63c3de117828ec8f825ee55c13454ad883+0x10>
100041df4:     	add	x9, x9, #0xd80
100041df8:     	str	x0, [sp, #0x40]
100041dfc:     	sub	x0, x29, #0xa8
100041e00:     	sub	x1, x29, #0xc8
100041e04:     	stp	x20, x8, [x29, #-0xc8]
100041e08:     	add	x8, sp, #0x40
100041e0c:     	mov	w2, #0x2                ; =2
100041e10:     	stp	x8, x9, [x29, #-0xb8]
100041e14:     	stur	q0, [x29, #-0xa8]
100041e18:     	stur	xzr, [x29, #-0x98]
100041e1c:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100041e20:     	movi.2d	v0, #0000000000000000
100041e24:     	sub	x0, x29, #0x90
100041e28:     	sub	x1, x29, #0x90
100041e2c:     	stp	q0, q0, [x21, #0x280]
100041e30:     	stp	q0, q0, [x21, #0x2a0]
100041e34:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100041e38:     	mov	w0, #0x3                ; =3
100041e3c:     	bl	0x1000691b0 <_m34_container_end>
100041e40:     	sub	x0, x29, #0x90
100041e44:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100041e48:     	ldr	x21, [sp, #0x88]
100041e4c:     	ldr	x20, [sp, #0x40]
100041e50:     	sub	x0, x29, #0xa8
100041e54:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100041e58:     	str	x21, [sp, #0x88]
100041e5c:     	str	x20, [sp, #0x40]
100041e60:     	ldr	x8, [x20, #0x18]
100041e64:     	cmp	x8, x19
100041e68:     	b.ne	0x100041ea0 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x6d0>
100041e6c:     	ldr	x8, [sp, #0x88]
100041e70:     	sub	x1, x19, #0x1
100041e74:     	mov	x0, x20
100041e78:     	stp	x20, x8, [sp, #0x20]
100041e7c:     	bl	0x10006ad1c <dyld_stub_binder+0x10006ad1c>
100041e80:     	ldp	x9, x8, [sp, #0x20]
100041e84:     	str	x8, [sp, #0x88]
100041e88:     	str	x9, [sp, #0x40]
100041e8c:     	sub	w8, w19, #0x1
100041e90:     	str	x9, [sp, #0x30]
100041e94:     	cmp	w0, w8
100041e98:     	cset	w0, eq
100041e9c:     	b	0x100041ea4 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x6d4>
100041ea0:     	mov	w0, wzr
100041ea4:     	ldr	x8, [sp, #0x88]
100041ea8:     	ldr	x9, [sp, #0x40]
100041eac:     	stp	x9, x8, [sp, #0x20]
100041eb0:     	bl	0x10003ec6c <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
100041eb4:     	ldp	x8, x0, [sp, #0x20]
100041eb8:     	str	x0, [sp, #0x88]
100041ebc:     	str	x8, [sp, #0x40]
100041ec0:     	bl	0x100017e48 <_scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
100041ec4:     	ldp	x9, x8, [sp, #0x20]
100041ec8:     	str	x8, [sp, #0x88]
100041ecc:     	str	x9, [sp, #0x40]
100041ed0:     	ldr	x8, [x9, #0x18]
100041ed4:     	add	x0, x0, x8
100041ed8:     	add	sp, sp, #0x3e0
100041edc:     	ldp	x29, x30, [sp, #0x50]
100041ee0:     	ldp	x20, x19, [sp, #0x40]
100041ee4:     	ldp	x22, x21, [sp, #0x30]
100041ee8:     	ldp	x24, x23, [sp, #0x20]
100041eec:     	ldp	x26, x25, [sp, #0x10]
100041ef0:     	ldp	x28, x27, [sp], #0x60
100041ef4:     	ret

; scalarList, MIR fn4, LIR scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-on-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100048518 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b>:
100048518:     	stp	x28, x27, [sp, #-0x60]!
10004851c:     	stp	x26, x25, [sp, #0x10]
100048520:     	stp	x24, x23, [sp, #0x20]
100048524:     	stp	x22, x21, [sp, #0x30]
100048528:     	stp	x20, x19, [sp, #0x40]
10004852c:     	stp	x29, x30, [sp, #0x50]
100048530:     	add	x29, sp, #0x50
100048534:     	sub	sp, sp, #0x430
100048538:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
10004853c:     	mov	x19, x0
100048540:     	add	x8, x8, #0x5d0
100048544:     	ldr	x9, [x8]
100048548:     	mov	x0, x8
10004854c:     	blr	x9
100048550:     	adrp	x23, 0x10020d000 <_scoop_gc_marker+0x800>
100048554:     	adrp	x25, 0x10020d000 <_scoop_gc_marker+0x800>
100048558:     	add	x23, x23, #0xa10
10004855c:     	ldr	x24, [x0]
100048560:     	add	x25, x25, #0x9fc
100048564:     	add	x10, x24, #0x8
100048568:     	ldar	x8, [x23]
10004856c:     	ldar	w11, [x25]
100048570:     	ldar	w9, [x24]
100048574:     	ldar	x10, [x10]
100048578:     	cbnz	w11, 0x100048588 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x70>
10004857c:     	cmp	w9, #0x1
100048580:     	ccmp	x10, x8, #0x0, eq
100048584:     	b.eq	0x10004858c <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x74>
100048588:     	bl	0x100058378 <_scoop_rt_safepoint>
10004858c:     	movi.2d	v0, #0000000000000000
100048590:     	add	x0, sp, #0x80
100048594:     	mov	x1, xzr
100048598:     	mov	x2, xzr
10004859c:     	add	x26, sp, #0x100
1000485a0:     	str	xzr, [sp, #0x90]
1000485a4:     	str	q0, [sp, #0x80]
1000485a8:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
1000485ac:     	movi.2d	v0, #0000000000000000
1000485b0:     	add	x0, sp, #0x98
1000485b4:     	add	x1, sp, #0x98
1000485b8:     	stur	q0, [sp, #0x98]
1000485bc:     	stur	q0, [sp, #0xa8]
1000485c0:     	stur	q0, [sp, #0xb8]
1000485c4:     	stur	q0, [sp, #0xc8]
1000485c8:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
1000485cc:     	bl	0x100069138 <_m34_container_begin>
1000485d0:     	add	x0, sp, #0x98
1000485d4:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
1000485d8:     	add	x0, sp, #0x80
1000485dc:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
1000485e0:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
1000485e4:     	adrp	x10, 0x100207000 <_scoop$1$bs$c50a1dc0e8b9295adba1796616b5ff27699caf40e937bbb0ef79ecdb0d61b685+0x50>
1000485e8:     	add	x0, x0, #0x5b8
1000485ec:     	ldr	x10, [x10, #0xfe8]
1000485f0:     	ldr	x8, [x0]
1000485f4:     	ldr	x9, [x10, #0x18]
1000485f8:     	blr	x8
1000485fc:     	ldr	x8, [x0]
100048600:     	neg	x11, x9
100048604:     	ldr	x12, [x8]
100048608:     	add	x12, x9, x12
10004860c:     	sub	x12, x12, #0x1
100048610:     	ands	x20, x12, x11
100048614:     	b.eq	0x100048670 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x158>
100048618:     	add	x12, x9, #0x1f
10004861c:     	and	x2, x12, x11
100048620:     	mov	w11, #0x7f80            ; =32640
100048624:     	cmp	x2, x11
100048628:     	b.hi	0x100048670 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x158>
10004862c:     	ldr	x10, [x10, #0x90]
100048630:     	cbnz	x10, 0x100048670 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x158>
100048634:     	ldr	x11, [x8, #0x8]
100048638:     	add	x10, x20, x2
10004863c:     	cmp	x10, x11
100048640:     	b.hi	0x100048670 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x158>
100048644:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
100048648:     	add	x9, x9, x11
10004864c:     	cmn	x9, #0x21
100048650:     	b.hi	0x100048670 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x158>
100048654:     	str	x10, [x8]
100048658:     	adrp	x1, 0x100207000 <_scoop$1$bs$c50a1dc0e8b9295adba1796616b5ff27699caf40e937bbb0ef79ecdb0d61b685+0x50>
10004865c:     	mov	x0, x20
100048660:     	ldr	x1, [x1, #0xfe8]
100048664:     	bl	0x100053da0 <_scoop_runtime_finish_tlab_alloc>
100048668:     	mov	x0, x20
10004866c:     	b	0x100048680 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x168>
100048670:     	adrp	x0, 0x100207000 <_scoop$1$bs$c50a1dc0e8b9295adba1796616b5ff27699caf40e937bbb0ef79ecdb0d61b685+0x50>
100048674:     	mov	w1, #0x20               ; =32
100048678:     	ldr	x0, [x0, #0xfe8]
10004867c:     	bl	0x100058388 <_scoop_runtime_alloc_slow>
100048680:     	mov	x1, x19
100048684:     	str	x0, [sp, #0x18]
100048688:     	bl	0x10006ad88 <dyld_stub_binder+0x10006ad88>
10004868c:     	ldr	x8, [sp, #0x18]
100048690:     	movi.2d	v0, #0000000000000000
100048694:     	str	x8, [sp, #0x78]
100048698:     	add	x20, sp, #0x70
10004869c:     	str	x8, [sp, #0x70]
1000486a0:     	adrp	x8, 0x1001a4000 <_scoop$1$sr$92b4f4998c0aff05fffeaaf8f0fd7a4db1f375520da3f41b5363663c377951aa+0x50>
1000486a4:     	add	x8, x8, #0x130
1000486a8:     	add	x0, sp, #0xe8
1000486ac:     	add	x1, sp, #0xd8
1000486b0:     	mov	w2, #0x1                ; =1
1000486b4:     	stp	x20, x8, [sp, #0xd8]
1000486b8:     	stur	q0, [sp, #0xe8]
1000486bc:     	str	xzr, [sp, #0xf8]
1000486c0:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
1000486c4:     	movi.2d	v0, #0000000000000000
1000486c8:     	add	x0, sp, #0x100
1000486cc:     	add	x1, sp, #0x100
1000486d0:     	stp	q0, q0, [x26]
1000486d4:     	stp	q0, q0, [x26, #0x20]
1000486d8:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
1000486dc:     	mov	w0, wzr
1000486e0:     	bl	0x1000691b0 <_m34_container_end>
1000486e4:     	add	x0, sp, #0x100
1000486e8:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
1000486ec:     	ldr	x21, [sp, #0x70]
1000486f0:     	add	x0, sp, #0xe8
1000486f4:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
1000486f8:     	movi.2d	v0, #0000000000000000
1000486fc:     	adrp	x8, 0x1001a4000 <_scoop$1$sr$92b4f4998c0aff05fffeaaf8f0fd7a4db1f375520da3f41b5363663c377951aa+0x50>
100048700:     	add	x8, x8, #0x140
100048704:     	str	x21, [sp, #0x70]
100048708:     	adrp	x9, 0x1001a4000 <_scoop$1$sr$92b4f4998c0aff05fffeaaf8f0fd7a4db1f375520da3f41b5363663c377951aa+0x50>
10004870c:     	add	x9, x9, #0x150
100048710:     	stp	x20, x8, [sp, #0x140]
100048714:     	add	x8, sp, #0x68
100048718:     	add	x0, sp, #0x160
10004871c:     	add	x1, sp, #0x140
100048720:     	mov	w2, #0x2                ; =2
100048724:     	str	x21, [sp, #0x68]
100048728:     	stp	x8, x9, [sp, #0x150]
10004872c:     	str	q0, [x26, #0x60]
100048730:     	str	xzr, [sp, #0x170]
100048734:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100048738:     	movi.2d	v0, #0000000000000000
10004873c:     	add	x8, sp, #0x79
100048740:     	add	x0, sp, #0x178
100048744:     	add	x1, sp, #0x178
100048748:     	stur	q0, [x8, #0xff]
10004874c:     	add	x8, sp, #0x89
100048750:     	stur	q0, [x8, #0xff]
100048754:     	add	x8, sp, #0x99
100048758:     	stur	q0, [x8, #0xff]
10004875c:     	add	x8, sp, #0xa9
100048760:     	stur	q0, [x8, #0xff]
100048764:     	bl	0x100058488 <_scoop_rt_enter_native_borrowed>
100048768:     	ldr	x0, [sp, #0x68]
10004876c:     	bl	0x1000692ac <_m34_container_layout>
100048770:     	add	x0, sp, #0x178
100048774:     	bl	0x100063ad0 <_scoop_rt_leave_native_borrowed>
100048778:     	ldr	x21, [sp, #0x70]
10004877c:     	ldr	x22, [sp, #0x68]
100048780:     	add	x0, sp, #0x160
100048784:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100048788:     	movi.2d	v0, #0000000000000000
10004878c:     	adrp	x8, 0x1001a4000 <_scoop$1$sr$92b4f4998c0aff05fffeaaf8f0fd7a4db1f375520da3f41b5363663c377951aa+0x50>
100048790:     	add	x8, x8, #0x160
100048794:     	str	x21, [sp, #0x70]
100048798:     	add	x0, sp, #0x1c8
10004879c:     	add	x1, sp, #0x1b8
1000487a0:     	str	x22, [sp, #0x68]
1000487a4:     	mov	w2, #0x1                ; =1
1000487a8:     	stp	x20, x8, [sp, #0x1b8]
1000487ac:     	add	x8, sp, #0xc9
1000487b0:     	stur	q0, [x8, #0xff]
1000487b4:     	str	xzr, [sp, #0x1d8]
1000487b8:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
1000487bc:     	movi.2d	v0, #0000000000000000
1000487c0:     	add	x0, sp, #0x1e0
1000487c4:     	add	x1, sp, #0x1e0
1000487c8:     	stp	q0, q0, [x26, #0xe0]
1000487cc:     	stp	q0, q0, [x26, #0x100]
1000487d0:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
1000487d4:     	bl	0x100069138 <_m34_container_begin>
1000487d8:     	add	x0, sp, #0x1e0
1000487dc:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
1000487e0:     	ldr	x21, [sp, #0x70]
1000487e4:     	add	x0, sp, #0x1c8
1000487e8:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
1000487ec:     	str	x21, [sp, #0x70]
1000487f0:     	mov	x21, #0x51d1            ; =20945
1000487f4:     	mov	x28, #0xa8e9            ; =43241
1000487f8:     	movk	x21, #0x2f81, lsl #16
1000487fc:     	movk	x28, #0x17c0, lsl #16
100048800:     	mov	x26, xzr
100048804:     	movk	x21, #0x7eae, lsl #32
100048808:     	movk	x28, #0x3f57, lsl #32
10004880c:     	mov	x20, xzr
100048810:     	movk	x21, #0x51d0, lsl #48
100048814:     	mov	w27, #0x61              ; =97
100048818:     	movk	x28, #0xa8e8, lsl #48
10004881c:     	add	x9, x24, #0x8
100048820:     	ldar	x8, [x23]
100048824:     	ldar	w11, [x25]
100048828:     	ldar	w10, [x24]
10004882c:     	ldar	x9, [x9]
100048830:     	cbnz	w11, 0x100048844 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x32c>
100048834:     	cmp	w10, #0x1
100048838:     	b.ne	0x100048844 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x32c>
10004883c:     	cmp	x9, x8
100048840:     	b.eq	0x100048858 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x340>
100048844:     	ldr	x8, [sp, #0x70]
100048848:     	str	x8, [sp, #0x18]
10004884c:     	bl	0x100058378 <_scoop_rt_safepoint>
100048850:     	ldr	x8, [sp, #0x18]
100048854:     	str	x8, [sp, #0x70]
100048858:     	cmp	x20, x19
10004885c:     	b.ge	0x1000488b0 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x398>
100048860:     	umulh	x9, x20, x21
100048864:     	ldr	x0, [sp, #0x70]
100048868:     	smulh	x8, x20, x28
10004886c:     	str	x0, [sp, #0x60]
100048870:     	str	x0, [sp, #0x18]
100048874:     	sub	x11, x20, x9
100048878:     	add	x9, x9, x11, lsr #1
10004887c:     	add	x8, x8, x20
100048880:     	lsr	x9, x9, #6
100048884:     	asr	x10, x8, #6
100048888:     	msub	w1, w9, w27, w20
10004888c:     	add	x8, x10, x8, lsr #63
100048890:     	msub	x22, x8, x27, x20
100048894:     	bl	0x10006ab78 <dyld_stub_binder+0x10006ab78>
100048898:     	ldr	x8, [sp, #0x18]
10004889c:     	str	x8, [sp, #0x70]
1000488a0:     	add	x26, x26, x22
1000488a4:     	add	x20, x20, #0x1
1000488a8:     	str	x8, [sp, #0x60]
1000488ac:     	b	0x10004881c <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x304>
1000488b0:     	movi.2d	v0, #0000000000000000
1000488b4:     	add	x20, sp, #0x70
1000488b8:     	adrp	x8, 0x1001a4000 <_scoop$1$sr$92b4f4998c0aff05fffeaaf8f0fd7a4db1f375520da3f41b5363663c377951aa+0x50>
1000488bc:     	add	x8, x8, #0x170
1000488c0:     	add	x22, sp, #0x100
1000488c4:     	add	x0, sp, #0x230
1000488c8:     	add	x1, sp, #0x220
1000488cc:     	mov	w2, #0x1                ; =1
1000488d0:     	str	x20, [sp, #0x220]
1000488d4:     	str	x8, [sp, #0x228]
1000488d8:     	str	q0, [x22, #0x130]
1000488dc:     	str	xzr, [sp, #0x240]
1000488e0:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
1000488e4:     	movi.2d	v0, #0000000000000000
1000488e8:     	add	x8, sp, #0x149
1000488ec:     	add	x0, sp, #0x248
1000488f0:     	add	x1, sp, #0x248
1000488f4:     	stur	q0, [x8, #0xff]
1000488f8:     	add	x8, sp, #0x159
1000488fc:     	stur	q0, [x8, #0xff]
100048900:     	add	x8, sp, #0x169
100048904:     	stur	q0, [x8, #0xff]
100048908:     	add	x8, sp, #0x179
10004890c:     	stur	q0, [x8, #0xff]
100048910:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100048914:     	mov	w0, #0x1                ; =1
100048918:     	bl	0x1000691b0 <_m34_container_end>
10004891c:     	add	x0, sp, #0x248
100048920:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100048924:     	ldr	x21, [sp, #0x70]
100048928:     	add	x0, sp, #0x230
10004892c:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100048930:     	movi.2d	v0, #0000000000000000
100048934:     	adrp	x8, 0x1001a4000 <_scoop$1$sr$92b4f4998c0aff05fffeaaf8f0fd7a4db1f375520da3f41b5363663c377951aa+0x50>
100048938:     	add	x8, x8, #0x180
10004893c:     	str	x21, [sp, #0x70]
100048940:     	add	x0, sp, #0x298
100048944:     	add	x1, sp, #0x288
100048948:     	str	x8, [sp, #0x290]
10004894c:     	add	x8, sp, #0x199
100048950:     	mov	w2, #0x1                ; =1
100048954:     	str	x20, [sp, #0x288]
100048958:     	stur	q0, [x8, #0xff]
10004895c:     	str	xzr, [sp, #0x2a8]
100048960:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100048964:     	movi.2d	v0, #0000000000000000
100048968:     	add	x0, sp, #0x2b0
10004896c:     	add	x1, sp, #0x2b0
100048970:     	stp	q0, q0, [x22, #0x1b0]
100048974:     	stp	q0, q0, [x22, #0x1d0]
100048978:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10004897c:     	bl	0x100069138 <_m34_container_begin>
100048980:     	add	x0, sp, #0x2b0
100048984:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100048988:     	ldr	x27, [sp, #0x70]
10004898c:     	add	x0, sp, #0x298
100048990:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100048994:     	mov	x20, xzr
100048998:     	mov	x21, xzr
10004899c:     	str	x27, [sp, #0x70]
1000489a0:     	ldar	x8, [x23]
1000489a4:     	ldar	w9, [x25]
1000489a8:     	add	x11, x24, #0x8
1000489ac:     	ldar	w10, [x24]
1000489b0:     	ldar	x11, [x11]
1000489b4:     	cmp	w9, #0x0
1000489b8:     	ccmp	w10, #0x1, #0x0, eq
1000489bc:     	ccmp	x11, x8, #0x0, eq
1000489c0:     	b.eq	0x1000489d8 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x4c0>
1000489c4:     	ldr	x8, [sp, #0x70]
1000489c8:     	str	x8, [sp, #0x18]
1000489cc:     	bl	0x100058378 <_scoop_rt_safepoint>
1000489d0:     	ldr	x8, [sp, #0x18]
1000489d4:     	str	x8, [sp, #0x70]
1000489d8:     	cmp	x21, x19
1000489dc:     	b.ge	0x100048ac0 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x5a8>
1000489e0:     	ldr	x0, [sp, #0x70]
1000489e4:     	mov	x1, x21
1000489e8:     	str	x0, [sp, #0x50]
1000489ec:     	str	x0, [sp, #0x28]
1000489f0:     	stp	x0, x0, [sp, #0x10]
1000489f4:     	bl	0x10006acf8 <dyld_stub_binder+0x10006acf8>
1000489f8:     	ldp	x9, x8, [sp, #0x10]
1000489fc:     	str	x8, [sp, #0x70]
100048a00:     	str	x9, [sp, #0x50]
100048a04:     	str	x9, [sp, #0x28]
100048a08:     	mov	x1, x21
100048a0c:     	ldr	x0, [x9, #0x10]
100048a10:     	stp	x8, x0, [sp, #0x18]
100048a14:     	str	x0, [sp, #0x10]
100048a18:     	bl	0x10006abf0 <dyld_stub_binder+0x10006abf0>
100048a1c:     	ldp	x9, x8, [sp, #0x10]
100048a20:     	str	x8, [sp, #0x70]
100048a24:     	add	x20, x20, w0, sxtw
100048a28:     	mov	x0, x8
100048a2c:     	mov	x1, x21
100048a30:     	str	x9, [sp, #0x20]
100048a34:     	stp	x8, x8, [sp, #0x40]
100048a38:     	str	x8, [sp, #0x38]
100048a3c:     	stp	x8, x8, [sp, #0x10]
100048a40:     	str	x8, [sp, #0x8]
100048a44:     	bl	0x10006acf8 <dyld_stub_binder+0x10006acf8>
100048a48:     	ldp	x10, x8, [sp, #0x10]
100048a4c:     	ldr	x9, [sp, #0x8]
100048a50:     	str	x8, [sp, #0x70]
100048a54:     	str	x10, [sp, #0x48]
100048a58:     	str	x9, [sp, #0x40]
100048a5c:     	str	x9, [sp, #0x38]
100048a60:     	mov	x1, x21
100048a64:     	ldr	x0, [x9, #0x10]
100048a68:     	mov	x9, x10
100048a6c:     	stp	x9, x8, [sp, #0x10]
100048a70:     	str	x0, [sp, #0x30]
100048a74:     	str	x0, [sp, #0x8]
100048a78:     	bl	0x10006abf0 <dyld_stub_binder+0x10006abf0>
100048a7c:     	ldp	x10, x8, [sp, #0x10]
100048a80:     	ldr	x9, [sp, #0x8]
100048a84:     	str	x8, [sp, #0x70]
100048a88:     	str	x10, [sp, #0x48]
100048a8c:     	str	x9, [sp, #0x30]
100048a90:     	mov	x9, x8
100048a94:     	mov	x8, x10
100048a98:     	add	w2, w0, #0x1
100048a9c:     	mov	x0, x8
100048aa0:     	mov	x1, x21
100048aa4:     	stp	x8, x9, [sp, #0x10]
100048aa8:     	bl	0x10006ace0 <dyld_stub_binder+0x10006ace0>
100048aac:     	ldp	x9, x8, [sp, #0x10]
100048ab0:     	str	x8, [sp, #0x70]
100048ab4:     	str	x9, [sp, #0x48]
100048ab8:     	add	x21, x21, #0x1
100048abc:     	b	0x1000489a0 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x488>
100048ac0:     	movi.2d	v0, #0000000000000000
100048ac4:     	add	x21, sp, #0x70
100048ac8:     	adrp	x8, 0x1001a4000 <_scoop$1$sr$92b4f4998c0aff05fffeaaf8f0fd7a4db1f375520da3f41b5363663c377951aa+0x50>
100048acc:     	add	x8, x8, #0x190
100048ad0:     	add	x0, sp, #0x300
100048ad4:     	add	x1, sp, #0x2f0
100048ad8:     	mov	w2, #0x1                ; =1
100048adc:     	str	x21, [sp, #0x2f0]
100048ae0:     	str	x8, [sp, #0x2f8]
100048ae4:     	str	q0, [x22, #0x200]
100048ae8:     	str	xzr, [sp, #0x310]
100048aec:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100048af0:     	movi.2d	v0, #0000000000000000
100048af4:     	add	x8, sp, #0x219
100048af8:     	add	x0, sp, #0x318
100048afc:     	add	x1, sp, #0x318
100048b00:     	stur	q0, [x8, #0xff]
100048b04:     	add	x8, sp, #0x229
100048b08:     	stur	q0, [x8, #0xff]
100048b0c:     	add	x8, sp, #0x239
100048b10:     	stur	q0, [x8, #0xff]
100048b14:     	add	x8, sp, #0x249
100048b18:     	stur	q0, [x8, #0xff]
100048b1c:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100048b20:     	mov	w0, #0x2                ; =2
100048b24:     	bl	0x1000691b0 <_m34_container_end>
100048b28:     	add	x0, sp, #0x318
100048b2c:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100048b30:     	ldr	x19, [sp, #0x70]
100048b34:     	add	x0, sp, #0x300
100048b38:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100048b3c:     	cmp	x20, x26
100048b40:     	str	x19, [sp, #0x70]
100048b44:     	cset	w0, eq
100048b48:     	str	x19, [sp, #0x18]
100048b4c:     	bl	0x10003ec6c <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
100048b50:     	ldr	x8, [sp, #0x18]
100048b54:     	movi.2d	v0, #0000000000000000
100048b58:     	str	x8, [sp, #0x70]
100048b5c:     	adrp	x8, 0x1001a4000 <_scoop$1$sr$92b4f4998c0aff05fffeaaf8f0fd7a4db1f375520da3f41b5363663c377951aa+0x50>
100048b60:     	add	x8, x8, #0x1a0
100048b64:     	add	x0, sp, #0x368
100048b68:     	add	x1, sp, #0x358
100048b6c:     	str	x8, [sp, #0x360]
100048b70:     	add	x8, sp, #0x269
100048b74:     	mov	w2, #0x1                ; =1
100048b78:     	str	x21, [sp, #0x358]
100048b7c:     	stur	q0, [x8, #0xff]
100048b80:     	str	xzr, [sp, #0x378]
100048b84:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100048b88:     	movi.2d	v0, #0000000000000000
100048b8c:     	sub	x0, x29, #0x100
100048b90:     	sub	x1, x29, #0x100
100048b94:     	stp	q0, q0, [x22, #0x280]
100048b98:     	stp	q0, q0, [x22, #0x2a0]
100048b9c:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100048ba0:     	bl	0x100069138 <_m34_container_begin>
100048ba4:     	sub	x0, x29, #0x100
100048ba8:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100048bac:     	ldr	x19, [sp, #0x70]
100048bb0:     	add	x0, sp, #0x368
100048bb4:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100048bb8:     	mov	x0, x19
100048bbc:     	str	x19, [sp, #0x70]
100048bc0:     	str	x19, [sp, #0x18]
100048bc4:     	bl	0x10006aadc <dyld_stub_binder+0x10006aadc>
100048bc8:     	ldr	x8, [sp, #0x18]
100048bcc:     	str	x8, [sp, #0x70]
100048bd0:     	movi.2d	v0, #0000000000000000
100048bd4:     	str	x8, [sp, #0x58]
100048bd8:     	adrp	x8, 0x1001a4000 <_scoop$1$sr$92b4f4998c0aff05fffeaaf8f0fd7a4db1f375520da3f41b5363663c377951aa+0x50>
100048bdc:     	add	x8, x8, #0x1b0
100048be0:     	sub	x0, x29, #0xb0
100048be4:     	sub	x1, x29, #0xc0
100048be8:     	mov	w2, #0x1                ; =1
100048bec:     	stp	x21, x8, [x29, #-0xc0]
100048bf0:     	stur	xzr, [x29, #-0xa0]
100048bf4:     	str	q0, [x22, #0x2d0]
100048bf8:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100048bfc:     	movi.2d	v0, #0000000000000000
100048c00:     	sub	x0, x29, #0x98
100048c04:     	sub	x1, x29, #0x98
100048c08:     	stur	q0, [x29, #-0x98]
100048c0c:     	stur	q0, [x29, #-0x88]
100048c10:     	stur	q0, [x29, #-0x78]
100048c14:     	stur	q0, [x29, #-0x68]
100048c18:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100048c1c:     	mov	w0, #0x3                ; =3
100048c20:     	bl	0x1000691b0 <_m34_container_end>
100048c24:     	sub	x0, x29, #0x98
100048c28:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100048c2c:     	ldr	x19, [sp, #0x70]
100048c30:     	sub	x0, x29, #0xb0
100048c34:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100048c38:     	str	x19, [sp, #0x70]
100048c3c:     	ldr	x8, [x19, #0x18]
100048c40:     	cmp	x8, #0x0
100048c44:     	cset	w0, eq
100048c48:     	bl	0x10003ec6c <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
100048c4c:     	mov	x0, x20
100048c50:     	add	sp, sp, #0x430
100048c54:     	ldp	x29, x30, [sp, #0x50]
100048c58:     	ldp	x20, x19, [sp, #0x40]
100048c5c:     	ldp	x22, x21, [sp, #0x30]
100048c60:     	ldp	x24, x23, [sp, #0x20]
100048c64:     	ldp	x26, x25, [sp, #0x10]
100048c68:     	ldp	x28, x27, [sp], #0x60
100048c6c:     	ret

; interfaceList, MIR fn5, LIR scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-on-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000493d8 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c>:
1000493d8:     	stp	x28, x27, [sp, #-0x60]!
1000493dc:     	stp	x26, x25, [sp, #0x10]
1000493e0:     	stp	x24, x23, [sp, #0x20]
1000493e4:     	stp	x22, x21, [sp, #0x30]
1000493e8:     	stp	x20, x19, [sp, #0x40]
1000493ec:     	stp	x29, x30, [sp, #0x50]
1000493f0:     	add	x29, sp, #0x50
1000493f4:     	sub	sp, sp, #0x450
1000493f8:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
1000493fc:     	mov	x19, x0
100049400:     	add	x23, sp, #0xa0
100049404:     	add	x8, x8, #0x5d0
100049408:     	ldr	x9, [x8]
10004940c:     	mov	x0, x8
100049410:     	blr	x9
100049414:     	adrp	x24, 0x10020d000 <_scoop_gc_marker+0x800>
100049418:     	adrp	x26, 0x10020d000 <_scoop_gc_marker+0x800>
10004941c:     	add	x24, x24, #0xa10
100049420:     	ldr	x25, [x0]
100049424:     	add	x26, x26, #0x9fc
100049428:     	str	xzr, [sp, #0x98]
10004942c:     	add	x10, x25, #0x8
100049430:     	ldar	x8, [x24]
100049434:     	ldar	w11, [x26]
100049438:     	ldar	w9, [x25]
10004943c:     	ldar	x10, [x10]
100049440:     	cbnz	w11, 0x100049450 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x78>
100049444:     	cmp	w9, #0x1
100049448:     	ccmp	x10, x8, #0x0, eq
10004944c:     	b.eq	0x100049454 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x7c>
100049450:     	bl	0x100058378 <_scoop_rt_safepoint>
100049454:     	movi.2d	v0, #0000000000000000
100049458:     	add	x0, sp, #0xa0
10004945c:     	mov	x1, xzr
100049460:     	mov	x2, xzr
100049464:     	str	xzr, [sp, #0xb0]
100049468:     	str	q0, [x23]
10004946c:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100049470:     	movi.2d	v0, #0000000000000000
100049474:     	add	x0, sp, #0xb8
100049478:     	add	x1, sp, #0xb8
10004947c:     	stur	q0, [sp, #0xb8]
100049480:     	stur	q0, [sp, #0xc8]
100049484:     	stur	q0, [sp, #0xd8]
100049488:     	stur	q0, [sp, #0xe8]
10004948c:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100049490:     	bl	0x100069138 <_m34_container_begin>
100049494:     	add	x0, sp, #0xb8
100049498:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10004949c:     	add	x0, sp, #0xa0
1000494a0:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
1000494a4:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
1000494a8:     	adrp	x10, 0x100208000 <dyld_stub_binder+0x100208000>
1000494ac:     	add	x0, x0, #0x5b8
1000494b0:     	ldr	x10, [x10, #0xa0]
1000494b4:     	ldr	x27, [x0]
1000494b8:     	ldr	x9, [x10, #0x18]
1000494bc:     	blr	x27
1000494c0:     	ldr	x8, [x0]
1000494c4:     	neg	x11, x9
1000494c8:     	ldr	x12, [x8]
1000494cc:     	add	x12, x9, x12
1000494d0:     	sub	x12, x12, #0x1
1000494d4:     	ands	x21, x12, x11
1000494d8:     	b.eq	0x100049534 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x15c>
1000494dc:     	add	x12, x9, #0x1f
1000494e0:     	and	x2, x12, x11
1000494e4:     	mov	w11, #0x7f80            ; =32640
1000494e8:     	cmp	x2, x11
1000494ec:     	b.hi	0x100049534 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x15c>
1000494f0:     	ldr	x10, [x10, #0x90]
1000494f4:     	cbnz	x10, 0x100049534 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x15c>
1000494f8:     	ldr	x11, [x8, #0x8]
1000494fc:     	add	x10, x21, x2
100049500:     	cmp	x10, x11
100049504:     	b.hi	0x100049534 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x15c>
100049508:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10004950c:     	add	x9, x9, x11
100049510:     	cmn	x9, #0x21
100049514:     	b.hi	0x100049534 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x15c>
100049518:     	str	x10, [x8]
10004951c:     	adrp	x1, 0x100208000 <dyld_stub_binder+0x100208000>
100049520:     	mov	x0, x21
100049524:     	ldr	x1, [x1, #0xa0]
100049528:     	bl	0x100053da0 <_scoop_runtime_finish_tlab_alloc>
10004952c:     	mov	x0, x21
100049530:     	b	0x100049544 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x16c>
100049534:     	adrp	x0, 0x100208000 <dyld_stub_binder+0x100208000>
100049538:     	mov	w1, #0x20               ; =32
10004953c:     	ldr	x0, [x0, #0xa0]
100049540:     	bl	0x100058388 <_scoop_runtime_alloc_slow>
100049544:     	mov	x1, x19
100049548:     	str	x0, [sp, #0x18]
10004954c:     	bl	0x10006ab6c <dyld_stub_binder+0x10006ab6c>
100049550:     	ldr	x8, [sp, #0x18]
100049554:     	movi.2d	v0, #0000000000000000
100049558:     	str	x8, [sp, #0x90]
10004955c:     	add	x20, sp, #0x88
100049560:     	str	x8, [sp, #0x88]
100049564:     	adrp	x8, 0x1001a6000 <_scoop$1$sr$1b87c2119cd87b2a3a24bfe10a2b6f9ca5227429405a54625917b3b05d41613d+0x60>
100049568:     	add	x8, x8, #0x940
10004956c:     	stp	x20, x8, [sp, #0xf8]
100049570:     	add	x8, sp, #0x9
100049574:     	add	x0, sp, #0x108
100049578:     	add	x1, sp, #0xf8
10004957c:     	mov	w2, #0x1                ; =1
100049580:     	str	xzr, [sp, #0x118]
100049584:     	stur	q0, [x8, #0xff]
100049588:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10004958c:     	movi.2d	v0, #0000000000000000
100049590:     	add	x0, sp, #0x120
100049594:     	add	x1, sp, #0x120
100049598:     	stp	q0, q0, [x23, #0x80]
10004959c:     	stp	q0, q0, [x23, #0xa0]
1000495a0:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
1000495a4:     	mov	w0, wzr
1000495a8:     	bl	0x1000691b0 <_m34_container_end>
1000495ac:     	add	x0, sp, #0x120
1000495b0:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
1000495b4:     	ldr	x21, [sp, #0x88]
1000495b8:     	add	x0, sp, #0x108
1000495bc:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
1000495c0:     	movi.2d	v0, #0000000000000000
1000495c4:     	adrp	x8, 0x1001a6000 <_scoop$1$sr$1b87c2119cd87b2a3a24bfe10a2b6f9ca5227429405a54625917b3b05d41613d+0x60>
1000495c8:     	add	x8, x8, #0x950
1000495cc:     	str	x21, [sp, #0x88]
1000495d0:     	adrp	x9, 0x1001a6000 <_scoop$1$sr$1b87c2119cd87b2a3a24bfe10a2b6f9ca5227429405a54625917b3b05d41613d+0x60>
1000495d4:     	add	x9, x9, #0x960
1000495d8:     	stp	x20, x8, [sp, #0x160]
1000495dc:     	add	x8, sp, #0x80
1000495e0:     	add	x0, sp, #0x180
1000495e4:     	add	x1, sp, #0x160
1000495e8:     	mov	w2, #0x2                ; =2
1000495ec:     	str	x21, [sp, #0x80]
1000495f0:     	stp	x8, x9, [sp, #0x170]
1000495f4:     	str	q0, [x23, #0xe0]
1000495f8:     	str	xzr, [sp, #0x190]
1000495fc:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100049600:     	movi.2d	v0, #0000000000000000
100049604:     	add	x8, sp, #0x99
100049608:     	add	x0, sp, #0x198
10004960c:     	add	x1, sp, #0x198
100049610:     	stur	q0, [x8, #0xff]
100049614:     	add	x8, sp, #0xa9
100049618:     	stur	q0, [x8, #0xff]
10004961c:     	add	x8, sp, #0xb9
100049620:     	stur	q0, [x8, #0xff]
100049624:     	add	x8, sp, #0xc9
100049628:     	stur	q0, [x8, #0xff]
10004962c:     	bl	0x100058488 <_scoop_rt_enter_native_borrowed>
100049630:     	ldr	x0, [sp, #0x80]
100049634:     	bl	0x1000692ac <_m34_container_layout>
100049638:     	add	x0, sp, #0x198
10004963c:     	bl	0x100063ad0 <_scoop_rt_leave_native_borrowed>
100049640:     	ldr	x21, [sp, #0x88]
100049644:     	ldr	x22, [sp, #0x80]
100049648:     	add	x0, sp, #0x180
10004964c:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100049650:     	movi.2d	v0, #0000000000000000
100049654:     	adrp	x8, 0x1001a6000 <_scoop$1$sr$1b87c2119cd87b2a3a24bfe10a2b6f9ca5227429405a54625917b3b05d41613d+0x60>
100049658:     	add	x8, x8, #0x970
10004965c:     	str	x21, [sp, #0x88]
100049660:     	add	x0, sp, #0x1e8
100049664:     	add	x1, sp, #0x1d8
100049668:     	str	x22, [sp, #0x80]
10004966c:     	mov	w2, #0x1                ; =1
100049670:     	stp	x20, x8, [sp, #0x1d8]
100049674:     	add	x8, sp, #0xe9
100049678:     	stur	q0, [x8, #0xff]
10004967c:     	str	xzr, [sp, #0x1f8]
100049680:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100049684:     	movi.2d	v0, #0000000000000000
100049688:     	add	x0, sp, #0x200
10004968c:     	add	x1, sp, #0x200
100049690:     	stp	q0, q0, [x23, #0x160]
100049694:     	stp	q0, q0, [x23, #0x180]
100049698:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10004969c:     	bl	0x100069138 <_m34_container_begin>
1000496a0:     	add	x0, sp, #0x200
1000496a4:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
1000496a8:     	ldr	x20, [sp, #0x88]
1000496ac:     	add	x0, sp, #0x1e8
1000496b0:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
1000496b4:     	adrp	x21, 0x10017f000 <_scoop$1$sr$c85fe09b5bacd3d7860263c6dd1ef05984e8fbc7f0cf96d0b4e70ee19ac755fb+0x90>
1000496b8:     	mov	x28, xzr
1000496bc:     	mov	x23, #0x7fffffffffffffff ; =9223372036854775807
1000496c0:     	add	x21, x21, #0x500
1000496c4:     	str	x20, [sp, #0x88]
1000496c8:     	mov	w20, #0x7f80            ; =32640
1000496cc:     	b	0x100049734 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x35c>
1000496d0:     	ldr	x9, [sp, #0x88]
1000496d4:     	mov	x0, x21
1000496d8:     	mov	w1, #0x18               ; =24
1000496dc:     	stp	x8, x9, [sp, #0x10]
1000496e0:     	bl	0x100058388 <_scoop_runtime_alloc_slow>
1000496e4:     	ldp	x9, x8, [sp, #0x10]
1000496e8:     	mov	x22, x0
1000496ec:     	str	x8, [sp, #0x88]
1000496f0:     	str	x9, [sp, #0x78]
1000496f4:     	ldr	x8, [x21, #0x60]
1000496f8:     	str	w28, [x22, #0x10]
1000496fc:     	ldr	x2, [x8, #0x8]
100049700:     	str	x22, [sp, #0x28]
100049704:     	ldr	x1, [sp, #0x28]
100049708:     	ldr	x8, [sp, #0x88]
10004970c:     	ldr	x0, [sp, #0x78]
100049710:     	stp	x8, x1, [sp, #0x18]
100049714:     	stp	x1, x0, [sp, #0x8]
100049718:     	bl	0x10006ab90 <dyld_stub_binder+0x10006ab90>
10004971c:     	ldp	x10, x8, [sp, #0x10]
100049720:     	ldr	x9, [sp, #0x8]
100049724:     	str	x8, [sp, #0x88]
100049728:     	str	x10, [sp, #0x78]
10004972c:     	str	x9, [sp, #0x20]
100049730:     	add	x28, x28, #0x1
100049734:     	ldar	x8, [x24]
100049738:     	ldar	w9, [x26]
10004973c:     	add	x11, x25, #0x8
100049740:     	ldar	w10, [x25]
100049744:     	ldar	x11, [x11]
100049748:     	cmp	w9, #0x0
10004974c:     	ccmp	w10, #0x1, #0x0, eq
100049750:     	ccmp	x11, x8, #0x0, eq
100049754:     	b.eq	0x10004976c <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x394>
100049758:     	ldr	x8, [sp, #0x88]
10004975c:     	str	x8, [sp, #0x18]
100049760:     	bl	0x100058378 <_scoop_rt_safepoint>
100049764:     	ldr	x8, [sp, #0x18]
100049768:     	str	x8, [sp, #0x88]
10004976c:     	cmp	x28, x19
100049770:     	b.ge	0x1000497f0 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x418>
100049774:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
100049778:     	ldr	x8, [sp, #0x88]
10004977c:     	ldr	x10, [x21, #0x18]
100049780:     	add	x0, x0, #0x5b8
100049784:     	str	x8, [sp, #0x78]
100049788:     	blr	x27
10004978c:     	ldr	x9, [x0]
100049790:     	neg	x11, x10
100049794:     	ldr	x12, [x9]
100049798:     	add	x12, x10, x12
10004979c:     	sub	x12, x12, #0x1
1000497a0:     	ands	x22, x12, x11
1000497a4:     	b.eq	0x1000496d0 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x2f8>
1000497a8:     	add	x12, x10, #0x17
1000497ac:     	and	x2, x12, x11
1000497b0:     	cmp	x2, x20
1000497b4:     	b.hi	0x1000496d0 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x2f8>
1000497b8:     	ldr	x11, [x21, #0x90]
1000497bc:     	cbnz	x11, 0x1000496d0 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x2f8>
1000497c0:     	ldr	x12, [x9, #0x8]
1000497c4:     	add	x11, x22, x2
1000497c8:     	cmp	x11, x12
1000497cc:     	b.hi	0x1000496d0 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x2f8>
1000497d0:     	add	x10, x10, x23
1000497d4:     	cmn	x10, #0x18
1000497d8:     	b.hs	0x1000496d0 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x2f8>
1000497dc:     	mov	x0, x22
1000497e0:     	mov	x1, x21
1000497e4:     	str	x11, [x9]
1000497e8:     	bl	0x100053da0 <_scoop_runtime_finish_tlab_alloc>
1000497ec:     	b	0x1000496f4 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x31c>
1000497f0:     	movi.2d	v0, #0000000000000000
1000497f4:     	add	x20, sp, #0x88
1000497f8:     	adrp	x8, 0x1001a6000 <_scoop$1$sr$1b87c2119cd87b2a3a24bfe10a2b6f9ca5227429405a54625917b3b05d41613d+0x60>
1000497fc:     	add	x8, x8, #0x980
100049800:     	add	x23, sp, #0xa0
100049804:     	add	x0, sp, #0x250
100049808:     	add	x1, sp, #0x240
10004980c:     	mov	w2, #0x1                ; =1
100049810:     	str	x20, [sp, #0x240]
100049814:     	str	x8, [sp, #0x248]
100049818:     	str	q0, [x23, #0x1b0]
10004981c:     	str	xzr, [sp, #0x260]
100049820:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100049824:     	movi.2d	v0, #0000000000000000
100049828:     	add	x8, sp, #0x169
10004982c:     	add	x0, sp, #0x268
100049830:     	add	x1, sp, #0x268
100049834:     	stur	q0, [x8, #0xff]
100049838:     	add	x8, sp, #0x179
10004983c:     	stur	q0, [x8, #0xff]
100049840:     	add	x8, sp, #0x189
100049844:     	stur	q0, [x8, #0xff]
100049848:     	add	x8, sp, #0x199
10004984c:     	stur	q0, [x8, #0xff]
100049850:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100049854:     	mov	w0, #0x1                ; =1
100049858:     	bl	0x1000691b0 <_m34_container_end>
10004985c:     	add	x0, sp, #0x268
100049860:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100049864:     	ldr	x21, [sp, #0x88]
100049868:     	add	x0, sp, #0x250
10004986c:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100049870:     	movi.2d	v0, #0000000000000000
100049874:     	adrp	x8, 0x1001a6000 <_scoop$1$sr$1b87c2119cd87b2a3a24bfe10a2b6f9ca5227429405a54625917b3b05d41613d+0x60>
100049878:     	add	x8, x8, #0x990
10004987c:     	str	x21, [sp, #0x88]
100049880:     	add	x0, sp, #0x2b8
100049884:     	add	x1, sp, #0x2a8
100049888:     	str	x8, [sp, #0x2b0]
10004988c:     	add	x8, sp, #0x1b9
100049890:     	mov	w2, #0x1                ; =1
100049894:     	str	x20, [sp, #0x2a8]
100049898:     	stur	q0, [x8, #0xff]
10004989c:     	str	xzr, [sp, #0x2c8]
1000498a0:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
1000498a4:     	movi.2d	v0, #0000000000000000
1000498a8:     	add	x0, sp, #0x2d0
1000498ac:     	add	x1, sp, #0x2d0
1000498b0:     	stp	q0, q0, [x23, #0x230]
1000498b4:     	stp	q0, q0, [x23, #0x250]
1000498b8:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
1000498bc:     	bl	0x100069138 <_m34_container_begin>
1000498c0:     	add	x0, sp, #0x2d0
1000498c4:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
1000498c8:     	ldr	x22, [sp, #0x88]
1000498cc:     	add	x0, sp, #0x2b8
1000498d0:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
1000498d4:     	mov	x20, xzr
1000498d8:     	mov	x21, xzr
1000498dc:     	str	x22, [sp, #0x88]
1000498e0:     	ldar	x8, [x24]
1000498e4:     	ldar	w9, [x26]
1000498e8:     	add	x11, x25, #0x8
1000498ec:     	ldar	w10, [x25]
1000498f0:     	ldar	x11, [x11]
1000498f4:     	cmp	w9, #0x0
1000498f8:     	ccmp	w10, #0x1, #0x0, eq
1000498fc:     	ccmp	x11, x8, #0x0, eq
100049900:     	b.eq	0x100049918 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x540>
100049904:     	ldr	x8, [sp, #0x88]
100049908:     	str	x8, [sp, #0x18]
10004990c:     	bl	0x100058378 <_scoop_rt_safepoint>
100049910:     	ldr	x8, [sp, #0x18]
100049914:     	str	x8, [sp, #0x88]
100049918:     	cmp	x21, x19
10004991c:     	b.ge	0x1000499b8 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x5e0>
100049920:     	ldr	x0, [sp, #0x88]
100049924:     	mov	x1, x21
100049928:     	str	x0, [sp, #0x70]
10004992c:     	str	x0, [sp, #0x50]
100049930:     	stp	x0, x0, [sp, #0x10]
100049934:     	bl	0x10006ab9c <dyld_stub_binder+0x10006ab9c>
100049938:     	ldp	x9, x8, [sp, #0x10]
10004993c:     	str	x8, [sp, #0x88]
100049940:     	str	x9, [sp, #0x70]
100049944:     	str	x9, [sp, #0x50]
100049948:     	mov	x1, x21
10004994c:     	ldr	x0, [x9, #0x10]
100049950:     	stp	x0, x8, [sp, #0x10]
100049954:     	add	x8, sp, #0x38
100049958:     	str	x0, [sp, #0x48]
10004995c:     	bl	0x10006ad70 <dyld_stub_binder+0x10006ad70>
100049960:     	ldp	x9, x8, [sp, #0x10]
100049964:     	str	x8, [sp, #0x88]
100049968:     	str	x9, [sp, #0x48]
10004996c:     	ldp	x8, x9, [sp, #0x38]
100049970:     	str	x8, [sp, #0x30]
100049974:     	ldr	x8, [sp, #0x30]
100049978:     	str	x8, [sp, #0x60]
10004997c:     	ldr	x8, [sp, #0x60]
100049980:     	str	x8, [sp, #0x58]
100049984:     	ldr	x8, [sp, #0x58]
100049988:     	ldr	x0, [sp, #0x58]
10004998c:     	ldr	x8, [sp, #0x88]
100049990:     	str	x0, [sp, #0x98]
100049994:     	ldr	x9, [x9]
100049998:     	stp	x0, x8, [sp, #0x10]
10004999c:     	blr	x9
1000499a0:     	ldp	x9, x8, [sp, #0x10]
1000499a4:     	str	x8, [sp, #0x88]
1000499a8:     	add	x20, x20, w0, sxtw
1000499ac:     	str	x9, [sp, #0x98]
1000499b0:     	add	x21, x21, #0x1
1000499b4:     	b	0x1000498e0 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x508>
1000499b8:     	movi.2d	v0, #0000000000000000
1000499bc:     	add	x21, sp, #0x88
1000499c0:     	adrp	x8, 0x1001a6000 <_scoop$1$sr$1b87c2119cd87b2a3a24bfe10a2b6f9ca5227429405a54625917b3b05d41613d+0x60>
1000499c4:     	add	x8, x8, #0x9a0
1000499c8:     	add	x0, sp, #0x320
1000499cc:     	add	x1, sp, #0x310
1000499d0:     	mov	w2, #0x1                ; =1
1000499d4:     	str	x21, [sp, #0x310]
1000499d8:     	str	x8, [sp, #0x318]
1000499dc:     	str	q0, [x23, #0x280]
1000499e0:     	str	xzr, [sp, #0x330]
1000499e4:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
1000499e8:     	movi.2d	v0, #0000000000000000
1000499ec:     	add	x8, sp, #0x239
1000499f0:     	add	x0, sp, #0x338
1000499f4:     	add	x1, sp, #0x338
1000499f8:     	stur	q0, [x8, #0xff]
1000499fc:     	add	x8, sp, #0x249
100049a00:     	stur	q0, [x8, #0xff]
100049a04:     	add	x8, sp, #0x259
100049a08:     	stur	q0, [x8, #0xff]
100049a0c:     	add	x8, sp, #0x269
100049a10:     	stur	q0, [x8, #0xff]
100049a14:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100049a18:     	mov	w0, #0x2                ; =2
100049a1c:     	bl	0x1000691b0 <_m34_container_end>
100049a20:     	add	x0, sp, #0x338
100049a24:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100049a28:     	ldr	x22, [sp, #0x88]
100049a2c:     	add	x0, sp, #0x320
100049a30:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100049a34:     	sub	x8, x19, #0x1
100049a38:     	mov	x9, x22
100049a3c:     	str	x22, [sp, #0x88]
100049a40:     	mul	x8, x19, x8
100049a44:     	str	x9, [sp, #0x18]
100049a48:     	add	x8, x8, x8, lsr #63
100049a4c:     	cmp	x20, x8, asr #1
100049a50:     	cset	w0, eq
100049a54:     	bl	0x10003ec6c <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
100049a58:     	ldr	x8, [sp, #0x18]
100049a5c:     	movi.2d	v0, #0000000000000000
100049a60:     	str	x8, [sp, #0x88]
100049a64:     	adrp	x8, 0x1001a6000 <_scoop$1$sr$1b87c2119cd87b2a3a24bfe10a2b6f9ca5227429405a54625917b3b05d41613d+0x60>
100049a68:     	add	x8, x8, #0x9b0
100049a6c:     	add	x0, sp, #0x388
100049a70:     	add	x1, sp, #0x378
100049a74:     	str	x8, [sp, #0x380]
100049a78:     	add	x8, sp, #0x289
100049a7c:     	mov	w2, #0x1                ; =1
100049a80:     	str	x21, [sp, #0x378]
100049a84:     	stur	q0, [x8, #0xff]
100049a88:     	str	xzr, [sp, #0x398]
100049a8c:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100049a90:     	movi.2d	v0, #0000000000000000
100049a94:     	sub	x0, x29, #0x100
100049a98:     	sub	x1, x29, #0x100
100049a9c:     	stp	q0, q0, [x23, #0x300]
100049aa0:     	stp	q0, q0, [x23, #0x320]
100049aa4:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100049aa8:     	bl	0x100069138 <_m34_container_begin>
100049aac:     	sub	x0, x29, #0x100
100049ab0:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100049ab4:     	ldr	x19, [sp, #0x88]
100049ab8:     	add	x0, sp, #0x388
100049abc:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100049ac0:     	mov	x0, x19
100049ac4:     	str	x19, [sp, #0x88]
100049ac8:     	str	x19, [sp, #0x18]
100049acc:     	bl	0x10006aac4 <dyld_stub_binder+0x10006aac4>
100049ad0:     	ldr	x8, [sp, #0x18]
100049ad4:     	str	x8, [sp, #0x88]
100049ad8:     	movi.2d	v0, #0000000000000000
100049adc:     	str	x8, [sp, #0x68]
100049ae0:     	adrp	x8, 0x1001a6000 <_scoop$1$sr$1b87c2119cd87b2a3a24bfe10a2b6f9ca5227429405a54625917b3b05d41613d+0x60>
100049ae4:     	add	x8, x8, #0x9c0
100049ae8:     	sub	x0, x29, #0xb0
100049aec:     	sub	x1, x29, #0xc0
100049af0:     	mov	w2, #0x1                ; =1
100049af4:     	stp	x21, x8, [x29, #-0xc0]
100049af8:     	stur	xzr, [x29, #-0xa0]
100049afc:     	str	q0, [x23, #0x350]
100049b00:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
100049b04:     	movi.2d	v0, #0000000000000000
100049b08:     	sub	x0, x29, #0x98
100049b0c:     	sub	x1, x29, #0x98
100049b10:     	stur	q0, [x29, #-0x98]
100049b14:     	stur	q0, [x29, #-0x88]
100049b18:     	stur	q0, [x29, #-0x78]
100049b1c:     	stur	q0, [x29, #-0x68]
100049b20:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
100049b24:     	mov	w0, #0x3                ; =3
100049b28:     	bl	0x1000691b0 <_m34_container_end>
100049b2c:     	sub	x0, x29, #0x98
100049b30:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
100049b34:     	ldr	x19, [sp, #0x88]
100049b38:     	sub	x0, x29, #0xb0
100049b3c:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
100049b40:     	str	x19, [sp, #0x88]
100049b44:     	ldr	x8, [x19, #0x18]
100049b48:     	cmp	x8, #0x0
100049b4c:     	cset	w0, eq
100049b50:     	bl	0x10003ec6c <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
100049b54:     	mov	x0, x20
100049b58:     	add	sp, sp, #0x450
100049b5c:     	ldp	x29, x30, [sp, #0x50]
100049b60:     	ldp	x20, x19, [sp, #0x40]
100049b64:     	ldp	x22, x21, [sp, #0x30]
100049b68:     	ldp	x24, x23, [sp, #0x20]
100049b6c:     	ldp	x26, x25, [sp, #0x10]
100049b70:     	ldp	x28, x27, [sp], #0x60
100049b74:     	ret

; zeroSizeList, MIR fn6, LIR scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-on-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010003b4e4 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0>:
10003b4e4:     	stp	x28, x27, [sp, #-0x60]!
10003b4e8:     	stp	x26, x25, [sp, #0x10]
10003b4ec:     	stp	x24, x23, [sp, #0x20]
10003b4f0:     	stp	x22, x21, [sp, #0x30]
10003b4f4:     	stp	x20, x19, [sp, #0x40]
10003b4f8:     	stp	x29, x30, [sp, #0x50]
10003b4fc:     	add	x29, sp, #0x50
10003b500:     	sub	sp, sp, #0x310
10003b504:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
10003b508:     	mov	x19, x0
10003b50c:     	add	x8, x8, #0x5d0
10003b510:     	ldr	x9, [x8]
10003b514:     	mov	x0, x8
10003b518:     	blr	x9
10003b51c:     	adrp	x23, 0x10020d000 <_scoop_gc_marker+0x800>
10003b520:     	adrp	x25, 0x10020d000 <_scoop_gc_marker+0x800>
10003b524:     	add	x23, x23, #0xa10
10003b528:     	ldr	x24, [x0]
10003b52c:     	add	x25, x25, #0x9fc
10003b530:     	add	x10, x24, #0x8
10003b534:     	ldar	x8, [x23]
10003b538:     	ldar	w11, [x25]
10003b53c:     	ldar	w9, [x24]
10003b540:     	ldar	x10, [x10]
10003b544:     	cbnz	w11, 0x10003b554 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x70>
10003b548:     	cmp	w9, #0x1
10003b54c:     	ccmp	x10, x8, #0x0, eq
10003b550:     	b.eq	0x10003b558 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x74>
10003b554:     	bl	0x100058378 <_scoop_rt_safepoint>
10003b558:     	add	x22, sp, #0xb8
10003b55c:     	bl	0x100047364 <_scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
10003b560:     	adrp	x21, 0x10020d000 <_scoop_gc_marker+0x800>
10003b564:     	movi.2d	v0, #0000000000000000
10003b568:     	add	x0, sp, #0x38
10003b56c:     	add	x21, x21, #0x700
10003b570:     	mov	x1, xzr
10003b574:     	mov	x2, xzr
10003b578:     	ldr	x8, [x21]
10003b57c:     	str	xzr, [x8, #0x10]
10003b580:     	stur	q0, [sp, #0x38]
10003b584:     	str	xzr, [sp, #0x48]
10003b588:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003b58c:     	movi.2d	v0, #0000000000000000
10003b590:     	add	x0, sp, #0x50
10003b594:     	add	x1, sp, #0x50
10003b598:     	stp	q0, q0, [sp, #0x50]
10003b59c:     	stp	q0, q0, [sp, #0x70]
10003b5a0:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10003b5a4:     	bl	0x100069138 <_m34_container_begin>
10003b5a8:     	add	x0, sp, #0x50
10003b5ac:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10003b5b0:     	add	x0, sp, #0x38
10003b5b4:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003b5b8:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
10003b5bc:     	adrp	x10, 0x100208000 <dyld_stub_binder+0x100208000>
10003b5c0:     	add	x0, x0, #0x5b8
10003b5c4:     	ldr	x10, [x10, #0x110]
10003b5c8:     	ldr	x8, [x0]
10003b5cc:     	ldr	x9, [x10, #0x18]
10003b5d0:     	blr	x8
10003b5d4:     	ldr	x8, [x0]
10003b5d8:     	neg	x11, x9
10003b5dc:     	ldr	x12, [x8]
10003b5e0:     	add	x12, x9, x12
10003b5e4:     	sub	x12, x12, #0x1
10003b5e8:     	ands	x20, x12, x11
10003b5ec:     	b.eq	0x10003b648 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x164>
10003b5f0:     	add	x12, x9, #0x1f
10003b5f4:     	and	x2, x12, x11
10003b5f8:     	mov	w11, #0x7f80            ; =32640
10003b5fc:     	cmp	x2, x11
10003b600:     	b.hi	0x10003b648 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x164>
10003b604:     	ldr	x10, [x10, #0x90]
10003b608:     	cbnz	x10, 0x10003b648 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x164>
10003b60c:     	ldr	x11, [x8, #0x8]
10003b610:     	add	x10, x20, x2
10003b614:     	cmp	x10, x11
10003b618:     	b.hi	0x10003b648 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x164>
10003b61c:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10003b620:     	add	x9, x9, x11
10003b624:     	cmn	x9, #0x21
10003b628:     	b.hi	0x10003b648 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x164>
10003b62c:     	str	x10, [x8]
10003b630:     	adrp	x1, 0x100208000 <dyld_stub_binder+0x100208000>
10003b634:     	mov	x0, x20
10003b638:     	ldr	x1, [x1, #0x110]
10003b63c:     	bl	0x100053da0 <_scoop_runtime_finish_tlab_alloc>
10003b640:     	mov	x0, x20
10003b644:     	b	0x10003b658 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x174>
10003b648:     	adrp	x0, 0x100208000 <dyld_stub_binder+0x100208000>
10003b64c:     	mov	w1, #0x20               ; =32
10003b650:     	ldr	x0, [x0, #0x110]
10003b654:     	bl	0x100058388 <_scoop_runtime_alloc_slow>
10003b658:     	mov	x1, x19
10003b65c:     	str	x0, [sp, #0x8]
10003b660:     	bl	0x10006ac74 <dyld_stub_binder+0x10006ac74>
10003b664:     	ldr	x8, [sp, #0x8]
10003b668:     	movi.2d	v0, #0000000000000000
10003b66c:     	str	x8, [sp, #0x30]
10003b670:     	add	x20, sp, #0x28
10003b674:     	str	x8, [sp, #0x28]
10003b678:     	adrp	x8, 0x10017d000 <_scoop$1$sr$9220b82b32a160780ba06baf06e25a4e69082e995fa1f4f0fd59ef8c95cea840+0x90>
10003b67c:     	add	x8, x8, #0xee0
10003b680:     	add	x0, sp, #0xa0
10003b684:     	add	x1, sp, #0x90
10003b688:     	mov	w2, #0x1                ; =1
10003b68c:     	stp	x20, x8, [sp, #0x90]
10003b690:     	str	q0, [sp, #0xa0]
10003b694:     	str	xzr, [sp, #0xb0]
10003b698:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003b69c:     	movi.2d	v0, #0000000000000000
10003b6a0:     	add	x0, sp, #0xb8
10003b6a4:     	add	x1, sp, #0xb8
10003b6a8:     	stp	q0, q0, [x22]
10003b6ac:     	stp	q0, q0, [x22, #0x20]
10003b6b0:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10003b6b4:     	mov	w0, wzr
10003b6b8:     	bl	0x1000691b0 <_m34_container_end>
10003b6bc:     	add	x0, sp, #0xb8
10003b6c0:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10003b6c4:     	ldr	x26, [sp, #0x28]
10003b6c8:     	add	x0, sp, #0xa0
10003b6cc:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003b6d0:     	movi.2d	v0, #0000000000000000
10003b6d4:     	adrp	x8, 0x10017d000 <_scoop$1$sr$9220b82b32a160780ba06baf06e25a4e69082e995fa1f4f0fd59ef8c95cea840+0x90>
10003b6d8:     	add	x8, x8, #0xef0
10003b6dc:     	str	x26, [sp, #0x28]
10003b6e0:     	adrp	x9, 0x10017d000 <_scoop$1$sr$9220b82b32a160780ba06baf06e25a4e69082e995fa1f4f0fd59ef8c95cea840+0x90>
10003b6e4:     	add	x9, x9, #0xf00
10003b6e8:     	stp	x20, x8, [sp, #0xf8]
10003b6ec:     	add	x8, sp, #0x20
10003b6f0:     	add	x0, sp, #0x118
10003b6f4:     	add	x1, sp, #0xf8
10003b6f8:     	mov	w2, #0x2                ; =2
10003b6fc:     	str	x26, [sp, #0x20]
10003b700:     	stp	x8, x9, [sp, #0x108]
10003b704:     	str	q0, [x22, #0x60]
10003b708:     	str	xzr, [sp, #0x128]
10003b70c:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003b710:     	movi.2d	v0, #0000000000000000
10003b714:     	add	x0, sp, #0x130
10003b718:     	add	x1, sp, #0x130
10003b71c:     	stp	q0, q0, [sp, #0x130]
10003b720:     	stp	q0, q0, [sp, #0x150]
10003b724:     	bl	0x100058488 <_scoop_rt_enter_native_borrowed>
10003b728:     	ldr	x0, [sp, #0x20]
10003b72c:     	bl	0x1000692ac <_m34_container_layout>
10003b730:     	add	x0, sp, #0x130
10003b734:     	bl	0x100063ad0 <_scoop_rt_leave_native_borrowed>
10003b738:     	ldr	x26, [sp, #0x28]
10003b73c:     	ldr	x27, [sp, #0x20]
10003b740:     	add	x0, sp, #0x118
10003b744:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003b748:     	movi.2d	v0, #0000000000000000
10003b74c:     	adrp	x8, 0x10017d000 <_scoop$1$sr$9220b82b32a160780ba06baf06e25a4e69082e995fa1f4f0fd59ef8c95cea840+0x90>
10003b750:     	add	x8, x8, #0xf10
10003b754:     	add	x0, sp, #0x180
10003b758:     	add	x1, sp, #0x170
10003b75c:     	mov	w2, #0x1                ; =1
10003b760:     	str	x26, [sp, #0x28]
10003b764:     	str	x27, [sp, #0x20]
10003b768:     	stp	x20, x8, [sp, #0x170]
10003b76c:     	str	q0, [sp, #0x180]
10003b770:     	str	xzr, [sp, #0x190]
10003b774:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003b778:     	movi.2d	v0, #0000000000000000
10003b77c:     	add	x0, sp, #0x198
10003b780:     	add	x1, sp, #0x198
10003b784:     	stp	q0, q0, [x22, #0xe0]
10003b788:     	stp	q0, q0, [x22, #0x100]
10003b78c:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10003b790:     	bl	0x100069138 <_m34_container_begin>
10003b794:     	add	x0, sp, #0x198
10003b798:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10003b79c:     	ldr	x26, [sp, #0x28]
10003b7a0:     	add	x0, sp, #0x180
10003b7a4:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003b7a8:     	mov	x20, xzr
10003b7ac:     	str	x26, [sp, #0x28]
10003b7b0:     	ldar	x8, [x23]
10003b7b4:     	ldar	w9, [x25]
10003b7b8:     	add	x11, x24, #0x8
10003b7bc:     	ldar	w10, [x24]
10003b7c0:     	ldar	x11, [x11]
10003b7c4:     	cmp	w9, #0x0
10003b7c8:     	ccmp	w10, #0x1, #0x0, eq
10003b7cc:     	ccmp	x11, x8, #0x0, eq
10003b7d0:     	b.eq	0x10003b7e8 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x304>
10003b7d4:     	ldr	x8, [sp, #0x28]
10003b7d8:     	str	x8, [sp, #0x8]
10003b7dc:     	bl	0x100058378 <_scoop_rt_safepoint>
10003b7e0:     	ldr	x8, [sp, #0x8]
10003b7e4:     	str	x8, [sp, #0x28]
10003b7e8:     	cmp	x20, x19
10003b7ec:     	b.ge	0x10003b840 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x35c>
10003b7f0:     	ldr	x8, [sp, #0x28]
10003b7f4:     	str	x8, [sp, #0x18]
10003b7f8:     	stp	x8, x8, [sp]
10003b7fc:     	bl	0x100047364 <_scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
10003b800:     	ldp	x9, x8, [sp]
10003b804:     	str	x8, [sp, #0x28]
10003b808:     	str	x9, [sp, #0x18]
10003b80c:     	ldr	x8, [x21]
10003b810:     	ldr	x9, [x8, #0x10]
10003b814:     	add	x9, x9, #0x1
10003b818:     	str	x9, [x8, #0x10]
10003b81c:     	ldr	x8, [sp, #0x28]
10003b820:     	ldr	x0, [sp, #0x18]
10003b824:     	stp	x0, x8, [sp]
10003b828:     	bl	0x10006abb4 <dyld_stub_binder+0x10006abb4>
10003b82c:     	ldp	x9, x8, [sp]
10003b830:     	str	x8, [sp, #0x28]
10003b834:     	str	x9, [sp, #0x18]
10003b838:     	add	x20, x20, #0x1
10003b83c:     	b	0x10003b7b0 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x2cc>
10003b840:     	movi.2d	v0, #0000000000000000
10003b844:     	add	x20, sp, #0x28
10003b848:     	adrp	x8, 0x10017d000 <_scoop$1$sr$9220b82b32a160780ba06baf06e25a4e69082e995fa1f4f0fd59ef8c95cea840+0x90>
10003b84c:     	add	x8, x8, #0xf20
10003b850:     	add	x0, sp, #0x1e8
10003b854:     	add	x1, sp, #0x1d8
10003b858:     	mov	w2, #0x1                ; =1
10003b85c:     	stp	x20, x8, [sp, #0x1d8]
10003b860:     	str	xzr, [sp, #0x1f8]
10003b864:     	str	q0, [x22, #0x130]
10003b868:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003b86c:     	movi.2d	v0, #0000000000000000
10003b870:     	add	x0, sp, #0x200
10003b874:     	add	x1, sp, #0x200
10003b878:     	stp	q0, q0, [sp, #0x200]
10003b87c:     	stp	q0, q0, [sp, #0x220]
10003b880:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10003b884:     	mov	w0, #0x1                ; =1
10003b888:     	bl	0x1000691b0 <_m34_container_end>
10003b88c:     	add	x0, sp, #0x200
10003b890:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10003b894:     	ldr	x23, [sp, #0x28]
10003b898:     	add	x0, sp, #0x1e8
10003b89c:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003b8a0:     	str	x23, [sp, #0x28]
10003b8a4:     	ldr	x8, [x23, #0x18]
10003b8a8:     	cmp	x8, x19
10003b8ac:     	b.ne	0x10003b8d4 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3f0>
10003b8b0:     	str	x23, [sp, #0x8]
10003b8b4:     	bl	0x100047364 <_scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
10003b8b8:     	ldr	x8, [sp, #0x8]
10003b8bc:     	str	x8, [sp, #0x28]
10003b8c0:     	ldr	x8, [x21]
10003b8c4:     	ldr	x8, [x8, #0x10]
10003b8c8:     	cmp	x8, x19
10003b8cc:     	cset	w0, eq
10003b8d0:     	b	0x10003b8d8 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3f4>
10003b8d4:     	mov	w0, wzr
10003b8d8:     	ldr	x8, [sp, #0x28]
10003b8dc:     	str	x8, [sp, #0x8]
10003b8e0:     	bl	0x10003ec6c <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
10003b8e4:     	ldr	x8, [sp, #0x8]
10003b8e8:     	movi.2d	v0, #0000000000000000
10003b8ec:     	str	x8, [sp, #0x28]
10003b8f0:     	adrp	x8, 0x10017d000 <_scoop$1$sr$9220b82b32a160780ba06baf06e25a4e69082e995fa1f4f0fd59ef8c95cea840+0x90>
10003b8f4:     	add	x8, x8, #0xf30
10003b8f8:     	add	x0, sp, #0x250
10003b8fc:     	add	x1, sp, #0x240
10003b900:     	mov	w2, #0x1                ; =1
10003b904:     	str	x20, [sp, #0x240]
10003b908:     	str	x8, [sp, #0x248]
10003b90c:     	str	q0, [sp, #0x250]
10003b910:     	str	xzr, [sp, #0x260]
10003b914:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003b918:     	movi.2d	v0, #0000000000000000
10003b91c:     	sub	x0, x29, #0xf8
10003b920:     	sub	x1, x29, #0xf8
10003b924:     	stp	q0, q0, [x22, #0x1b0]
10003b928:     	stp	q0, q0, [x22, #0x1d0]
10003b92c:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10003b930:     	bl	0x100069138 <_m34_container_begin>
10003b934:     	sub	x0, x29, #0xf8
10003b938:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10003b93c:     	ldr	x19, [sp, #0x28]
10003b940:     	add	x0, sp, #0x250
10003b944:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003b948:     	mov	x0, x19
10003b94c:     	str	x19, [sp, #0x28]
10003b950:     	str	x19, [sp, #0x8]
10003b954:     	bl	0x10006aca4 <dyld_stub_binder+0x10006aca4>
10003b958:     	ldr	x8, [sp, #0x8]
10003b95c:     	str	x8, [sp, #0x28]
10003b960:     	movi.2d	v0, #0000000000000000
10003b964:     	str	x8, [sp, #0x10]
10003b968:     	adrp	x8, 0x10017d000 <_scoop$1$sr$9220b82b32a160780ba06baf06e25a4e69082e995fa1f4f0fd59ef8c95cea840+0x90>
10003b96c:     	add	x8, x8, #0xf40
10003b970:     	sub	x0, x29, #0xa8
10003b974:     	sub	x1, x29, #0xb8
10003b978:     	mov	w2, #0x1                ; =1
10003b97c:     	stp	x20, x8, [x29, #-0xb8]
10003b980:     	stur	xzr, [x29, #-0x98]
10003b984:     	str	q0, [x22, #0x200]
10003b988:     	bl	0x100064f28 <_scoop_rt_push_caller_roots>
10003b98c:     	movi.2d	v0, #0000000000000000
10003b990:     	sub	x0, x29, #0x90
10003b994:     	sub	x1, x29, #0x90
10003b998:     	stp	q0, q0, [x29, #-0x90]
10003b99c:     	stp	q0, q0, [x29, #-0x70]
10003b9a0:     	bl	0x100058478 <_scoop_rt_enter_native_safe>
10003b9a4:     	mov	w0, #0x3                ; =3
10003b9a8:     	bl	0x1000691b0 <_m34_container_end>
10003b9ac:     	sub	x0, x29, #0x90
10003b9b0:     	bl	0x100063970 <_scoop_rt_leave_native_safe>
10003b9b4:     	ldr	x19, [sp, #0x28]
10003b9b8:     	sub	x0, x29, #0xa8
10003b9bc:     	bl	0x100064fd0 <_scoop_rt_pop_caller_roots>
10003b9c0:     	str	x19, [sp, #0x28]
10003b9c4:     	ldr	x8, [x19, #0x18]
10003b9c8:     	cmp	x8, #0x0
10003b9cc:     	cset	w0, eq
10003b9d0:     	bl	0x10003ec6c <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
10003b9d4:     	bl	0x100047364 <_scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
10003b9d8:     	ldr	x8, [x21]
10003b9dc:     	ldr	x0, [x8, #0x10]
10003b9e0:     	add	sp, sp, #0x310
10003b9e4:     	ldp	x29, x30, [sp, #0x50]
10003b9e8:     	ldp	x20, x19, [sp, #0x40]
10003b9ec:     	ldp	x22, x21, [sp, #0x30]
10003b9f0:     	ldp	x24, x23, [sp, #0x20]
10003b9f4:     	ldp	x26, x25, [sp, #0x10]
10003b9f8:     	ldp	x28, x27, [sp], #0x60
10003b9fc:     	ret
