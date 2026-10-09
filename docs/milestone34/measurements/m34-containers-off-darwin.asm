; lowOccupancy, MIR fn1, LIR scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-off-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100047cd0 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2>:
100047cd0:     	stp	x28, x27, [sp, #-0x60]!
100047cd4:     	stp	x26, x25, [sp, #0x10]
100047cd8:     	stp	x24, x23, [sp, #0x20]
100047cdc:     	stp	x22, x21, [sp, #0x30]
100047ce0:     	stp	x20, x19, [sp, #0x40]
100047ce4:     	stp	x29, x30, [sp, #0x50]
100047ce8:     	add	x29, sp, #0x50
100047cec:     	sub	sp, sp, #0x260
100047cf0:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
100047cf4:     	mov	x19, x0
100047cf8:     	add	x8, x8, #0x5d0
100047cfc:     	ldr	x9, [x8]
100047d00:     	mov	x0, x8
100047d04:     	blr	x9
100047d08:     	adrp	x22, 0x10020d000 <_scoop_gc_marker+0x800>
100047d0c:     	adrp	x24, 0x10020d000 <_scoop_gc_marker+0x800>
100047d10:     	add	x22, x22, #0xa10
100047d14:     	ldr	x23, [x0]
100047d18:     	add	x24, x24, #0x9fc
100047d1c:     	add	x10, x23, #0x8
100047d20:     	ldar	x8, [x22]
100047d24:     	ldar	w11, [x24]
100047d28:     	ldar	w9, [x23]
100047d2c:     	ldar	x10, [x10]
100047d30:     	cbnz	w11, 0x100047d40 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x70>
100047d34:     	cmp	w9, #0x1
100047d38:     	ccmp	x10, x8, #0x0, eq
100047d3c:     	b.eq	0x100047d44 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x74>
100047d40:     	bl	0x100059084 <_scoop_rt_safepoint>
100047d44:     	movi.2d	v0, #0000000000000000
100047d48:     	add	x0, sp, #0x50
100047d4c:     	mov	x1, xzr
100047d50:     	mov	x2, xzr
100047d54:     	add	x25, sp, #0xd0
100047d58:     	str	xzr, [sp, #0x60]
100047d5c:     	str	q0, [sp, #0x50]
100047d60:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100047d64:     	movi.2d	v0, #0000000000000000
100047d68:     	add	x0, sp, #0x68
100047d6c:     	add	x1, sp, #0x68
100047d70:     	stur	q0, [sp, #0x68]
100047d74:     	stur	q0, [sp, #0x78]
100047d78:     	stur	q0, [sp, #0x88]
100047d7c:     	stur	q0, [sp, #0x98]
100047d80:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100047d84:     	bl	0x100069e44 <_m34_container_begin>
100047d88:     	add	x0, sp, #0x68
100047d8c:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100047d90:     	add	x0, sp, #0x50
100047d94:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100047d98:     	adrp	x20, 0x10020d000 <_scoop_gc_marker+0x800>
100047d9c:     	adrp	x10, 0x100208000 <dyld_stub_binder+0x100208000>
100047da0:     	add	x20, x20, #0x5b8
100047da4:     	ldr	x10, [x10, #0xc18]
100047da8:     	ldr	x26, [x20]
100047dac:     	ldr	x9, [x10, #0x18]
100047db0:     	mov	x0, x20
100047db4:     	blr	x26
100047db8:     	ldr	x8, [x0]
100047dbc:     	neg	x11, x9
100047dc0:     	ldr	x12, [x8]
100047dc4:     	add	x12, x9, x12
100047dc8:     	sub	x12, x12, #0x1
100047dcc:     	ands	x21, x12, x11
100047dd0:     	b.eq	0x100047e2c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x15c>
100047dd4:     	add	x12, x9, #0x1f
100047dd8:     	and	x2, x12, x11
100047ddc:     	mov	w11, #0x7f80            ; =32640
100047de0:     	cmp	x2, x11
100047de4:     	b.hi	0x100047e2c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x15c>
100047de8:     	ldr	x10, [x10, #0x90]
100047dec:     	cbnz	x10, 0x100047e2c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x15c>
100047df0:     	ldr	x11, [x8, #0x8]
100047df4:     	add	x10, x21, x2
100047df8:     	cmp	x10, x11
100047dfc:     	b.hi	0x100047e2c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x15c>
100047e00:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
100047e04:     	add	x9, x9, x11
100047e08:     	cmn	x9, #0x21
100047e0c:     	b.hi	0x100047e2c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x15c>
100047e10:     	str	x10, [x8]
100047e14:     	adrp	x1, 0x100208000 <dyld_stub_binder+0x100208000>
100047e18:     	mov	x0, x21
100047e1c:     	ldr	x1, [x1, #0xc18]
100047e20:     	bl	0x100054aac <_scoop_runtime_finish_tlab_alloc>
100047e24:     	mov	x0, x21
100047e28:     	b	0x100047e3c <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x16c>
100047e2c:     	adrp	x0, 0x100208000 <dyld_stub_binder+0x100208000>
100047e30:     	mov	w1, #0x20               ; =32
100047e34:     	ldr	x0, [x0, #0xc18]
100047e38:     	bl	0x100059094 <_scoop_runtime_alloc_slow>
100047e3c:     	mov	x1, x19
100047e40:     	str	x0, [sp, #0x18]
100047e44:     	bl	0x10006b8fc <dyld_stub_binder+0x10006b8fc>
100047e48:     	ldr	x8, [sp, #0x18]
100047e4c:     	movi.2d	v0, #0000000000000000
100047e50:     	str	x8, [sp, #0x48]
100047e54:     	adrp	x9, 0x1001a0000 <_scoop$1$sr$ef1cf0bec3634d609ce2c73d3d2ee69d1d6a7273a6c0315d9b2bf3764e4974f0+0x20>
100047e58:     	add	x9, x9, #0xaf0
100047e5c:     	str	x8, [sp, #0x40]
100047e60:     	add	x8, sp, #0x40
100047e64:     	add	x0, sp, #0xb8
100047e68:     	add	x1, sp, #0xa8
100047e6c:     	mov	w2, #0x1                ; =1
100047e70:     	stp	x8, x9, [sp, #0xa8]
100047e74:     	stur	q0, [sp, #0xb8]
100047e78:     	str	xzr, [sp, #0xc8]
100047e7c:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100047e80:     	movi.2d	v0, #0000000000000000
100047e84:     	add	x0, sp, #0xd0
100047e88:     	add	x1, sp, #0xd0
100047e8c:     	stp	q0, q0, [x25]
100047e90:     	stp	q0, q0, [x25, #0x20]
100047e94:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100047e98:     	mov	w0, wzr
100047e9c:     	bl	0x100069ebc <_m34_container_end>
100047ea0:     	add	x0, sp, #0xd0
100047ea4:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100047ea8:     	ldr	x21, [sp, #0x40]
100047eac:     	add	x0, sp, #0xb8
100047eb0:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100047eb4:     	adrp	x10, 0x100180000 <_scoop$1$sr$f64295cddd0798fe06071e3c922bcfb410cb3916b2f9ec2e2ac0c5edb140a652+0x30>
100047eb8:     	mov	x0, x20
100047ebc:     	add	x10, x10, #0xad0
100047ec0:     	str	x21, [sp, #0x40]
100047ec4:     	str	x21, [sp, #0x38]
100047ec8:     	ldr	x9, [x10, #0x18]
100047ecc:     	blr	x26
100047ed0:     	ldr	x8, [x0]
100047ed4:     	neg	x11, x9
100047ed8:     	ldr	x12, [x8]
100047edc:     	add	x12, x9, x12
100047ee0:     	sub	x12, x12, #0x1
100047ee4:     	ands	x19, x12, x11
100047ee8:     	b.eq	0x100047f40 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x270>
100047eec:     	add	x12, x9, #0x17
100047ef0:     	and	x2, x12, x11
100047ef4:     	mov	w11, #0x7f80            ; =32640
100047ef8:     	cmp	x2, x11
100047efc:     	b.hi	0x100047f40 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x270>
100047f00:     	ldr	x10, [x10, #0x90]
100047f04:     	cbnz	x10, 0x100047f40 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x270>
100047f08:     	ldr	x11, [x8, #0x8]
100047f0c:     	add	x10, x19, x2
100047f10:     	cmp	x10, x11
100047f14:     	b.hi	0x100047f40 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x270>
100047f18:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
100047f1c:     	add	x9, x9, x11
100047f20:     	cmn	x9, #0x19
100047f24:     	b.hi	0x100047f40 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x270>
100047f28:     	str	x10, [x8]
100047f2c:     	adrp	x1, 0x100180000 <_scoop$1$sr$f64295cddd0798fe06071e3c922bcfb410cb3916b2f9ec2e2ac0c5edb140a652+0x30>
100047f30:     	mov	x0, x19
100047f34:     	add	x1, x1, #0xad0
100047f38:     	bl	0x100054aac <_scoop_runtime_finish_tlab_alloc>
100047f3c:     	b	0x100047f68 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x298>
100047f40:     	ldr	x8, [sp, #0x40]
100047f44:     	adrp	x0, 0x100180000 <_scoop$1$sr$f64295cddd0798fe06071e3c922bcfb410cb3916b2f9ec2e2ac0c5edb140a652+0x30>
100047f48:     	mov	w1, #0x18               ; =24
100047f4c:     	stp	x21, x8, [sp, #0x10]
100047f50:     	add	x0, x0, #0xad0
100047f54:     	bl	0x100059094 <_scoop_runtime_alloc_slow>
100047f58:     	ldp	x9, x8, [sp, #0x10]
100047f5c:     	mov	x19, x0
100047f60:     	str	x8, [sp, #0x40]
100047f64:     	str	x9, [sp, #0x38]
100047f68:     	mov	w8, #0x7                ; =7
100047f6c:     	mov	x1, x19
100047f70:     	str	w8, [x19, #0x10]
100047f74:     	ldp	x0, x8, [sp, #0x38]
100047f78:     	stp	x19, x0, [sp, #0x10]
100047f7c:     	str	x8, [sp, #0x8]
100047f80:     	bl	0x10006b7c4 <dyld_stub_binder+0x10006b7c4>
100047f84:     	ldp	x8, x9, [sp, #0x8]
100047f88:     	ldr	x10, [sp, #0x18]
100047f8c:     	str	x8, [sp, #0x40]
100047f90:     	str	x10, [sp, #0x38]
100047f94:     	movi.2d	v0, #0000000000000000
100047f98:     	str	x9, [sp, #0x28]
100047f9c:     	add	x21, sp, #0x40
100047fa0:     	str	x8, [sp, #0x20]
100047fa4:     	adrp	x8, 0x1001a0000 <_scoop$1$sr$ef1cf0bec3634d609ce2c73d3d2ee69d1d6a7273a6c0315d9b2bf3764e4974f0+0x20>
100047fa8:     	add	x8, x8, #0xb20
100047fac:     	stp	x21, x8, [x29, #-0xd0]
100047fb0:     	add	x8, sp, #0x20
100047fb4:     	adrp	x9, 0x1001a0000 <_scoop$1$sr$ef1cf0bec3634d609ce2c73d3d2ee69d1d6a7273a6c0315d9b2bf3764e4974f0+0x20>
100047fb8:     	add	x9, x9, #0xb30
100047fbc:     	sub	x0, x29, #0xb0
100047fc0:     	sub	x1, x29, #0xd0
100047fc4:     	mov	w2, #0x2                ; =2
100047fc8:     	stp	x8, x9, [x29, #-0xc0]
100047fcc:     	str	q0, [x25, #0x130]
100047fd0:     	stur	xzr, [x29, #-0xa0]
100047fd4:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100047fd8:     	movi.2d	v0, #0000000000000000
100047fdc:     	sub	x0, x29, #0x98
100047fe0:     	sub	x1, x29, #0x98
100047fe4:     	stur	q0, [x29, #-0x98]
100047fe8:     	stur	q0, [x29, #-0x88]
100047fec:     	stur	q0, [x29, #-0x78]
100047ff0:     	stur	q0, [x29, #-0x68]
100047ff4:     	bl	0x100059194 <_scoop_rt_enter_native_borrowed>
100047ff8:     	ldr	x0, [sp, #0x20]
100047ffc:     	bl	0x100069fb8 <_m34_container_layout>
100048000:     	sub	x0, x29, #0x98
100048004:     	bl	0x1000647dc <_scoop_rt_leave_native_borrowed>
100048008:     	ldr	x19, [sp, #0x40]
10004800c:     	ldr	x20, [sp, #0x20]
100048010:     	sub	x0, x29, #0xb0
100048014:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100048018:     	str	x19, [sp, #0x40]
10004801c:     	str	x20, [sp, #0x20]
100048020:     	str	x19, [sp, #0x18]
100048024:     	bl	0x1000590a4 <_scoop_rt_gc_collect>
100048028:     	ldr	x8, [sp, #0x18]
10004802c:     	mov	x19, xzr
100048030:     	mov	w26, wzr
100048034:     	adrp	x27, 0x1001a0000 <_scoop$1$sr$ef1cf0bec3634d609ce2c73d3d2ee69d1d6a7273a6c0315d9b2bf3764e4974f0+0x20>
100048038:     	add	x27, x27, #0xb00
10004803c:     	adrp	x28, 0x1001a0000 <_scoop$1$sr$ef1cf0bec3634d609ce2c73d3d2ee69d1d6a7273a6c0315d9b2bf3764e4974f0+0x20>
100048040:     	add	x28, x28, #0xb10
100048044:     	str	x8, [sp, #0x40]
100048048:     	ldar	x8, [x22]
10004804c:     	ldar	w9, [x24]
100048050:     	add	x11, x23, #0x8
100048054:     	ldar	w10, [x23]
100048058:     	ldar	x11, [x11]
10004805c:     	cmp	w9, #0x0
100048060:     	ccmp	w10, #0x1, #0x0, eq
100048064:     	ccmp	x11, x8, #0x0, eq
100048068:     	b.eq	0x100048080 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x3b0>
10004806c:     	ldr	x8, [sp, #0x40]
100048070:     	str	x8, [sp, #0x18]
100048074:     	bl	0x100059084 <_scoop_rt_safepoint>
100048078:     	ldr	x8, [sp, #0x18]
10004807c:     	str	x8, [sp, #0x40]
100048080:     	cmp	w26, #0x5
100048084:     	b.ge	0x100048190 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x4c0>
100048088:     	movi.2d	v0, #0000000000000000
10004808c:     	add	x0, sp, #0x120
100048090:     	add	x1, sp, #0x110
100048094:     	mov	w2, #0x1                ; =1
100048098:     	stp	x21, x27, [sp, #0x110]
10004809c:     	str	xzr, [sp, #0x130]
1000480a0:     	str	q0, [x25, #0x50]
1000480a4:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
1000480a8:     	movi.2d	v0, #0000000000000000
1000480ac:     	add	x8, sp, #0x39
1000480b0:     	add	x0, sp, #0x138
1000480b4:     	add	x1, sp, #0x138
1000480b8:     	stur	q0, [x8, #0xff]
1000480bc:     	add	x8, sp, #0x49
1000480c0:     	stur	q0, [x8, #0xff]
1000480c4:     	add	x8, sp, #0x59
1000480c8:     	stur	q0, [x8, #0xff]
1000480cc:     	add	x8, sp, #0x69
1000480d0:     	stur	q0, [x8, #0xff]
1000480d4:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
1000480d8:     	bl	0x100069e44 <_m34_container_begin>
1000480dc:     	add	x0, sp, #0x138
1000480e0:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
1000480e4:     	ldr	x20, [sp, #0x40]
1000480e8:     	add	x0, sp, #0x120
1000480ec:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
1000480f0:     	str	x20, [sp, #0x40]
1000480f4:     	str	x20, [sp, #0x18]
1000480f8:     	bl	0x1000590a4 <_scoop_rt_gc_collect>
1000480fc:     	ldr	x8, [sp, #0x18]
100048100:     	movi.2d	v0, #0000000000000000
100048104:     	str	x8, [sp, #0x40]
100048108:     	add	x8, sp, #0x89
10004810c:     	add	x0, sp, #0x188
100048110:     	add	x1, sp, #0x178
100048114:     	mov	w2, #0x1                ; =1
100048118:     	stp	x21, x28, [sp, #0x178]
10004811c:     	str	xzr, [sp, #0x198]
100048120:     	stur	q0, [x8, #0xff]
100048124:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100048128:     	movi.2d	v0, #0000000000000000
10004812c:     	add	x0, sp, #0x1a0
100048130:     	add	x1, sp, #0x1a0
100048134:     	stp	q0, q0, [x25, #0xd0]
100048138:     	stp	q0, q0, [x25, #0xf0]
10004813c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100048140:     	mov	w0, #0x4                ; =4
100048144:     	bl	0x100069ebc <_m34_container_end>
100048148:     	add	x0, sp, #0x1a0
10004814c:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100048150:     	ldr	x20, [sp, #0x40]
100048154:     	add	x0, sp, #0x188
100048158:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004815c:     	mov	x0, x20
100048160:     	mov	x1, xzr
100048164:     	str	x20, [sp, #0x40]
100048168:     	str	x20, [sp, #0x30]
10004816c:     	str	x20, [sp, #0x18]
100048170:     	bl	0x10006b7a0 <dyld_stub_binder+0x10006b7a0>
100048174:     	ldr	x8, [sp, #0x18]
100048178:     	str	x8, [sp, #0x40]
10004817c:     	str	x8, [sp, #0x30]
100048180:     	add	w26, w26, #0x1
100048184:     	ldrsw	x8, [x0, #0x10]
100048188:     	add	x19, x19, x8
10004818c:     	b	0x100048048 <_scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x378>
100048190:     	cmp	x19, #0x23
100048194:     	cset	w0, eq
100048198:     	bl	0x10003f560 <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
10004819c:     	mov	x0, x19
1000481a0:     	add	sp, sp, #0x260
1000481a4:     	ldp	x29, x30, [sp, #0x50]
1000481a8:     	ldp	x20, x19, [sp, #0x40]
1000481ac:     	ldp	x22, x21, [sp, #0x30]
1000481b0:     	ldp	x24, x23, [sp, #0x20]
1000481b4:     	ldp	x26, x25, [sp, #0x10]
1000481b8:     	ldp	x28, x27, [sp], #0x60
1000481bc:     	ret

; buildStrings, MIR fn2, LIR scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-off-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010003aad8 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc>:
10003aad8:     	stp	x28, x27, [sp, #-0x60]!
10003aadc:     	stp	x26, x25, [sp, #0x10]
10003aae0:     	stp	x24, x23, [sp, #0x20]
10003aae4:     	stp	x22, x21, [sp, #0x30]
10003aae8:     	stp	x20, x19, [sp, #0x40]
10003aaec:     	stp	x29, x30, [sp, #0x50]
10003aaf0:     	add	x29, sp, #0x50
10003aaf4:     	sub	sp, sp, #0x1f0
10003aaf8:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
10003aafc:     	mov	x19, x0
10003ab00:     	add	x8, x8, #0x5d0
10003ab04:     	ldr	x9, [x8]
10003ab08:     	mov	x0, x8
10003ab0c:     	blr	x9
10003ab10:     	adrp	x22, 0x10020d000 <_scoop_gc_marker+0x800>
10003ab14:     	adrp	x24, 0x10020d000 <_scoop_gc_marker+0x800>
10003ab18:     	add	x22, x22, #0xa10
10003ab1c:     	ldr	x23, [x0]
10003ab20:     	add	x24, x24, #0x9fc
10003ab24:     	add	x10, x23, #0x8
10003ab28:     	ldar	x8, [x22]
10003ab2c:     	ldar	w11, [x24]
10003ab30:     	ldar	w9, [x23]
10003ab34:     	ldar	x10, [x10]
10003ab38:     	cbnz	w11, 0x10003ab48 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x70>
10003ab3c:     	cmp	w9, #0x1
10003ab40:     	ccmp	x10, x8, #0x0, eq
10003ab44:     	b.eq	0x10003ab4c <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x74>
10003ab48:     	bl	0x100059084 <_scoop_rt_safepoint>
10003ab4c:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
10003ab50:     	adrp	x10, 0x1000a9000 <_scoop$1$td$c05666274d0b373cd42e943443551d5b50c7f39941c56eeef65a2115f19699e8+0x70>
10003ab54:     	add	x0, x0, #0x5b8
10003ab58:     	add	x10, x10, #0xcb0
10003ab5c:     	ldr	x8, [x0]
10003ab60:     	ldr	x9, [x10, #0x18]
10003ab64:     	blr	x8
10003ab68:     	ldr	x8, [x0]
10003ab6c:     	neg	x11, x9
10003ab70:     	ldr	x12, [x8]
10003ab74:     	add	x12, x9, x12
10003ab78:     	sub	x12, x12, #0x1
10003ab7c:     	ands	x20, x12, x11
10003ab80:     	b.eq	0x10003abdc <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
10003ab84:     	add	x12, x9, #0x17
10003ab88:     	and	x2, x12, x11
10003ab8c:     	mov	w11, #0x7f80            ; =32640
10003ab90:     	cmp	x2, x11
10003ab94:     	b.hi	0x10003abdc <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
10003ab98:     	ldr	x10, [x10, #0x90]
10003ab9c:     	cbnz	x10, 0x10003abdc <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
10003aba0:     	ldr	x11, [x8, #0x8]
10003aba4:     	add	x10, x20, x2
10003aba8:     	cmp	x10, x11
10003abac:     	b.hi	0x10003abdc <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
10003abb0:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10003abb4:     	add	x9, x9, x11
10003abb8:     	cmn	x9, #0x19
10003abbc:     	b.hi	0x10003abdc <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
10003abc0:     	str	x10, [x8]
10003abc4:     	adrp	x1, 0x1000a9000 <_scoop$1$td$c05666274d0b373cd42e943443551d5b50c7f39941c56eeef65a2115f19699e8+0x70>
10003abc8:     	mov	x0, x20
10003abcc:     	add	x1, x1, #0xcb0
10003abd0:     	bl	0x100054aac <_scoop_runtime_finish_tlab_alloc>
10003abd4:     	mov	x0, x20
10003abd8:     	b	0x10003abec <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x114>
10003abdc:     	adrp	x0, 0x1000a9000 <_scoop$1$td$c05666274d0b373cd42e943443551d5b50c7f39941c56eeef65a2115f19699e8+0x70>
10003abe0:     	mov	w1, #0x18               ; =24
10003abe4:     	add	x0, x0, #0xcb0
10003abe8:     	bl	0x100059094 <_scoop_runtime_alloc_slow>
10003abec:     	add	x21, sp, #0xe0
10003abf0:     	str	x0, [sp, #0x10]
10003abf4:     	bl	0x1000175f0 <_scoop$1$cb$7fd0de94188adc1500f086a2b0e914cb1973f941c791324bcba453f6b7bbfd25>
10003abf8:     	ldr	x8, [sp, #0x10]
10003abfc:     	movi.2d	v0, #0000000000000000
10003ac00:     	str	x8, [sp, #0x48]
10003ac04:     	adrp	x9, 0x10017b000 <_scoop$1$sr$9ee6f3f6f31956ed99a5d6e863c4488e5f35f93bff583ad422d42f7db7052af3+0x40>
10003ac08:     	add	x9, x9, #0xa30
10003ac0c:     	str	x8, [sp, #0x40]
10003ac10:     	add	x8, sp, #0x40
10003ac14:     	add	x0, sp, #0x60
10003ac18:     	add	x1, sp, #0x50
10003ac1c:     	mov	w2, #0x1                ; =1
10003ac20:     	stp	x8, x9, [sp, #0x50]
10003ac24:     	str	q0, [sp, #0x60]
10003ac28:     	str	xzr, [sp, #0x70]
10003ac2c:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003ac30:     	movi.2d	v0, #0000000000000000
10003ac34:     	add	x0, sp, #0x78
10003ac38:     	add	x1, sp, #0x78
10003ac3c:     	stur	q0, [sp, #0x78]
10003ac40:     	stur	q0, [sp, #0x88]
10003ac44:     	stur	q0, [sp, #0x98]
10003ac48:     	stur	q0, [sp, #0xa8]
10003ac4c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10003ac50:     	bl	0x100069e44 <_m34_container_begin>
10003ac54:     	add	x0, sp, #0x78
10003ac58:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10003ac5c:     	ldr	x26, [sp, #0x40]
10003ac60:     	add	x0, sp, #0x60
10003ac64:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003ac68:     	mov	x25, xzr
10003ac6c:     	mov	x20, xzr
10003ac70:     	str	x26, [sp, #0x40]
10003ac74:     	ldar	x8, [x22]
10003ac78:     	ldar	w9, [x24]
10003ac7c:     	add	x11, x23, #0x8
10003ac80:     	ldar	w10, [x23]
10003ac84:     	ldar	x11, [x11]
10003ac88:     	cmp	w9, #0x0
10003ac8c:     	ccmp	w10, #0x1, #0x0, eq
10003ac90:     	ccmp	x11, x8, #0x0, eq
10003ac94:     	b.eq	0x10003acac <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x1d4>
10003ac98:     	ldr	x8, [sp, #0x40]
10003ac9c:     	str	x8, [sp, #0x10]
10003aca0:     	bl	0x100059084 <_scoop_rt_safepoint>
10003aca4:     	ldr	x8, [sp, #0x10]
10003aca8:     	str	x8, [sp, #0x40]
10003acac:     	cmp	x20, x19
10003acb0:     	b.ge	0x10003ad18 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x240>
10003acb4:     	ldr	x8, [sp, #0x40]
10003acb8:     	mov	x0, x20
10003acbc:     	str	x8, [sp, #0x10]
10003acc0:     	bl	0x10000a6f0 <_scoop$1$cb$ef730a437b587319fa729136c1c496c97d793c073731fe4cbfd67870ec8e2cc0>
10003acc4:     	ldr	x8, [sp, #0x10]
10003acc8:     	mov	x1, x0
10003accc:     	str	x8, [sp, #0x40]
10003acd0:     	stp	x8, x0, [sp, #0x20]
10003acd4:     	str	x0, [sp, #0x18]
10003acd8:     	str	x0, [sp, #0x8]
10003acdc:     	mov	x0, x8
10003ace0:     	bl	0x100009530 <_scoop$1$cb$bf2a768a0cb1375f89a63ed498891d7c57dab520218a10fddd14a0197c926150>
10003ace4:     	ldp	x0, x8, [sp, #0x8]
10003ace8:     	str	x8, [sp, #0x40]
10003acec:     	str	x0, [sp, #0x28]
10003acf0:     	str	x8, [sp, #0x20]
10003acf4:     	str	x0, [sp, #0x18]
10003acf8:     	str	x8, [sp, #0x10]
10003acfc:     	bl	0x100017f4c <_scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
10003ad00:     	ldp	x9, x8, [sp, #0x8]
10003ad04:     	str	x8, [sp, #0x40]
10003ad08:     	str	x9, [sp, #0x28]
10003ad0c:     	add	x25, x25, x0
10003ad10:     	add	x20, x20, #0x1
10003ad14:     	b	0x10003ac74 <_scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x19c>
10003ad18:     	movi.2d	v0, #0000000000000000
10003ad1c:     	add	x19, sp, #0x40
10003ad20:     	adrp	x8, 0x10017b000 <_scoop$1$sr$9ee6f3f6f31956ed99a5d6e863c4488e5f35f93bff583ad422d42f7db7052af3+0x40>
10003ad24:     	add	x8, x8, #0xa40
10003ad28:     	add	x0, sp, #0xc8
10003ad2c:     	add	x1, sp, #0xb8
10003ad30:     	mov	w2, #0x1                ; =1
10003ad34:     	stp	x19, x8, [sp, #0xb8]
10003ad38:     	str	xzr, [sp, #0xd8]
10003ad3c:     	stur	q0, [sp, #0xc8]
10003ad40:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003ad44:     	movi.2d	v0, #0000000000000000
10003ad48:     	add	x0, sp, #0xe0
10003ad4c:     	add	x1, sp, #0xe0
10003ad50:     	stp	q0, q0, [x21]
10003ad54:     	stp	q0, q0, [x21, #0x20]
10003ad58:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10003ad5c:     	mov	w0, #0x1                ; =1
10003ad60:     	bl	0x100069ebc <_m34_container_end>
10003ad64:     	add	x0, sp, #0xe0
10003ad68:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10003ad6c:     	ldr	x20, [sp, #0x40]
10003ad70:     	add	x0, sp, #0xc8
10003ad74:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003ad78:     	movi.2d	v0, #0000000000000000
10003ad7c:     	adrp	x8, 0x10017b000 <_scoop$1$sr$9ee6f3f6f31956ed99a5d6e863c4488e5f35f93bff583ad422d42f7db7052af3+0x40>
10003ad80:     	add	x8, x8, #0xa50
10003ad84:     	add	x0, sp, #0x130
10003ad88:     	add	x1, sp, #0x120
10003ad8c:     	mov	w2, #0x1                ; =1
10003ad90:     	str	x20, [sp, #0x40]
10003ad94:     	stp	x19, x8, [sp, #0x120]
10003ad98:     	str	q0, [x21, #0x50]
10003ad9c:     	str	xzr, [sp, #0x140]
10003ada0:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003ada4:     	movi.2d	v0, #0000000000000000
10003ada8:     	sub	x0, x29, #0xf8
10003adac:     	sub	x1, x29, #0xf8
10003adb0:     	stur	q0, [x29, #-0xf8]
10003adb4:     	stur	q0, [x29, #-0xe8]
10003adb8:     	stur	q0, [x29, #-0xd8]
10003adbc:     	stur	q0, [x29, #-0xc8]
10003adc0:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10003adc4:     	bl	0x100069e44 <_m34_container_begin>
10003adc8:     	sub	x0, x29, #0xf8
10003adcc:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10003add0:     	ldr	x19, [sp, #0x40]
10003add4:     	add	x0, sp, #0x130
10003add8:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003addc:     	mov	x0, x19
10003ade0:     	str	x19, [sp, #0x40]
10003ade4:     	str	x19, [sp, #0x10]
10003ade8:     	bl	0x100003b3c <_scoop$1$cb$22ffc90119bb7e713d02ab7beca1a040fd100be545734e1a344db51e6165d4ce>
10003adec:     	ldr	x8, [sp, #0x10]
10003adf0:     	movi.2d	v0, #0000000000000000
10003adf4:     	str	x8, [sp, #0x38]
10003adf8:     	add	x8, sp, #0x30
10003adfc:     	str	x0, [sp, #0x30]
10003ae00:     	adrp	x9, 0x10017b000 <_scoop$1$sr$9ee6f3f6f31956ed99a5d6e863c4488e5f35f93bff583ad422d42f7db7052af3+0x40>
10003ae04:     	add	x9, x9, #0xa60
10003ae08:     	sub	x0, x29, #0xa8
10003ae0c:     	sub	x1, x29, #0xb8
10003ae10:     	mov	w2, #0x1                ; =1
10003ae14:     	stp	x8, x9, [x29, #-0xb8]
10003ae18:     	stur	q0, [x29, #-0xa8]
10003ae1c:     	stur	xzr, [x29, #-0x98]
10003ae20:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003ae24:     	movi.2d	v0, #0000000000000000
10003ae28:     	sub	x0, x29, #0x90
10003ae2c:     	sub	x1, x29, #0x90
10003ae30:     	stp	q0, q0, [x21, #0xd0]
10003ae34:     	stp	q0, q0, [x21, #0xf0]
10003ae38:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10003ae3c:     	mov	w0, #0x2                ; =2
10003ae40:     	bl	0x100069ebc <_m34_container_end>
10003ae44:     	sub	x0, x29, #0x90
10003ae48:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10003ae4c:     	ldr	x19, [sp, #0x30]
10003ae50:     	sub	x0, x29, #0xa8
10003ae54:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003ae58:     	mov	x0, x19
10003ae5c:     	str	x19, [sp, #0x30]
10003ae60:     	str	x19, [sp, #0x10]
10003ae64:     	bl	0x100017f4c <_scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
10003ae68:     	ldr	x8, [sp, #0x10]
10003ae6c:     	cmp	x0, x25
10003ae70:     	str	x8, [sp, #0x30]
10003ae74:     	cset	w0, eq
10003ae78:     	bl	0x10003f560 <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
10003ae7c:     	ldr	x0, [sp, #0x10]
10003ae80:     	str	x0, [sp, #0x30]
10003ae84:     	bl	0x100017f4c <_scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
10003ae88:     	ldr	x8, [sp, #0x10]
10003ae8c:     	str	x8, [sp, #0x30]
10003ae90:     	add	sp, sp, #0x1f0
10003ae94:     	ldp	x29, x30, [sp, #0x50]
10003ae98:     	ldp	x20, x19, [sp, #0x40]
10003ae9c:     	ldp	x22, x21, [sp, #0x30]
10003aea0:     	ldp	x24, x23, [sp, #0x20]
10003aea4:     	ldp	x26, x25, [sp, #0x10]
10003aea8:     	ldp	x28, x27, [sp], #0x60
10003aeac:     	ret

; jsonRoundTrip, MIR fn3, LIR scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-off-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100042aec <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364>:
100042aec:     	stp	x28, x27, [sp, #-0x60]!
100042af0:     	stp	x26, x25, [sp, #0x10]
100042af4:     	stp	x24, x23, [sp, #0x20]
100042af8:     	stp	x22, x21, [sp, #0x30]
100042afc:     	stp	x20, x19, [sp, #0x40]
100042b00:     	stp	x29, x30, [sp, #0x50]
100042b04:     	add	x29, sp, #0x50
100042b08:     	sub	sp, sp, #0x3e0
100042b0c:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
100042b10:     	mov	x19, x0
100042b14:     	add	x8, x8, #0x5d0
100042b18:     	ldr	x9, [x8]
100042b1c:     	mov	x0, x8
100042b20:     	blr	x9
100042b24:     	adrp	x22, 0x10020d000 <_scoop_gc_marker+0x800>
100042b28:     	adrp	x24, 0x10020d000 <_scoop_gc_marker+0x800>
100042b2c:     	add	x22, x22, #0xa10
100042b30:     	ldr	x23, [x0]
100042b34:     	add	x24, x24, #0x9fc
100042b38:     	add	x10, x23, #0x8
100042b3c:     	ldar	x8, [x22]
100042b40:     	ldar	w11, [x24]
100042b44:     	ldar	w9, [x23]
100042b48:     	ldar	x10, [x10]
100042b4c:     	cbnz	w11, 0x100042b5c <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x70>
100042b50:     	cmp	w9, #0x1
100042b54:     	ccmp	x10, x8, #0x0, eq
100042b58:     	b.eq	0x100042b60 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x74>
100042b5c:     	bl	0x100059084 <_scoop_rt_safepoint>
100042b60:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
100042b64:     	adrp	x10, 0x100208000 <dyld_stub_binder+0x100208000>
100042b68:     	add	x21, sp, #0x120
100042b6c:     	add	x0, x0, #0x5b8
100042b70:     	ldr	x10, [x10, #0xb90]
100042b74:     	ldr	x8, [x0]
100042b78:     	ldr	x9, [x10, #0x18]
100042b7c:     	blr	x8
100042b80:     	ldr	x8, [x0]
100042b84:     	neg	x11, x9
100042b88:     	ldr	x12, [x8]
100042b8c:     	add	x12, x9, x12
100042b90:     	sub	x12, x12, #0x1
100042b94:     	ands	x20, x12, x11
100042b98:     	b.eq	0x100042bf4 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
100042b9c:     	add	x12, x9, #0x1f
100042ba0:     	and	x2, x12, x11
100042ba4:     	mov	w11, #0x7f80            ; =32640
100042ba8:     	cmp	x2, x11
100042bac:     	b.hi	0x100042bf4 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
100042bb0:     	ldr	x10, [x10, #0x90]
100042bb4:     	cbnz	x10, 0x100042bf4 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
100042bb8:     	ldr	x11, [x8, #0x8]
100042bbc:     	add	x10, x20, x2
100042bc0:     	cmp	x10, x11
100042bc4:     	b.hi	0x100042bf4 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
100042bc8:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
100042bcc:     	add	x9, x9, x11
100042bd0:     	cmn	x9, #0x21
100042bd4:     	b.hi	0x100042bf4 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
100042bd8:     	str	x10, [x8]
100042bdc:     	adrp	x1, 0x100208000 <dyld_stub_binder+0x100208000>
100042be0:     	mov	x0, x20
100042be4:     	ldr	x1, [x1, #0xb90]
100042be8:     	bl	0x100054aac <_scoop_runtime_finish_tlab_alloc>
100042bec:     	mov	x0, x20
100042bf0:     	b	0x100042c04 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x118>
100042bf4:     	adrp	x0, 0x100208000 <dyld_stub_binder+0x100208000>
100042bf8:     	mov	w1, #0x20               ; =32
100042bfc:     	ldr	x0, [x0, #0xb90]
100042c00:     	bl	0x100059094 <_scoop_runtime_alloc_slow>
100042c04:     	mov	x1, x19
100042c08:     	str	x0, [sp, #0x28]
100042c0c:     	bl	0x10006baac <dyld_stub_binder+0x10006baac>
100042c10:     	ldr	x8, [sp, #0x28]
100042c14:     	movi.2d	v0, #0000000000000000
100042c18:     	str	x8, [sp, #0xe0]
100042c1c:     	add	x20, sp, #0xd8
100042c20:     	stp	x8, x8, [sp, #0xd0]
100042c24:     	adrp	x8, 0x100192000 <_scoop$1$tr$a592a94a0cdd268c14be8d23ed186c91f7a4b23c9d349ecf9975d71d0c7d31d2+0x40>
100042c28:     	add	x8, x8, #0xa70
100042c2c:     	stp	x20, x8, [sp, #0xe8]
100042c30:     	add	x8, sp, #0xd0
100042c34:     	adrp	x9, 0x100192000 <_scoop$1$tr$a592a94a0cdd268c14be8d23ed186c91f7a4b23c9d349ecf9975d71d0c7d31d2+0x40>
100042c38:     	add	x9, x9, #0xa80
100042c3c:     	add	x0, sp, #0x108
100042c40:     	add	x1, sp, #0xe8
100042c44:     	stp	x8, x9, [sp, #0xf8]
100042c48:     	add	x8, sp, #0x9
100042c4c:     	mov	w2, #0x2                ; =2
100042c50:     	stur	q0, [x8, #0xff]
100042c54:     	str	xzr, [sp, #0x118]
100042c58:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100042c5c:     	movi.2d	v0, #0000000000000000
100042c60:     	add	x0, sp, #0x120
100042c64:     	add	x1, sp, #0x120
100042c68:     	stp	q0, q0, [x21]
100042c6c:     	stp	q0, q0, [x21, #0x20]
100042c70:     	bl	0x100059194 <_scoop_rt_enter_native_borrowed>
100042c74:     	ldr	x0, [sp, #0xd0]
100042c78:     	bl	0x100069fb8 <_m34_container_layout>
100042c7c:     	add	x0, sp, #0x120
100042c80:     	bl	0x1000647dc <_scoop_rt_leave_native_borrowed>
100042c84:     	ldr	x25, [sp, #0xd8]
100042c88:     	ldr	x26, [sp, #0xd0]
100042c8c:     	add	x0, sp, #0x108
100042c90:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100042c94:     	movi.2d	v0, #0000000000000000
100042c98:     	adrp	x8, 0x100192000 <_scoop$1$tr$a592a94a0cdd268c14be8d23ed186c91f7a4b23c9d349ecf9975d71d0c7d31d2+0x40>
100042c9c:     	add	x8, x8, #0xa90
100042ca0:     	add	x0, sp, #0x170
100042ca4:     	add	x1, sp, #0x160
100042ca8:     	mov	w2, #0x1                ; =1
100042cac:     	str	x25, [sp, #0xd8]
100042cb0:     	str	x26, [sp, #0xd0]
100042cb4:     	stp	x20, x8, [sp, #0x160]
100042cb8:     	str	q0, [x21, #0x50]
100042cbc:     	str	xzr, [sp, #0x180]
100042cc0:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100042cc4:     	movi.2d	v0, #0000000000000000
100042cc8:     	add	x8, sp, #0x89
100042ccc:     	add	x0, sp, #0x188
100042cd0:     	add	x1, sp, #0x188
100042cd4:     	stur	q0, [x8, #0xff]
100042cd8:     	add	x8, sp, #0x99
100042cdc:     	stur	q0, [x8, #0xff]
100042ce0:     	add	x8, sp, #0xa9
100042ce4:     	stur	q0, [x8, #0xff]
100042ce8:     	add	x8, sp, #0xb9
100042cec:     	stur	q0, [x8, #0xff]
100042cf0:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100042cf4:     	bl	0x100069e44 <_m34_container_begin>
100042cf8:     	add	x0, sp, #0x188
100042cfc:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100042d00:     	ldr	x25, [sp, #0xd8]
100042d04:     	add	x0, sp, #0x170
100042d08:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100042d0c:     	mov	x20, xzr
100042d10:     	str	x25, [sp, #0xd8]
100042d14:     	ldar	x8, [x22]
100042d18:     	ldar	w9, [x24]
100042d1c:     	add	x11, x23, #0x8
100042d20:     	ldar	w10, [x23]
100042d24:     	ldar	x11, [x11]
100042d28:     	cmp	w9, #0x0
100042d2c:     	ccmp	w10, #0x1, #0x0, eq
100042d30:     	ccmp	x11, x8, #0x0, eq
100042d34:     	b.eq	0x100042d4c <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x260>
100042d38:     	ldr	x8, [sp, #0xd8]
100042d3c:     	str	x8, [sp, #0x28]
100042d40:     	bl	0x100059084 <_scoop_rt_safepoint>
100042d44:     	ldr	x8, [sp, #0x28]
100042d48:     	str	x8, [sp, #0xd8]
100042d4c:     	cmp	x20, x19
100042d50:     	b.ge	0x100042d7c <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x290>
100042d54:     	ldr	x0, [sp, #0xd8]
100042d58:     	mov	w1, w20
100042d5c:     	str	x0, [sp, #0x38]
100042d60:     	str	x0, [sp, #0x28]
100042d64:     	bl	0x10006b89c <dyld_stub_binder+0x10006b89c>
100042d68:     	ldr	x8, [sp, #0x28]
100042d6c:     	str	x8, [sp, #0xd8]
100042d70:     	str	x8, [sp, #0x38]
100042d74:     	add	x20, x20, #0x1
100042d78:     	b	0x100042d14 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x228>
100042d7c:     	movi.2d	v0, #0000000000000000
100042d80:     	add	x20, sp, #0xd8
100042d84:     	adrp	x8, 0x100192000 <_scoop$1$tr$a592a94a0cdd268c14be8d23ed186c91f7a4b23c9d349ecf9975d71d0c7d31d2+0x40>
100042d88:     	add	x8, x8, #0xaa0
100042d8c:     	add	x0, sp, #0x1d8
100042d90:     	add	x1, sp, #0x1c8
100042d94:     	stp	x20, x8, [sp, #0x1c8]
100042d98:     	add	x8, sp, #0xd9
100042d9c:     	mov	w2, #0x1                ; =1
100042da0:     	str	xzr, [sp, #0x1e8]
100042da4:     	stur	q0, [x8, #0xff]
100042da8:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100042dac:     	movi.2d	v0, #0000000000000000
100042db0:     	add	x0, sp, #0x1f0
100042db4:     	add	x1, sp, #0x1f0
100042db8:     	stp	q0, q0, [x21, #0xd0]
100042dbc:     	stp	q0, q0, [x21, #0xf0]
100042dc0:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100042dc4:     	mov	w0, #0x1                ; =1
100042dc8:     	bl	0x100069ebc <_m34_container_end>
100042dcc:     	add	x0, sp, #0x1f0
100042dd0:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100042dd4:     	ldr	x22, [sp, #0xd8]
100042dd8:     	add	x0, sp, #0x1d8
100042ddc:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100042de0:     	movi.2d	v0, #0000000000000000
100042de4:     	adrp	x8, 0x100192000 <_scoop$1$tr$a592a94a0cdd268c14be8d23ed186c91f7a4b23c9d349ecf9975d71d0c7d31d2+0x40>
100042de8:     	add	x8, x8, #0xab0
100042dec:     	add	x0, sp, #0x240
100042df0:     	add	x1, sp, #0x230
100042df4:     	mov	w2, #0x1                ; =1
100042df8:     	str	x22, [sp, #0xd8]
100042dfc:     	str	x20, [sp, #0x230]
100042e00:     	str	x8, [sp, #0x238]
100042e04:     	str	q0, [x21, #0x120]
100042e08:     	str	xzr, [sp, #0x250]
100042e0c:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100042e10:     	movi.2d	v0, #0000000000000000
100042e14:     	add	x8, sp, #0x159
100042e18:     	add	x0, sp, #0x258
100042e1c:     	add	x1, sp, #0x258
100042e20:     	stur	q0, [x8, #0xff]
100042e24:     	add	x8, sp, #0x169
100042e28:     	stur	q0, [x8, #0xff]
100042e2c:     	add	x8, sp, #0x179
100042e30:     	stur	q0, [x8, #0xff]
100042e34:     	add	x8, sp, #0x189
100042e38:     	stur	q0, [x8, #0xff]
100042e3c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100042e40:     	bl	0x100069e44 <_m34_container_begin>
100042e44:     	add	x0, sp, #0x258
100042e48:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100042e4c:     	ldr	x20, [sp, #0xd8]
100042e50:     	add	x0, sp, #0x240
100042e54:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100042e58:     	str	x20, [sp, #0xd8]
100042e5c:     	str	x20, [sp, #0x28]
100042e60:     	bl	0x1000384c0 <_scoop$1$cb$0bded537c1012e7dc164ae08cf779ffaa1fbf40bc9bfe0e762282e5e75f642ac>
100042e64:     	ldr	x8, [sp, #0x28]
100042e68:     	adrp	x25, 0x10020d000 <_scoop_gc_marker+0x800>
100042e6c:     	add	x25, x25, #0x6f0
100042e70:     	str	x8, [sp, #0xd8]
100042e74:     	ldr	x8, [x25]
100042e78:     	str	x8, [sp, #0x20]
100042e7c:     	bl	0x10006b800 <dyld_stub_binder+0x10006b800>
100042e80:     	ldp	x8, x9, [sp, #0x20]
100042e84:     	str	x8, [sp, #0xc8]
100042e88:     	adrp	x24, 0x100208000 <dyld_stub_binder+0x100208000>
100042e8c:     	ldr	x24, [x24, #0xb30]
100042e90:     	str	x9, [sp, #0xc0]
100042e94:     	ldr	x8, [x24]
100042e98:     	str	x8, [sp, #0x18]
100042e9c:     	bl	0x10000e6e4 <_scoop$1$cb$5e012350ddc7cb238741da77cd1768afd61bf09fc92c1eb080f58c2c066df7f3>
100042ea0:     	ldp	x8, x10, [sp, #0x18]
100042ea4:     	ldr	x9, [sp, #0x28]
100042ea8:     	str	x8, [sp, #0xb8]
100042eac:     	str	x10, [sp, #0xc8]
100042eb0:     	adrp	x22, 0x10020d000 <_scoop_gc_marker+0x800>
100042eb4:     	adrp	x23, 0x1000a9000 <_scoop$1$td$c05666274d0b373cd42e943443551d5b50c7f39941c56eeef65a2115f19699e8+0x70>
100042eb8:     	add	x22, x22, #0x628
100042ebc:     	add	x23, x23, #0xfd0
100042ec0:     	str	x9, [sp, #0xc0]
100042ec4:     	ldr	x8, [x22]
100042ec8:     	ldr	x9, [x23, #0x60]
100042ecc:     	ldr	x2, [x9, #0x18]
100042ed0:     	str	x8, [sp, #0xb0]
100042ed4:     	mov	x8, x10
100042ed8:     	ldr	x1, [sp, #0xb0]
100042edc:     	ldp	x0, x9, [sp, #0xb8]
100042ee0:     	stp	x1, x0, [sp, #0x20]
100042ee4:     	stp	x9, x8, [sp, #0x10]
100042ee8:     	bl	0x10006b9e0 <dyld_stub_binder+0x10006b9e0>
100042eec:     	ldp	x11, x9, [sp, #0x20]
100042ef0:     	mov	x3, x1
100042ef4:     	ldp	x8, x10, [sp, #0x10]
100042ef8:     	str	x9, [sp, #0xb8]
100042efc:     	str	x11, [sp, #0xa8]
100042f00:     	str	x10, [sp, #0xc8]
100042f04:     	str	x8, [sp, #0xc0]
100042f08:     	mov	x1, x8
100042f0c:     	str	x0, [sp, #0xa0]
100042f10:     	mov	x0, x10
100042f14:     	ldr	x2, [sp, #0xa0]
100042f18:     	stp	x2, x0, [sp, #0x20]
100042f1c:     	bl	0x10006ba34 <dyld_stub_binder+0x10006ba34>
100042f20:     	ldp	x9, x8, [sp, #0x20]
100042f24:     	ldr	x10, [sp, #0x10]
100042f28:     	str	x8, [sp, #0xc8]
100042f2c:     	str	x10, [sp, #0x98]
100042f30:     	movi.2d	v0, #0000000000000000
100042f34:     	adrp	x8, 0x100192000 <_scoop$1$tr$a592a94a0cdd268c14be8d23ed186c91f7a4b23c9d349ecf9975d71d0c7d31d2+0x40>
100042f38:     	add	x8, x8, #0xac0
100042f3c:     	str	x9, [sp, #0x90]
100042f40:     	add	x20, sp, #0x88
100042f44:     	add	x1, sp, #0x298
100042f48:     	str	x0, [sp, #0x88]
100042f4c:     	add	x0, sp, #0x2a8
100042f50:     	mov	w2, #0x1                ; =1
100042f54:     	str	x8, [sp, #0x2a0]
100042f58:     	add	x8, sp, #0x1a9
100042f5c:     	str	x20, [sp, #0x298]
100042f60:     	stur	q0, [x8, #0xff]
100042f64:     	str	xzr, [sp, #0x2b8]
100042f68:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100042f6c:     	movi.2d	v0, #0000000000000000
100042f70:     	add	x0, sp, #0x2c0
100042f74:     	add	x1, sp, #0x2c0
100042f78:     	stp	q0, q0, [x21, #0x1a0]
100042f7c:     	stp	q0, q0, [x21, #0x1c0]
100042f80:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100042f84:     	mov	w0, #0x2                ; =2
100042f88:     	bl	0x100069ebc <_m34_container_end>
100042f8c:     	add	x0, sp, #0x2c0
100042f90:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100042f94:     	ldr	x26, [sp, #0x88]
100042f98:     	add	x0, sp, #0x2a8
100042f9c:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100042fa0:     	movi.2d	v0, #0000000000000000
100042fa4:     	adrp	x8, 0x100192000 <_scoop$1$tr$a592a94a0cdd268c14be8d23ed186c91f7a4b23c9d349ecf9975d71d0c7d31d2+0x40>
100042fa8:     	add	x8, x8, #0xad0
100042fac:     	add	x0, sp, #0x310
100042fb0:     	add	x1, sp, #0x300
100042fb4:     	mov	w2, #0x1                ; =1
100042fb8:     	str	x26, [sp, #0x88]
100042fbc:     	str	x20, [sp, #0x300]
100042fc0:     	str	x8, [sp, #0x308]
100042fc4:     	str	q0, [x21, #0x1f0]
100042fc8:     	str	xzr, [sp, #0x320]
100042fcc:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100042fd0:     	movi.2d	v0, #0000000000000000
100042fd4:     	add	x8, sp, #0x229
100042fd8:     	add	x0, sp, #0x328
100042fdc:     	add	x1, sp, #0x328
100042fe0:     	stur	q0, [x8, #0xff]
100042fe4:     	add	x8, sp, #0x239
100042fe8:     	stur	q0, [x8, #0xff]
100042fec:     	add	x8, sp, #0x249
100042ff0:     	stur	q0, [x8, #0xff]
100042ff4:     	add	x8, sp, #0x259
100042ff8:     	stur	q0, [x8, #0xff]
100042ffc:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100043000:     	bl	0x100069e44 <_m34_container_begin>
100043004:     	add	x0, sp, #0x328
100043008:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10004300c:     	ldr	x26, [sp, #0x88]
100043010:     	add	x0, sp, #0x310
100043014:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100043018:     	str	x26, [sp, #0x88]
10004301c:     	str	x26, [sp, #0x28]
100043020:     	bl	0x1000384c0 <_scoop$1$cb$0bded537c1012e7dc164ae08cf779ffaa1fbf40bc9bfe0e762282e5e75f642ac>
100043024:     	ldr	x8, [sp, #0x28]
100043028:     	str	x8, [sp, #0x88]
10004302c:     	ldr	x8, [x25]
100043030:     	str	x8, [sp, #0x20]
100043034:     	bl	0x10006b800 <dyld_stub_binder+0x10006b800>
100043038:     	ldp	x9, x8, [sp, #0x20]
10004303c:     	str	x8, [sp, #0x88]
100043040:     	str	x9, [sp, #0x80]
100043044:     	str	x8, [sp, #0x78]
100043048:     	ldr	x9, [x24]
10004304c:     	stp	x9, x8, [sp, #0x10]
100043050:     	bl	0x10000e6e4 <_scoop$1$cb$5e012350ddc7cb238741da77cd1768afd61bf09fc92c1eb080f58c2c066df7f3>
100043054:     	ldp	x8, x10, [sp, #0x18]
100043058:     	ldr	x9, [sp, #0x28]
10004305c:     	ldr	x11, [sp, #0x10]
100043060:     	str	x8, [sp, #0x88]
100043064:     	str	x11, [sp, #0x70]
100043068:     	str	x10, [sp, #0x80]
10004306c:     	str	x9, [sp, #0x78]
100043070:     	mov	x0, x11
100043074:     	ldr	x8, [x22]
100043078:     	ldr	x9, [x23, #0x60]
10004307c:     	ldr	x2, [x9, #0x8]
100043080:     	str	x8, [sp, #0x68]
100043084:     	mov	x9, x10
100043088:     	ldr	x1, [sp, #0x68]
10004308c:     	ldr	x8, [sp, #0x88]
100043090:     	ldr	x10, [sp, #0x78]
100043094:     	stp	x0, x8, [sp, #0x20]
100043098:     	stp	x10, x9, [sp, #0x8]
10004309c:     	str	x1, [sp, #0x18]
1000430a0:     	bl	0x10006b9c8 <dyld_stub_binder+0x10006b9c8>
1000430a4:     	ldp	x12, x9, [sp, #0x20]
1000430a8:     	mov	x3, x1
1000430ac:     	ldp	x8, x10, [sp, #0x8]
1000430b0:     	ldr	x11, [sp, #0x18]
1000430b4:     	str	x9, [sp, #0x88]
1000430b8:     	str	x12, [sp, #0x70]
1000430bc:     	str	x11, [sp, #0x60]
1000430c0:     	str	x10, [sp, #0x80]
1000430c4:     	str	x8, [sp, #0x78]
1000430c8:     	mov	x1, x8
1000430cc:     	str	x0, [sp, #0x58]
1000430d0:     	mov	x0, x10
1000430d4:     	ldr	x2, [sp, #0x58]
1000430d8:     	stp	x0, x9, [sp, #0x20]
1000430dc:     	str	x2, [sp, #0x18]
1000430e0:     	bl	0x10006b7b8 <dyld_stub_binder+0x10006b7b8>
1000430e4:     	ldp	x11, x8, [sp, #0x20]
1000430e8:     	ldr	x9, [sp, #0x18]
1000430ec:     	ldr	x10, [sp, #0x8]
1000430f0:     	str	x8, [sp, #0x88]
1000430f4:     	str	x11, [sp, #0x80]
1000430f8:     	str	x10, [sp, #0x50]
1000430fc:     	movi.2d	v0, #0000000000000000
100043100:     	adrp	x8, 0x100192000 <_scoop$1$tr$a592a94a0cdd268c14be8d23ed186c91f7a4b23c9d349ecf9975d71d0c7d31d2+0x40>
100043104:     	add	x8, x8, #0xae0
100043108:     	str	x9, [sp, #0x48]
10004310c:     	adrp	x9, 0x100192000 <_scoop$1$tr$a592a94a0cdd268c14be8d23ed186c91f7a4b23c9d349ecf9975d71d0c7d31d2+0x40>
100043110:     	add	x9, x9, #0xaf0
100043114:     	str	x0, [sp, #0x40]
100043118:     	sub	x0, x29, #0xa8
10004311c:     	sub	x1, x29, #0xc8
100043120:     	stp	x20, x8, [x29, #-0xc8]
100043124:     	add	x8, sp, #0x40
100043128:     	mov	w2, #0x2                ; =2
10004312c:     	stp	x8, x9, [x29, #-0xb8]
100043130:     	stur	q0, [x29, #-0xa8]
100043134:     	stur	xzr, [x29, #-0x98]
100043138:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004313c:     	movi.2d	v0, #0000000000000000
100043140:     	sub	x0, x29, #0x90
100043144:     	sub	x1, x29, #0x90
100043148:     	stp	q0, q0, [x21, #0x280]
10004314c:     	stp	q0, q0, [x21, #0x2a0]
100043150:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100043154:     	mov	w0, #0x3                ; =3
100043158:     	bl	0x100069ebc <_m34_container_end>
10004315c:     	sub	x0, x29, #0x90
100043160:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100043164:     	ldr	x21, [sp, #0x88]
100043168:     	ldr	x20, [sp, #0x40]
10004316c:     	sub	x0, x29, #0xa8
100043170:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100043174:     	str	x21, [sp, #0x88]
100043178:     	str	x20, [sp, #0x40]
10004317c:     	ldr	x8, [x20, #0x18]
100043180:     	cmp	x8, x19
100043184:     	b.ne	0x1000431bc <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x6d0>
100043188:     	ldr	x8, [sp, #0x88]
10004318c:     	sub	x1, x19, #0x1
100043190:     	mov	x0, x20
100043194:     	stp	x20, x8, [sp, #0x20]
100043198:     	bl	0x10006ba40 <dyld_stub_binder+0x10006ba40>
10004319c:     	ldp	x9, x8, [sp, #0x20]
1000431a0:     	str	x8, [sp, #0x88]
1000431a4:     	str	x9, [sp, #0x40]
1000431a8:     	sub	w8, w19, #0x1
1000431ac:     	str	x9, [sp, #0x30]
1000431b0:     	cmp	w0, w8
1000431b4:     	cset	w0, eq
1000431b8:     	b	0x1000431c0 <_scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x6d4>
1000431bc:     	mov	w0, wzr
1000431c0:     	ldr	x8, [sp, #0x88]
1000431c4:     	ldr	x9, [sp, #0x40]
1000431c8:     	stp	x9, x8, [sp, #0x20]
1000431cc:     	bl	0x10003f560 <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
1000431d0:     	ldp	x8, x0, [sp, #0x20]
1000431d4:     	str	x0, [sp, #0x88]
1000431d8:     	str	x8, [sp, #0x40]
1000431dc:     	bl	0x100017f4c <_scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
1000431e0:     	ldp	x9, x8, [sp, #0x20]
1000431e4:     	str	x8, [sp, #0x88]
1000431e8:     	str	x9, [sp, #0x40]
1000431ec:     	ldr	x8, [x9, #0x18]
1000431f0:     	add	x0, x0, x8
1000431f4:     	add	sp, sp, #0x3e0
1000431f8:     	ldp	x29, x30, [sp, #0x50]
1000431fc:     	ldp	x20, x19, [sp, #0x40]
100043200:     	ldp	x22, x21, [sp, #0x30]
100043204:     	ldp	x24, x23, [sp, #0x20]
100043208:     	ldp	x26, x25, [sp, #0x10]
10004320c:     	ldp	x28, x27, [sp], #0x60
100043210:     	ret

; scalarList, MIR fn4, LIR scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-off-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010004db60 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b>:
10004db60:     	stp	x28, x27, [sp, #-0x60]!
10004db64:     	stp	x26, x25, [sp, #0x10]
10004db68:     	stp	x24, x23, [sp, #0x20]
10004db6c:     	stp	x22, x21, [sp, #0x30]
10004db70:     	stp	x20, x19, [sp, #0x40]
10004db74:     	stp	x29, x30, [sp, #0x50]
10004db78:     	add	x29, sp, #0x50
10004db7c:     	sub	sp, sp, #0x400
10004db80:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
10004db84:     	mov	x19, x0
10004db88:     	add	x26, sp, #0x68
10004db8c:     	add	x8, x8, #0x5d0
10004db90:     	ldr	x9, [x8]
10004db94:     	mov	x0, x8
10004db98:     	blr	x9
10004db9c:     	adrp	x23, 0x10020d000 <_scoop_gc_marker+0x800>
10004dba0:     	adrp	x25, 0x10020d000 <_scoop_gc_marker+0x800>
10004dba4:     	add	x23, x23, #0xa10
10004dba8:     	ldr	x24, [x0]
10004dbac:     	add	x25, x25, #0x9fc
10004dbb0:     	add	x10, x24, #0x8
10004dbb4:     	ldar	x8, [x23]
10004dbb8:     	ldar	w11, [x25]
10004dbbc:     	ldar	w9, [x24]
10004dbc0:     	ldar	x10, [x10]
10004dbc4:     	cbnz	w11, 0x10004dbd4 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x74>
10004dbc8:     	cmp	w9, #0x1
10004dbcc:     	ccmp	x10, x8, #0x0, eq
10004dbd0:     	b.eq	0x10004dbd8 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x78>
10004dbd4:     	bl	0x100059084 <_scoop_rt_safepoint>
10004dbd8:     	movi.2d	v0, #0000000000000000
10004dbdc:     	add	x0, sp, #0x50
10004dbe0:     	mov	x1, xzr
10004dbe4:     	mov	x2, xzr
10004dbe8:     	str	xzr, [sp, #0x60]
10004dbec:     	str	q0, [sp, #0x50]
10004dbf0:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004dbf4:     	movi.2d	v0, #0000000000000000
10004dbf8:     	add	x0, sp, #0x68
10004dbfc:     	add	x1, sp, #0x68
10004dc00:     	stur	q0, [sp, #0x68]
10004dc04:     	stur	q0, [sp, #0x78]
10004dc08:     	stp	q0, q0, [x26, #0x20]
10004dc0c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10004dc10:     	bl	0x100069e44 <_m34_container_begin>
10004dc14:     	add	x0, sp, #0x68
10004dc18:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10004dc1c:     	add	x0, sp, #0x50
10004dc20:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004dc24:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
10004dc28:     	adrp	x10, 0x100208000 <dyld_stub_binder+0x100208000>
10004dc2c:     	add	x0, x0, #0x5b8
10004dc30:     	ldr	x10, [x10, #0xb90]
10004dc34:     	ldr	x8, [x0]
10004dc38:     	ldr	x9, [x10, #0x18]
10004dc3c:     	blr	x8
10004dc40:     	ldr	x8, [x0]
10004dc44:     	neg	x11, x9
10004dc48:     	ldr	x12, [x8]
10004dc4c:     	add	x12, x9, x12
10004dc50:     	sub	x12, x12, #0x1
10004dc54:     	ands	x20, x12, x11
10004dc58:     	b.eq	0x10004dcb4 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x154>
10004dc5c:     	add	x12, x9, #0x1f
10004dc60:     	and	x2, x12, x11
10004dc64:     	mov	w11, #0x7f80            ; =32640
10004dc68:     	cmp	x2, x11
10004dc6c:     	b.hi	0x10004dcb4 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x154>
10004dc70:     	ldr	x10, [x10, #0x90]
10004dc74:     	cbnz	x10, 0x10004dcb4 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x154>
10004dc78:     	ldr	x11, [x8, #0x8]
10004dc7c:     	add	x10, x20, x2
10004dc80:     	cmp	x10, x11
10004dc84:     	b.hi	0x10004dcb4 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x154>
10004dc88:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10004dc8c:     	add	x9, x9, x11
10004dc90:     	cmn	x9, #0x21
10004dc94:     	b.hi	0x10004dcb4 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x154>
10004dc98:     	str	x10, [x8]
10004dc9c:     	adrp	x1, 0x100208000 <dyld_stub_binder+0x100208000>
10004dca0:     	mov	x0, x20
10004dca4:     	ldr	x1, [x1, #0xb90]
10004dca8:     	bl	0x100054aac <_scoop_runtime_finish_tlab_alloc>
10004dcac:     	mov	x0, x20
10004dcb0:     	b	0x10004dcc4 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x164>
10004dcb4:     	adrp	x0, 0x100208000 <dyld_stub_binder+0x100208000>
10004dcb8:     	mov	w1, #0x20               ; =32
10004dcbc:     	ldr	x0, [x0, #0xb90]
10004dcc0:     	bl	0x100059094 <_scoop_runtime_alloc_slow>
10004dcc4:     	mov	x1, x19
10004dcc8:     	str	x0, [sp, #0x8]
10004dccc:     	bl	0x10006baac <dyld_stub_binder+0x10006baac>
10004dcd0:     	ldr	x8, [sp, #0x8]
10004dcd4:     	movi.2d	v0, #0000000000000000
10004dcd8:     	str	x8, [sp, #0x48]
10004dcdc:     	add	x20, sp, #0x40
10004dce0:     	str	x8, [sp, #0x40]
10004dce4:     	adrp	x8, 0x1001b1000 <_scoop$1$tr$a7bf7611636503acb7f3d62ef99aa1b0c731f87447cf9668cea4d36b0fd4e48b>
10004dce8:     	add	x8, x8, #0x860
10004dcec:     	add	x0, sp, #0xb8
10004dcf0:     	add	x1, sp, #0xa8
10004dcf4:     	mov	w2, #0x1                ; =1
10004dcf8:     	stp	x20, x8, [sp, #0xa8]
10004dcfc:     	str	q0, [x26, #0x50]
10004dd00:     	str	xzr, [sp, #0xc8]
10004dd04:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004dd08:     	movi.2d	v0, #0000000000000000
10004dd0c:     	add	x0, sp, #0xd0
10004dd10:     	add	x1, sp, #0xd0
10004dd14:     	stp	q0, q0, [sp, #0xd0]
10004dd18:     	stp	q0, q0, [sp, #0xf0]
10004dd1c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10004dd20:     	mov	w0, wzr
10004dd24:     	bl	0x100069ebc <_m34_container_end>
10004dd28:     	add	x0, sp, #0xd0
10004dd2c:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10004dd30:     	ldr	x21, [sp, #0x40]
10004dd34:     	add	x0, sp, #0xb8
10004dd38:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004dd3c:     	movi.2d	v0, #0000000000000000
10004dd40:     	adrp	x8, 0x1001b1000 <_scoop$1$tr$a7bf7611636503acb7f3d62ef99aa1b0c731f87447cf9668cea4d36b0fd4e48b>
10004dd44:     	add	x8, x8, #0x870
10004dd48:     	str	x21, [sp, #0x40]
10004dd4c:     	adrp	x9, 0x1001b1000 <_scoop$1$tr$a7bf7611636503acb7f3d62ef99aa1b0c731f87447cf9668cea4d36b0fd4e48b>
10004dd50:     	add	x9, x9, #0x880
10004dd54:     	stp	x20, x8, [sp, #0x110]
10004dd58:     	add	x8, sp, #0x38
10004dd5c:     	add	x0, sp, #0x130
10004dd60:     	add	x1, sp, #0x110
10004dd64:     	mov	w2, #0x2                ; =2
10004dd68:     	str	x21, [sp, #0x38]
10004dd6c:     	stp	x8, x9, [sp, #0x120]
10004dd70:     	str	q0, [sp, #0x130]
10004dd74:     	str	xzr, [sp, #0x140]
10004dd78:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004dd7c:     	movi.2d	v0, #0000000000000000
10004dd80:     	add	x0, sp, #0x148
10004dd84:     	add	x1, sp, #0x148
10004dd88:     	stp	q0, q0, [x26, #0xe0]
10004dd8c:     	stp	q0, q0, [x26, #0x100]
10004dd90:     	bl	0x100059194 <_scoop_rt_enter_native_borrowed>
10004dd94:     	ldr	x0, [sp, #0x38]
10004dd98:     	bl	0x100069fb8 <_m34_container_layout>
10004dd9c:     	add	x0, sp, #0x148
10004dda0:     	bl	0x1000647dc <_scoop_rt_leave_native_borrowed>
10004dda4:     	ldr	x21, [sp, #0x40]
10004dda8:     	ldr	x22, [sp, #0x38]
10004ddac:     	add	x0, sp, #0x130
10004ddb0:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004ddb4:     	movi.2d	v0, #0000000000000000
10004ddb8:     	adrp	x8, 0x1001b1000 <_scoop$1$tr$a7bf7611636503acb7f3d62ef99aa1b0c731f87447cf9668cea4d36b0fd4e48b>
10004ddbc:     	add	x8, x8, #0x890
10004ddc0:     	add	x0, sp, #0x198
10004ddc4:     	add	x1, sp, #0x188
10004ddc8:     	mov	w2, #0x1                ; =1
10004ddcc:     	str	x21, [sp, #0x40]
10004ddd0:     	str	x22, [sp, #0x38]
10004ddd4:     	stp	x20, x8, [sp, #0x188]
10004ddd8:     	str	q0, [x26, #0x130]
10004dddc:     	str	xzr, [sp, #0x1a8]
10004dde0:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004dde4:     	movi.2d	v0, #0000000000000000
10004dde8:     	add	x0, sp, #0x1b0
10004ddec:     	add	x1, sp, #0x1b0
10004ddf0:     	stp	q0, q0, [sp, #0x1b0]
10004ddf4:     	stp	q0, q0, [sp, #0x1d0]
10004ddf8:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10004ddfc:     	bl	0x100069e44 <_m34_container_begin>
10004de00:     	add	x0, sp, #0x1b0
10004de04:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10004de08:     	ldr	x21, [sp, #0x40]
10004de0c:     	add	x0, sp, #0x198
10004de10:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004de14:     	str	x21, [sp, #0x40]
10004de18:     	mov	x21, #0x51d1            ; =20945
10004de1c:     	mov	x28, #0xa8e9            ; =43241
10004de20:     	movk	x21, #0x2f81, lsl #16
10004de24:     	movk	x28, #0x17c0, lsl #16
10004de28:     	mov	x26, xzr
10004de2c:     	movk	x21, #0x7eae, lsl #32
10004de30:     	movk	x28, #0x3f57, lsl #32
10004de34:     	mov	x20, xzr
10004de38:     	movk	x21, #0x51d0, lsl #48
10004de3c:     	mov	w27, #0x61              ; =97
10004de40:     	movk	x28, #0xa8e8, lsl #48
10004de44:     	add	x9, x24, #0x8
10004de48:     	ldar	x8, [x23]
10004de4c:     	ldar	w11, [x25]
10004de50:     	ldar	w10, [x24]
10004de54:     	ldar	x9, [x9]
10004de58:     	cbnz	w11, 0x10004de6c <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x30c>
10004de5c:     	cmp	w10, #0x1
10004de60:     	b.ne	0x10004de6c <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x30c>
10004de64:     	cmp	x9, x8
10004de68:     	b.eq	0x10004de80 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x320>
10004de6c:     	ldr	x8, [sp, #0x40]
10004de70:     	str	x8, [sp, #0x8]
10004de74:     	bl	0x100059084 <_scoop_rt_safepoint>
10004de78:     	ldr	x8, [sp, #0x8]
10004de7c:     	str	x8, [sp, #0x40]
10004de80:     	cmp	x20, x19
10004de84:     	b.ge	0x10004ded8 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x378>
10004de88:     	umulh	x9, x20, x21
10004de8c:     	ldr	x0, [sp, #0x40]
10004de90:     	smulh	x8, x20, x28
10004de94:     	str	x0, [sp, #0x30]
10004de98:     	str	x0, [sp, #0x8]
10004de9c:     	sub	x11, x20, x9
10004dea0:     	add	x9, x9, x11, lsr #1
10004dea4:     	add	x8, x8, x20
10004dea8:     	lsr	x9, x9, #6
10004deac:     	asr	x10, x8, #6
10004deb0:     	msub	w1, w9, w27, w20
10004deb4:     	add	x8, x10, x8, lsr #63
10004deb8:     	msub	x22, x8, x27, x20
10004debc:     	bl	0x10006b89c <dyld_stub_binder+0x10006b89c>
10004dec0:     	ldr	x8, [sp, #0x8]
10004dec4:     	str	x8, [sp, #0x40]
10004dec8:     	add	x26, x26, x22
10004decc:     	add	x20, x20, #0x1
10004ded0:     	str	x8, [sp, #0x30]
10004ded4:     	b	0x10004de44 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x2e4>
10004ded8:     	movi.2d	v0, #0000000000000000
10004dedc:     	add	x20, sp, #0x40
10004dee0:     	adrp	x8, 0x1001b1000 <_scoop$1$tr$a7bf7611636503acb7f3d62ef99aa1b0c731f87447cf9668cea4d36b0fd4e48b>
10004dee4:     	add	x8, x8, #0x8a0
10004dee8:     	add	x0, sp, #0x200
10004deec:     	add	x1, sp, #0x1f0
10004def0:     	mov	w2, #0x1                ; =1
10004def4:     	stp	x20, x8, [sp, #0x1f0]
10004def8:     	str	xzr, [sp, #0x210]
10004defc:     	str	q0, [sp, #0x200]
10004df00:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004df04:     	movi.2d	v0, #0000000000000000
10004df08:     	add	x22, sp, #0x68
10004df0c:     	add	x0, sp, #0x218
10004df10:     	add	x1, sp, #0x218
10004df14:     	stp	q0, q0, [x22, #0x1b0]
10004df18:     	stp	q0, q0, [x22, #0x1d0]
10004df1c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10004df20:     	mov	w0, #0x1                ; =1
10004df24:     	bl	0x100069ebc <_m34_container_end>
10004df28:     	add	x0, sp, #0x218
10004df2c:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10004df30:     	ldr	x21, [sp, #0x40]
10004df34:     	add	x0, sp, #0x200
10004df38:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004df3c:     	movi.2d	v0, #0000000000000000
10004df40:     	adrp	x8, 0x1001b1000 <_scoop$1$tr$a7bf7611636503acb7f3d62ef99aa1b0c731f87447cf9668cea4d36b0fd4e48b>
10004df44:     	add	x8, x8, #0x8b0
10004df48:     	add	x0, sp, #0x268
10004df4c:     	add	x1, sp, #0x258
10004df50:     	mov	w2, #0x1                ; =1
10004df54:     	str	x21, [sp, #0x40]
10004df58:     	str	x20, [sp, #0x258]
10004df5c:     	str	x8, [sp, #0x260]
10004df60:     	str	q0, [x22, #0x200]
10004df64:     	str	xzr, [sp, #0x278]
10004df68:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004df6c:     	movi.2d	v0, #0000000000000000
10004df70:     	add	x0, sp, #0x280
10004df74:     	add	x1, sp, #0x280
10004df78:     	stp	q0, q0, [sp, #0x280]
10004df7c:     	stp	q0, q0, [sp, #0x2a0]
10004df80:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10004df84:     	bl	0x100069e44 <_m34_container_begin>
10004df88:     	add	x0, sp, #0x280
10004df8c:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10004df90:     	ldr	x27, [sp, #0x40]
10004df94:     	add	x0, sp, #0x268
10004df98:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004df9c:     	mov	x20, xzr
10004dfa0:     	mov	x21, xzr
10004dfa4:     	str	x27, [sp, #0x40]
10004dfa8:     	ldar	x8, [x23]
10004dfac:     	ldar	w9, [x25]
10004dfb0:     	add	x11, x24, #0x8
10004dfb4:     	ldar	w10, [x24]
10004dfb8:     	ldar	x11, [x11]
10004dfbc:     	cmp	w9, #0x0
10004dfc0:     	ccmp	w10, #0x1, #0x0, eq
10004dfc4:     	ccmp	x11, x8, #0x0, eq
10004dfc8:     	b.eq	0x10004dfe0 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x480>
10004dfcc:     	ldr	x8, [sp, #0x40]
10004dfd0:     	str	x8, [sp, #0x8]
10004dfd4:     	bl	0x100059084 <_scoop_rt_safepoint>
10004dfd8:     	ldr	x8, [sp, #0x8]
10004dfdc:     	str	x8, [sp, #0x40]
10004dfe0:     	cmp	x21, x19
10004dfe4:     	b.ge	0x10004e058 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x4f8>
10004dfe8:     	ldr	x0, [sp, #0x40]
10004dfec:     	mov	x1, x21
10004dff0:     	str	x0, [sp, #0x20]
10004dff4:     	str	x0, [sp, #0x8]
10004dff8:     	bl	0x10006ba40 <dyld_stub_binder+0x10006ba40>
10004dffc:     	ldr	x8, [sp, #0x8]
10004e000:     	str	x8, [sp, #0x40]
10004e004:     	mov	x9, x8
10004e008:     	add	x20, x20, w0, sxtw
10004e00c:     	mov	x0, x8
10004e010:     	mov	x1, x21
10004e014:     	str	x8, [sp, #0x20]
10004e018:     	stp	x8, x8, [sp, #0x10]
10004e01c:     	str	x9, [sp]
10004e020:     	bl	0x10006ba40 <dyld_stub_binder+0x10006ba40>
10004e024:     	ldp	x9, x8, [sp]
10004e028:     	str	x9, [sp, #0x40]
10004e02c:     	str	x8, [sp, #0x10]
10004e030:     	add	w2, w0, #0x1
10004e034:     	mov	x0, x8
10004e038:     	mov	x1, x21
10004e03c:     	str	x8, [sp, #0x18]
10004e040:     	bl	0x10006b9ec <dyld_stub_binder+0x10006b9ec>
10004e044:     	ldp	x8, x9, [sp]
10004e048:     	str	x8, [sp, #0x40]
10004e04c:     	str	x9, [sp, #0x18]
10004e050:     	add	x21, x21, #0x1
10004e054:     	b	0x10004dfa8 <_scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x448>
10004e058:     	movi.2d	v0, #0000000000000000
10004e05c:     	add	x21, sp, #0x40
10004e060:     	adrp	x8, 0x1001b1000 <_scoop$1$tr$a7bf7611636503acb7f3d62ef99aa1b0c731f87447cf9668cea4d36b0fd4e48b>
10004e064:     	add	x8, x8, #0x8c0
10004e068:     	add	x0, sp, #0x2d0
10004e06c:     	add	x1, sp, #0x2c0
10004e070:     	mov	w2, #0x1                ; =1
10004e074:     	str	x21, [sp, #0x2c0]
10004e078:     	str	x8, [sp, #0x2c8]
10004e07c:     	str	q0, [sp, #0x2d0]
10004e080:     	str	xzr, [sp, #0x2e0]
10004e084:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004e088:     	movi.2d	v0, #0000000000000000
10004e08c:     	add	x0, sp, #0x2e8
10004e090:     	add	x1, sp, #0x2e8
10004e094:     	stp	q0, q0, [x22, #0x280]
10004e098:     	stp	q0, q0, [x22, #0x2a0]
10004e09c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10004e0a0:     	mov	w0, #0x2                ; =2
10004e0a4:     	bl	0x100069ebc <_m34_container_end>
10004e0a8:     	add	x0, sp, #0x2e8
10004e0ac:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10004e0b0:     	ldr	x19, [sp, #0x40]
10004e0b4:     	add	x0, sp, #0x2d0
10004e0b8:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004e0bc:     	cmp	x20, x26
10004e0c0:     	str	x19, [sp, #0x40]
10004e0c4:     	cset	w0, eq
10004e0c8:     	str	x19, [sp, #0x8]
10004e0cc:     	bl	0x10003f560 <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
10004e0d0:     	ldr	x8, [sp, #0x8]
10004e0d4:     	movi.2d	v0, #0000000000000000
10004e0d8:     	str	x8, [sp, #0x40]
10004e0dc:     	adrp	x8, 0x1001b1000 <_scoop$1$tr$a7bf7611636503acb7f3d62ef99aa1b0c731f87447cf9668cea4d36b0fd4e48b>
10004e0e0:     	add	x8, x8, #0x8d0
10004e0e4:     	add	x0, sp, #0x338
10004e0e8:     	add	x1, sp, #0x328
10004e0ec:     	mov	w2, #0x1                ; =1
10004e0f0:     	str	x21, [sp, #0x328]
10004e0f4:     	str	x8, [sp, #0x330]
10004e0f8:     	str	q0, [x22, #0x2d0]
10004e0fc:     	str	xzr, [sp, #0x348]
10004e100:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004e104:     	movi.2d	v0, #0000000000000000
10004e108:     	sub	x0, x29, #0x100
10004e10c:     	sub	x1, x29, #0x100
10004e110:     	stp	q0, q0, [x29, #-0x100]
10004e114:     	stp	q0, q0, [x29, #-0xe0]
10004e118:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10004e11c:     	bl	0x100069e44 <_m34_container_begin>
10004e120:     	sub	x0, x29, #0x100
10004e124:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10004e128:     	ldr	x19, [sp, #0x40]
10004e12c:     	add	x0, sp, #0x338
10004e130:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004e134:     	mov	x0, x19
10004e138:     	str	x19, [sp, #0x40]
10004e13c:     	str	x19, [sp, #0x8]
10004e140:     	bl	0x10006b7f4 <dyld_stub_binder+0x10006b7f4>
10004e144:     	ldr	x8, [sp, #0x8]
10004e148:     	str	x8, [sp, #0x40]
10004e14c:     	movi.2d	v0, #0000000000000000
10004e150:     	str	x8, [sp, #0x28]
10004e154:     	adrp	x8, 0x1001b1000 <_scoop$1$tr$a7bf7611636503acb7f3d62ef99aa1b0c731f87447cf9668cea4d36b0fd4e48b>
10004e158:     	add	x8, x8, #0x8e0
10004e15c:     	sub	x0, x29, #0xb0
10004e160:     	sub	x1, x29, #0xc0
10004e164:     	mov	w2, #0x1                ; =1
10004e168:     	stp	x21, x8, [x29, #-0xc0]
10004e16c:     	stur	xzr, [x29, #-0xa0]
10004e170:     	stur	q0, [x29, #-0xb0]
10004e174:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004e178:     	movi.2d	v0, #0000000000000000
10004e17c:     	sub	x0, x29, #0x98
10004e180:     	sub	x1, x29, #0x98
10004e184:     	stp	q0, q0, [x22, #0x350]
10004e188:     	stp	q0, q0, [x22, #0x370]
10004e18c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10004e190:     	mov	w0, #0x3                ; =3
10004e194:     	bl	0x100069ebc <_m34_container_end>
10004e198:     	sub	x0, x29, #0x98
10004e19c:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10004e1a0:     	ldr	x19, [sp, #0x40]
10004e1a4:     	sub	x0, x29, #0xb0
10004e1a8:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004e1ac:     	str	x19, [sp, #0x40]
10004e1b0:     	ldr	x8, [x19, #0x18]
10004e1b4:     	cmp	x8, #0x0
10004e1b8:     	cset	w0, eq
10004e1bc:     	bl	0x10003f560 <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
10004e1c0:     	mov	x0, x20
10004e1c4:     	add	sp, sp, #0x400
10004e1c8:     	ldp	x29, x30, [sp, #0x50]
10004e1cc:     	ldp	x20, x19, [sp, #0x40]
10004e1d0:     	ldp	x22, x21, [sp, #0x30]
10004e1d4:     	ldp	x24, x23, [sp, #0x20]
10004e1d8:     	ldp	x26, x25, [sp, #0x10]
10004e1dc:     	ldp	x28, x27, [sp], #0x60
10004e1e0:     	ret

; interfaceList, MIR fn5, LIR scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-off-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000481c0 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c>:
1000481c0:     	stp	x28, x27, [sp, #-0x60]!
1000481c4:     	stp	x26, x25, [sp, #0x10]
1000481c8:     	stp	x24, x23, [sp, #0x20]
1000481cc:     	stp	x22, x21, [sp, #0x30]
1000481d0:     	stp	x20, x19, [sp, #0x40]
1000481d4:     	stp	x29, x30, [sp, #0x50]
1000481d8:     	add	x29, sp, #0x50
1000481dc:     	sub	sp, sp, #0x420
1000481e0:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
1000481e4:     	mov	x19, x0
1000481e8:     	add	x8, x8, #0x5d0
1000481ec:     	ldr	x9, [x8]
1000481f0:     	mov	x0, x8
1000481f4:     	blr	x9
1000481f8:     	adrp	x24, 0x10020d000 <_scoop_gc_marker+0x800>
1000481fc:     	adrp	x26, 0x10020d000 <_scoop_gc_marker+0x800>
100048200:     	add	x24, x24, #0xa10
100048204:     	ldr	x25, [x0]
100048208:     	add	x26, x26, #0x9fc
10004820c:     	str	xzr, [sp, #0x68]
100048210:     	add	x10, x25, #0x8
100048214:     	ldar	x8, [x24]
100048218:     	ldar	w11, [x26]
10004821c:     	ldar	w9, [x25]
100048220:     	ldar	x10, [x10]
100048224:     	cbnz	w11, 0x100048234 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x74>
100048228:     	cmp	w9, #0x1
10004822c:     	ccmp	x10, x8, #0x0, eq
100048230:     	b.eq	0x100048238 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x78>
100048234:     	bl	0x100059084 <_scoop_rt_safepoint>
100048238:     	movi.2d	v0, #0000000000000000
10004823c:     	add	x0, sp, #0x70
100048240:     	mov	x1, xzr
100048244:     	mov	x2, xzr
100048248:     	add	x23, sp, #0xf0
10004824c:     	str	xzr, [sp, #0x80]
100048250:     	str	q0, [sp, #0x70]
100048254:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100048258:     	movi.2d	v0, #0000000000000000
10004825c:     	add	x0, sp, #0x88
100048260:     	add	x1, sp, #0x88
100048264:     	stur	q0, [sp, #0x88]
100048268:     	stur	q0, [sp, #0x98]
10004826c:     	stur	q0, [sp, #0xa8]
100048270:     	stur	q0, [sp, #0xb8]
100048274:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100048278:     	bl	0x100069e44 <_m34_container_begin>
10004827c:     	add	x0, sp, #0x88
100048280:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100048284:     	add	x0, sp, #0x70
100048288:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10004828c:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
100048290:     	adrp	x10, 0x100208000 <dyld_stub_binder+0x100208000>
100048294:     	add	x0, x0, #0x5b8
100048298:     	ldr	x10, [x10, #0xc30]
10004829c:     	ldr	x27, [x0]
1000482a0:     	ldr	x9, [x10, #0x18]
1000482a4:     	blr	x27
1000482a8:     	ldr	x8, [x0]
1000482ac:     	neg	x11, x9
1000482b0:     	ldr	x12, [x8]
1000482b4:     	add	x12, x9, x12
1000482b8:     	sub	x12, x12, #0x1
1000482bc:     	ands	x21, x12, x11
1000482c0:     	b.eq	0x10004831c <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x15c>
1000482c4:     	add	x12, x9, #0x1f
1000482c8:     	and	x2, x12, x11
1000482cc:     	mov	w11, #0x7f80            ; =32640
1000482d0:     	cmp	x2, x11
1000482d4:     	b.hi	0x10004831c <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x15c>
1000482d8:     	ldr	x10, [x10, #0x90]
1000482dc:     	cbnz	x10, 0x10004831c <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x15c>
1000482e0:     	ldr	x11, [x8, #0x8]
1000482e4:     	add	x10, x21, x2
1000482e8:     	cmp	x10, x11
1000482ec:     	b.hi	0x10004831c <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x15c>
1000482f0:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
1000482f4:     	add	x9, x9, x11
1000482f8:     	cmn	x9, #0x21
1000482fc:     	b.hi	0x10004831c <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x15c>
100048300:     	str	x10, [x8]
100048304:     	adrp	x1, 0x100208000 <dyld_stub_binder+0x100208000>
100048308:     	mov	x0, x21
10004830c:     	ldr	x1, [x1, #0xc30]
100048310:     	bl	0x100054aac <_scoop_runtime_finish_tlab_alloc>
100048314:     	mov	x0, x21
100048318:     	b	0x10004832c <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x16c>
10004831c:     	adrp	x0, 0x100208000 <dyld_stub_binder+0x100208000>
100048320:     	mov	w1, #0x20               ; =32
100048324:     	ldr	x0, [x0, #0xc30]
100048328:     	bl	0x100059094 <_scoop_runtime_alloc_slow>
10004832c:     	mov	x1, x19
100048330:     	str	x0, [sp, #0x18]
100048334:     	bl	0x10006b890 <dyld_stub_binder+0x10006b890>
100048338:     	ldr	x8, [sp, #0x18]
10004833c:     	movi.2d	v0, #0000000000000000
100048340:     	str	x8, [sp, #0x60]
100048344:     	add	x20, sp, #0x58
100048348:     	str	x8, [sp, #0x58]
10004834c:     	adrp	x8, 0x1001a1000 <_scoop$1$sr$43335df2b5c1b46dc7a791174a45f8f0b1c6c3d01e219b421c8dc6076c4f7f15+0xb0>
100048350:     	add	x8, x8, #0xaa0
100048354:     	add	x0, sp, #0xd8
100048358:     	add	x1, sp, #0xc8
10004835c:     	mov	w2, #0x1                ; =1
100048360:     	stp	x20, x8, [sp, #0xc8]
100048364:     	stur	q0, [sp, #0xd8]
100048368:     	str	xzr, [sp, #0xe8]
10004836c:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100048370:     	movi.2d	v0, #0000000000000000
100048374:     	add	x0, sp, #0xf0
100048378:     	add	x1, sp, #0xf0
10004837c:     	stp	q0, q0, [x23]
100048380:     	stp	q0, q0, [x23, #0x20]
100048384:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100048388:     	mov	w0, wzr
10004838c:     	bl	0x100069ebc <_m34_container_end>
100048390:     	add	x0, sp, #0xf0
100048394:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100048398:     	ldr	x21, [sp, #0x58]
10004839c:     	add	x0, sp, #0xd8
1000483a0:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
1000483a4:     	movi.2d	v0, #0000000000000000
1000483a8:     	adrp	x8, 0x1001a1000 <_scoop$1$sr$43335df2b5c1b46dc7a791174a45f8f0b1c6c3d01e219b421c8dc6076c4f7f15+0xb0>
1000483ac:     	add	x8, x8, #0xab0
1000483b0:     	str	x21, [sp, #0x58]
1000483b4:     	adrp	x9, 0x1001a1000 <_scoop$1$sr$43335df2b5c1b46dc7a791174a45f8f0b1c6c3d01e219b421c8dc6076c4f7f15+0xb0>
1000483b8:     	add	x9, x9, #0xac0
1000483bc:     	stp	x20, x8, [sp, #0x130]
1000483c0:     	add	x8, sp, #0x50
1000483c4:     	add	x0, sp, #0x150
1000483c8:     	add	x1, sp, #0x130
1000483cc:     	mov	w2, #0x2                ; =2
1000483d0:     	str	x21, [sp, #0x50]
1000483d4:     	stp	x8, x9, [sp, #0x140]
1000483d8:     	str	q0, [x23, #0x60]
1000483dc:     	str	xzr, [sp, #0x160]
1000483e0:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
1000483e4:     	movi.2d	v0, #0000000000000000
1000483e8:     	add	x8, sp, #0x69
1000483ec:     	add	x0, sp, #0x168
1000483f0:     	add	x1, sp, #0x168
1000483f4:     	stur	q0, [x8, #0xff]
1000483f8:     	add	x8, sp, #0x79
1000483fc:     	stur	q0, [x8, #0xff]
100048400:     	add	x8, sp, #0x89
100048404:     	stur	q0, [x8, #0xff]
100048408:     	add	x8, sp, #0x99
10004840c:     	stur	q0, [x8, #0xff]
100048410:     	bl	0x100059194 <_scoop_rt_enter_native_borrowed>
100048414:     	ldr	x0, [sp, #0x50]
100048418:     	bl	0x100069fb8 <_m34_container_layout>
10004841c:     	add	x0, sp, #0x168
100048420:     	bl	0x1000647dc <_scoop_rt_leave_native_borrowed>
100048424:     	ldr	x21, [sp, #0x58]
100048428:     	ldr	x22, [sp, #0x50]
10004842c:     	add	x0, sp, #0x150
100048430:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100048434:     	movi.2d	v0, #0000000000000000
100048438:     	adrp	x8, 0x1001a1000 <_scoop$1$sr$43335df2b5c1b46dc7a791174a45f8f0b1c6c3d01e219b421c8dc6076c4f7f15+0xb0>
10004843c:     	add	x8, x8, #0xad0
100048440:     	str	x21, [sp, #0x58]
100048444:     	add	x0, sp, #0x1b8
100048448:     	add	x1, sp, #0x1a8
10004844c:     	str	x22, [sp, #0x50]
100048450:     	mov	w2, #0x1                ; =1
100048454:     	stp	x20, x8, [sp, #0x1a8]
100048458:     	add	x8, sp, #0xb9
10004845c:     	stur	q0, [x8, #0xff]
100048460:     	str	xzr, [sp, #0x1c8]
100048464:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100048468:     	movi.2d	v0, #0000000000000000
10004846c:     	add	x0, sp, #0x1d0
100048470:     	add	x1, sp, #0x1d0
100048474:     	stp	q0, q0, [x23, #0xe0]
100048478:     	stp	q0, q0, [x23, #0x100]
10004847c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100048480:     	bl	0x100069e44 <_m34_container_begin>
100048484:     	add	x0, sp, #0x1d0
100048488:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10004848c:     	ldr	x20, [sp, #0x58]
100048490:     	add	x0, sp, #0x1b8
100048494:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100048498:     	adrp	x21, 0x100180000 <_scoop$1$sr$f64295cddd0798fe06071e3c922bcfb410cb3916b2f9ec2e2ac0c5edb140a652+0x30>
10004849c:     	mov	x28, xzr
1000484a0:     	mov	x23, #0x7fffffffffffffff ; =9223372036854775807
1000484a4:     	add	x21, x21, #0xad0
1000484a8:     	str	x20, [sp, #0x58]
1000484ac:     	mov	w20, #0x7f80            ; =32640
1000484b0:     	b	0x100048518 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x358>
1000484b4:     	ldr	x9, [sp, #0x58]
1000484b8:     	mov	x0, x21
1000484bc:     	mov	w1, #0x18               ; =24
1000484c0:     	stp	x8, x9, [sp, #0x10]
1000484c4:     	bl	0x100059094 <_scoop_runtime_alloc_slow>
1000484c8:     	ldp	x9, x8, [sp, #0x10]
1000484cc:     	mov	x22, x0
1000484d0:     	str	x8, [sp, #0x58]
1000484d4:     	str	x9, [sp, #0x48]
1000484d8:     	ldr	x8, [x21, #0x60]
1000484dc:     	str	w28, [x22, #0x10]
1000484e0:     	ldr	x2, [x8, #0x8]
1000484e4:     	str	x22, [sp, #0x28]
1000484e8:     	ldr	x1, [sp, #0x28]
1000484ec:     	ldr	x8, [sp, #0x58]
1000484f0:     	ldr	x0, [sp, #0x48]
1000484f4:     	stp	x8, x1, [sp, #0x18]
1000484f8:     	stp	x1, x0, [sp, #0x8]
1000484fc:     	bl	0x10006b8b4 <dyld_stub_binder+0x10006b8b4>
100048500:     	ldp	x10, x8, [sp, #0x10]
100048504:     	ldr	x9, [sp, #0x8]
100048508:     	str	x8, [sp, #0x58]
10004850c:     	str	x10, [sp, #0x48]
100048510:     	str	x9, [sp, #0x20]
100048514:     	add	x28, x28, #0x1
100048518:     	ldar	x8, [x24]
10004851c:     	ldar	w9, [x26]
100048520:     	add	x11, x25, #0x8
100048524:     	ldar	w10, [x25]
100048528:     	ldar	x11, [x11]
10004852c:     	cmp	w9, #0x0
100048530:     	ccmp	w10, #0x1, #0x0, eq
100048534:     	ccmp	x11, x8, #0x0, eq
100048538:     	b.eq	0x100048550 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x390>
10004853c:     	ldr	x8, [sp, #0x58]
100048540:     	str	x8, [sp, #0x18]
100048544:     	bl	0x100059084 <_scoop_rt_safepoint>
100048548:     	ldr	x8, [sp, #0x18]
10004854c:     	str	x8, [sp, #0x58]
100048550:     	cmp	x28, x19
100048554:     	b.ge	0x1000485d4 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x414>
100048558:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
10004855c:     	ldr	x8, [sp, #0x58]
100048560:     	ldr	x10, [x21, #0x18]
100048564:     	add	x0, x0, #0x5b8
100048568:     	str	x8, [sp, #0x48]
10004856c:     	blr	x27
100048570:     	ldr	x9, [x0]
100048574:     	neg	x11, x10
100048578:     	ldr	x12, [x9]
10004857c:     	add	x12, x10, x12
100048580:     	sub	x12, x12, #0x1
100048584:     	ands	x22, x12, x11
100048588:     	b.eq	0x1000484b4 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x2f4>
10004858c:     	add	x12, x10, #0x17
100048590:     	and	x2, x12, x11
100048594:     	cmp	x2, x20
100048598:     	b.hi	0x1000484b4 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x2f4>
10004859c:     	ldr	x11, [x21, #0x90]
1000485a0:     	cbnz	x11, 0x1000484b4 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x2f4>
1000485a4:     	ldr	x12, [x9, #0x8]
1000485a8:     	add	x11, x22, x2
1000485ac:     	cmp	x11, x12
1000485b0:     	b.hi	0x1000484b4 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x2f4>
1000485b4:     	add	x10, x10, x23
1000485b8:     	cmn	x10, #0x18
1000485bc:     	b.hs	0x1000484b4 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x2f4>
1000485c0:     	mov	x0, x22
1000485c4:     	mov	x1, x21
1000485c8:     	str	x11, [x9]
1000485cc:     	bl	0x100054aac <_scoop_runtime_finish_tlab_alloc>
1000485d0:     	b	0x1000484d8 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x318>
1000485d4:     	movi.2d	v0, #0000000000000000
1000485d8:     	add	x20, sp, #0x58
1000485dc:     	adrp	x8, 0x1001a1000 <_scoop$1$sr$43335df2b5c1b46dc7a791174a45f8f0b1c6c3d01e219b421c8dc6076c4f7f15+0xb0>
1000485e0:     	add	x8, x8, #0xae0
1000485e4:     	add	x23, sp, #0xf0
1000485e8:     	add	x0, sp, #0x220
1000485ec:     	add	x1, sp, #0x210
1000485f0:     	mov	w2, #0x1                ; =1
1000485f4:     	str	x20, [sp, #0x210]
1000485f8:     	str	x8, [sp, #0x218]
1000485fc:     	str	q0, [x23, #0x130]
100048600:     	str	xzr, [sp, #0x230]
100048604:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100048608:     	movi.2d	v0, #0000000000000000
10004860c:     	add	x8, sp, #0x139
100048610:     	add	x0, sp, #0x238
100048614:     	add	x1, sp, #0x238
100048618:     	stur	q0, [x8, #0xff]
10004861c:     	add	x8, sp, #0x149
100048620:     	stur	q0, [x8, #0xff]
100048624:     	add	x8, sp, #0x159
100048628:     	stur	q0, [x8, #0xff]
10004862c:     	add	x8, sp, #0x169
100048630:     	stur	q0, [x8, #0xff]
100048634:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
100048638:     	mov	w0, #0x1                ; =1
10004863c:     	bl	0x100069ebc <_m34_container_end>
100048640:     	add	x0, sp, #0x238
100048644:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100048648:     	ldr	x21, [sp, #0x58]
10004864c:     	add	x0, sp, #0x220
100048650:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100048654:     	movi.2d	v0, #0000000000000000
100048658:     	adrp	x8, 0x1001a1000 <_scoop$1$sr$43335df2b5c1b46dc7a791174a45f8f0b1c6c3d01e219b421c8dc6076c4f7f15+0xb0>
10004865c:     	add	x8, x8, #0xaf0
100048660:     	str	x21, [sp, #0x58]
100048664:     	add	x0, sp, #0x288
100048668:     	add	x1, sp, #0x278
10004866c:     	str	x8, [sp, #0x280]
100048670:     	add	x8, sp, #0x189
100048674:     	mov	w2, #0x1                ; =1
100048678:     	str	x20, [sp, #0x278]
10004867c:     	stur	q0, [x8, #0xff]
100048680:     	str	xzr, [sp, #0x298]
100048684:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100048688:     	movi.2d	v0, #0000000000000000
10004868c:     	add	x0, sp, #0x2a0
100048690:     	add	x1, sp, #0x2a0
100048694:     	stp	q0, q0, [x23, #0x1b0]
100048698:     	stp	q0, q0, [x23, #0x1d0]
10004869c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
1000486a0:     	bl	0x100069e44 <_m34_container_begin>
1000486a4:     	add	x0, sp, #0x2a0
1000486a8:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
1000486ac:     	ldr	x22, [sp, #0x58]
1000486b0:     	add	x0, sp, #0x288
1000486b4:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
1000486b8:     	mov	x20, xzr
1000486bc:     	mov	x21, xzr
1000486c0:     	str	x22, [sp, #0x58]
1000486c4:     	ldar	x8, [x24]
1000486c8:     	ldar	w9, [x26]
1000486cc:     	add	x11, x25, #0x8
1000486d0:     	ldar	w10, [x25]
1000486d4:     	ldar	x11, [x11]
1000486d8:     	cmp	w9, #0x0
1000486dc:     	ccmp	w10, #0x1, #0x0, eq
1000486e0:     	ccmp	x11, x8, #0x0, eq
1000486e4:     	b.eq	0x1000486fc <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x53c>
1000486e8:     	ldr	x8, [sp, #0x58]
1000486ec:     	str	x8, [sp, #0x18]
1000486f0:     	bl	0x100059084 <_scoop_rt_safepoint>
1000486f4:     	ldr	x8, [sp, #0x18]
1000486f8:     	str	x8, [sp, #0x58]
1000486fc:     	cmp	x21, x19
100048700:     	b.ge	0x10004875c <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x59c>
100048704:     	ldr	x0, [sp, #0x58]
100048708:     	mov	x1, x21
10004870c:     	str	x0, [sp, #0x40]
100048710:     	str	x0, [sp, #0x18]
100048714:     	bl	0x10006b9f8 <dyld_stub_binder+0x10006b9f8>
100048718:     	ldr	x8, [sp, #0x18]
10004871c:     	str	x8, [sp, #0x58]
100048720:     	str	x8, [sp, #0x40]
100048724:     	str	x0, [sp, #0x38]
100048728:     	ldr	x8, [sp, #0x38]
10004872c:     	ldr	x0, [sp, #0x38]
100048730:     	ldr	x8, [sp, #0x58]
100048734:     	str	x0, [sp, #0x68]
100048738:     	ldr	x9, [x1]
10004873c:     	stp	x0, x8, [sp, #0x10]
100048740:     	blr	x9
100048744:     	ldp	x9, x8, [sp, #0x10]
100048748:     	str	x8, [sp, #0x58]
10004874c:     	add	x20, x20, w0, sxtw
100048750:     	str	x9, [sp, #0x68]
100048754:     	add	x21, x21, #0x1
100048758:     	b	0x1000486c4 <_scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x504>
10004875c:     	movi.2d	v0, #0000000000000000
100048760:     	add	x21, sp, #0x58
100048764:     	adrp	x8, 0x1001a1000 <_scoop$1$sr$43335df2b5c1b46dc7a791174a45f8f0b1c6c3d01e219b421c8dc6076c4f7f15+0xb0>
100048768:     	add	x8, x8, #0xb00
10004876c:     	add	x0, sp, #0x2f0
100048770:     	add	x1, sp, #0x2e0
100048774:     	mov	w2, #0x1                ; =1
100048778:     	str	x21, [sp, #0x2e0]
10004877c:     	str	x8, [sp, #0x2e8]
100048780:     	str	q0, [x23, #0x200]
100048784:     	str	xzr, [sp, #0x300]
100048788:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10004878c:     	movi.2d	v0, #0000000000000000
100048790:     	add	x8, sp, #0x209
100048794:     	add	x0, sp, #0x308
100048798:     	add	x1, sp, #0x308
10004879c:     	stur	q0, [x8, #0xff]
1000487a0:     	add	x8, sp, #0x219
1000487a4:     	stur	q0, [x8, #0xff]
1000487a8:     	add	x8, sp, #0x229
1000487ac:     	stur	q0, [x8, #0xff]
1000487b0:     	add	x8, sp, #0x239
1000487b4:     	stur	q0, [x8, #0xff]
1000487b8:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
1000487bc:     	mov	w0, #0x2                ; =2
1000487c0:     	bl	0x100069ebc <_m34_container_end>
1000487c4:     	add	x0, sp, #0x308
1000487c8:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
1000487cc:     	ldr	x22, [sp, #0x58]
1000487d0:     	add	x0, sp, #0x2f0
1000487d4:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
1000487d8:     	sub	x8, x19, #0x1
1000487dc:     	mov	x9, x22
1000487e0:     	str	x22, [sp, #0x58]
1000487e4:     	mul	x8, x19, x8
1000487e8:     	str	x9, [sp, #0x18]
1000487ec:     	add	x8, x8, x8, lsr #63
1000487f0:     	cmp	x20, x8, asr #1
1000487f4:     	cset	w0, eq
1000487f8:     	bl	0x10003f560 <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
1000487fc:     	ldr	x8, [sp, #0x18]
100048800:     	movi.2d	v0, #0000000000000000
100048804:     	str	x8, [sp, #0x58]
100048808:     	adrp	x8, 0x1001a1000 <_scoop$1$sr$43335df2b5c1b46dc7a791174a45f8f0b1c6c3d01e219b421c8dc6076c4f7f15+0xb0>
10004880c:     	add	x8, x8, #0xb10
100048810:     	add	x0, sp, #0x358
100048814:     	add	x1, sp, #0x348
100048818:     	str	x8, [sp, #0x350]
10004881c:     	add	x8, sp, #0x259
100048820:     	mov	w2, #0x1                ; =1
100048824:     	str	x21, [sp, #0x348]
100048828:     	stur	q0, [x8, #0xff]
10004882c:     	str	xzr, [sp, #0x368]
100048830:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
100048834:     	movi.2d	v0, #0000000000000000
100048838:     	sub	x0, x29, #0x100
10004883c:     	sub	x1, x29, #0x100
100048840:     	stp	q0, q0, [x23, #0x280]
100048844:     	stp	q0, q0, [x23, #0x2a0]
100048848:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10004884c:     	bl	0x100069e44 <_m34_container_begin>
100048850:     	sub	x0, x29, #0x100
100048854:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
100048858:     	ldr	x19, [sp, #0x58]
10004885c:     	add	x0, sp, #0x358
100048860:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
100048864:     	mov	x0, x19
100048868:     	str	x19, [sp, #0x58]
10004886c:     	str	x19, [sp, #0x18]
100048870:     	bl	0x10006b7dc <dyld_stub_binder+0x10006b7dc>
100048874:     	ldr	x8, [sp, #0x18]
100048878:     	str	x8, [sp, #0x58]
10004887c:     	movi.2d	v0, #0000000000000000
100048880:     	str	x8, [sp, #0x30]
100048884:     	adrp	x8, 0x1001a1000 <_scoop$1$sr$43335df2b5c1b46dc7a791174a45f8f0b1c6c3d01e219b421c8dc6076c4f7f15+0xb0>
100048888:     	add	x8, x8, #0xb20
10004888c:     	sub	x0, x29, #0xb0
100048890:     	sub	x1, x29, #0xc0
100048894:     	mov	w2, #0x1                ; =1
100048898:     	stp	x21, x8, [x29, #-0xc0]
10004889c:     	stur	xzr, [x29, #-0xa0]
1000488a0:     	str	q0, [x23, #0x2d0]
1000488a4:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
1000488a8:     	movi.2d	v0, #0000000000000000
1000488ac:     	sub	x0, x29, #0x98
1000488b0:     	sub	x1, x29, #0x98
1000488b4:     	stur	q0, [x29, #-0x98]
1000488b8:     	stur	q0, [x29, #-0x88]
1000488bc:     	stur	q0, [x29, #-0x78]
1000488c0:     	stur	q0, [x29, #-0x68]
1000488c4:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
1000488c8:     	mov	w0, #0x3                ; =3
1000488cc:     	bl	0x100069ebc <_m34_container_end>
1000488d0:     	sub	x0, x29, #0x98
1000488d4:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
1000488d8:     	ldr	x19, [sp, #0x58]
1000488dc:     	sub	x0, x29, #0xb0
1000488e0:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
1000488e4:     	str	x19, [sp, #0x58]
1000488e8:     	ldr	x8, [x19, #0x18]
1000488ec:     	cmp	x8, #0x0
1000488f0:     	cset	w0, eq
1000488f4:     	bl	0x10003f560 <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
1000488f8:     	mov	x0, x20
1000488fc:     	add	sp, sp, #0x420
100048900:     	ldp	x29, x30, [sp, #0x50]
100048904:     	ldp	x20, x19, [sp, #0x40]
100048908:     	ldp	x22, x21, [sp, #0x30]
10004890c:     	ldp	x24, x23, [sp, #0x20]
100048910:     	ldp	x26, x25, [sp, #0x10]
100048914:     	ldp	x28, x27, [sp], #0x60
100048918:     	ret

; zeroSizeList, MIR fn6, LIR scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/containers-off-darwin/containers:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010003be64 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0>:
10003be64:     	stp	x28, x27, [sp, #-0x60]!
10003be68:     	stp	x26, x25, [sp, #0x10]
10003be6c:     	stp	x24, x23, [sp, #0x20]
10003be70:     	stp	x22, x21, [sp, #0x30]
10003be74:     	stp	x20, x19, [sp, #0x40]
10003be78:     	stp	x29, x30, [sp, #0x50]
10003be7c:     	add	x29, sp, #0x50
10003be80:     	sub	sp, sp, #0x310
10003be84:     	adrp	x8, 0x10020d000 <_scoop_gc_marker+0x800>
10003be88:     	mov	x19, x0
10003be8c:     	add	x8, x8, #0x5d0
10003be90:     	ldr	x9, [x8]
10003be94:     	mov	x0, x8
10003be98:     	blr	x9
10003be9c:     	adrp	x23, 0x10020d000 <_scoop_gc_marker+0x800>
10003bea0:     	adrp	x25, 0x10020d000 <_scoop_gc_marker+0x800>
10003bea4:     	add	x23, x23, #0xa10
10003bea8:     	ldr	x24, [x0]
10003beac:     	add	x25, x25, #0x9fc
10003beb0:     	add	x10, x24, #0x8
10003beb4:     	ldar	x8, [x23]
10003beb8:     	ldar	w11, [x25]
10003bebc:     	ldar	w9, [x24]
10003bec0:     	ldar	x10, [x10]
10003bec4:     	cbnz	w11, 0x10003bed4 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x70>
10003bec8:     	cmp	w9, #0x1
10003becc:     	ccmp	x10, x8, #0x0, eq
10003bed0:     	b.eq	0x10003bed8 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x74>
10003bed4:     	bl	0x100059084 <_scoop_rt_safepoint>
10003bed8:     	add	x22, sp, #0xb8
10003bedc:     	bl	0x1000497ac <_scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
10003bee0:     	adrp	x21, 0x10020d000 <_scoop_gc_marker+0x800>
10003bee4:     	movi.2d	v0, #0000000000000000
10003bee8:     	add	x0, sp, #0x38
10003beec:     	add	x21, x21, #0x700
10003bef0:     	mov	x1, xzr
10003bef4:     	mov	x2, xzr
10003bef8:     	ldr	x8, [x21]
10003befc:     	str	xzr, [x8, #0x10]
10003bf00:     	stur	q0, [sp, #0x38]
10003bf04:     	str	xzr, [sp, #0x48]
10003bf08:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003bf0c:     	movi.2d	v0, #0000000000000000
10003bf10:     	add	x0, sp, #0x50
10003bf14:     	add	x1, sp, #0x50
10003bf18:     	stp	q0, q0, [sp, #0x50]
10003bf1c:     	stp	q0, q0, [sp, #0x70]
10003bf20:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10003bf24:     	bl	0x100069e44 <_m34_container_begin>
10003bf28:     	add	x0, sp, #0x50
10003bf2c:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10003bf30:     	add	x0, sp, #0x38
10003bf34:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003bf38:     	adrp	x0, 0x10020d000 <_scoop_gc_marker+0x800>
10003bf3c:     	adrp	x10, 0x100208000 <dyld_stub_binder+0x100208000>
10003bf40:     	add	x0, x0, #0x5b8
10003bf44:     	ldr	x10, [x10, #0xcb8]
10003bf48:     	ldr	x8, [x0]
10003bf4c:     	ldr	x9, [x10, #0x18]
10003bf50:     	blr	x8
10003bf54:     	ldr	x8, [x0]
10003bf58:     	neg	x11, x9
10003bf5c:     	ldr	x12, [x8]
10003bf60:     	add	x12, x9, x12
10003bf64:     	sub	x12, x12, #0x1
10003bf68:     	ands	x20, x12, x11
10003bf6c:     	b.eq	0x10003bfc8 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x164>
10003bf70:     	add	x12, x9, #0x1f
10003bf74:     	and	x2, x12, x11
10003bf78:     	mov	w11, #0x7f80            ; =32640
10003bf7c:     	cmp	x2, x11
10003bf80:     	b.hi	0x10003bfc8 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x164>
10003bf84:     	ldr	x10, [x10, #0x90]
10003bf88:     	cbnz	x10, 0x10003bfc8 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x164>
10003bf8c:     	ldr	x11, [x8, #0x8]
10003bf90:     	add	x10, x20, x2
10003bf94:     	cmp	x10, x11
10003bf98:     	b.hi	0x10003bfc8 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x164>
10003bf9c:     	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10003bfa0:     	add	x9, x9, x11
10003bfa4:     	cmn	x9, #0x21
10003bfa8:     	b.hi	0x10003bfc8 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x164>
10003bfac:     	str	x10, [x8]
10003bfb0:     	adrp	x1, 0x100208000 <dyld_stub_binder+0x100208000>
10003bfb4:     	mov	x0, x20
10003bfb8:     	ldr	x1, [x1, #0xcb8]
10003bfbc:     	bl	0x100054aac <_scoop_runtime_finish_tlab_alloc>
10003bfc0:     	mov	x0, x20
10003bfc4:     	b	0x10003bfd8 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x174>
10003bfc8:     	adrp	x0, 0x100208000 <dyld_stub_binder+0x100208000>
10003bfcc:     	mov	w1, #0x20               ; =32
10003bfd0:     	ldr	x0, [x0, #0xcb8]
10003bfd4:     	bl	0x100059094 <_scoop_runtime_alloc_slow>
10003bfd8:     	mov	x1, x19
10003bfdc:     	str	x0, [sp, #0x8]
10003bfe0:     	bl	0x10006b98c <dyld_stub_binder+0x10006b98c>
10003bfe4:     	ldr	x8, [sp, #0x8]
10003bfe8:     	movi.2d	v0, #0000000000000000
10003bfec:     	str	x8, [sp, #0x30]
10003bff0:     	add	x20, sp, #0x28
10003bff4:     	str	x8, [sp, #0x28]
10003bff8:     	adrp	x8, 0x10017f000 <_scoop$1$sr$9bc66fb5e6596307e7ecccaa8e62af2a26973f3e59a3033be911251d8c2ae020+0xa0>
10003bffc:     	add	x8, x8, #0x190
10003c000:     	add	x0, sp, #0xa0
10003c004:     	add	x1, sp, #0x90
10003c008:     	mov	w2, #0x1                ; =1
10003c00c:     	stp	x20, x8, [sp, #0x90]
10003c010:     	str	q0, [sp, #0xa0]
10003c014:     	str	xzr, [sp, #0xb0]
10003c018:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003c01c:     	movi.2d	v0, #0000000000000000
10003c020:     	add	x0, sp, #0xb8
10003c024:     	add	x1, sp, #0xb8
10003c028:     	stp	q0, q0, [x22]
10003c02c:     	stp	q0, q0, [x22, #0x20]
10003c030:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10003c034:     	mov	w0, wzr
10003c038:     	bl	0x100069ebc <_m34_container_end>
10003c03c:     	add	x0, sp, #0xb8
10003c040:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10003c044:     	ldr	x26, [sp, #0x28]
10003c048:     	add	x0, sp, #0xa0
10003c04c:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003c050:     	movi.2d	v0, #0000000000000000
10003c054:     	adrp	x8, 0x10017f000 <_scoop$1$sr$9bc66fb5e6596307e7ecccaa8e62af2a26973f3e59a3033be911251d8c2ae020+0xa0>
10003c058:     	add	x8, x8, #0x1a0
10003c05c:     	str	x26, [sp, #0x28]
10003c060:     	adrp	x9, 0x10017f000 <_scoop$1$sr$9bc66fb5e6596307e7ecccaa8e62af2a26973f3e59a3033be911251d8c2ae020+0xa0>
10003c064:     	add	x9, x9, #0x1b0
10003c068:     	stp	x20, x8, [sp, #0xf8]
10003c06c:     	add	x8, sp, #0x20
10003c070:     	add	x0, sp, #0x118
10003c074:     	add	x1, sp, #0xf8
10003c078:     	mov	w2, #0x2                ; =2
10003c07c:     	str	x26, [sp, #0x20]
10003c080:     	stp	x8, x9, [sp, #0x108]
10003c084:     	str	q0, [x22, #0x60]
10003c088:     	str	xzr, [sp, #0x128]
10003c08c:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003c090:     	movi.2d	v0, #0000000000000000
10003c094:     	add	x0, sp, #0x130
10003c098:     	add	x1, sp, #0x130
10003c09c:     	stp	q0, q0, [sp, #0x130]
10003c0a0:     	stp	q0, q0, [sp, #0x150]
10003c0a4:     	bl	0x100059194 <_scoop_rt_enter_native_borrowed>
10003c0a8:     	ldr	x0, [sp, #0x20]
10003c0ac:     	bl	0x100069fb8 <_m34_container_layout>
10003c0b0:     	add	x0, sp, #0x130
10003c0b4:     	bl	0x1000647dc <_scoop_rt_leave_native_borrowed>
10003c0b8:     	ldr	x26, [sp, #0x28]
10003c0bc:     	ldr	x27, [sp, #0x20]
10003c0c0:     	add	x0, sp, #0x118
10003c0c4:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003c0c8:     	movi.2d	v0, #0000000000000000
10003c0cc:     	adrp	x8, 0x10017f000 <_scoop$1$sr$9bc66fb5e6596307e7ecccaa8e62af2a26973f3e59a3033be911251d8c2ae020+0xa0>
10003c0d0:     	add	x8, x8, #0x1c0
10003c0d4:     	add	x0, sp, #0x180
10003c0d8:     	add	x1, sp, #0x170
10003c0dc:     	mov	w2, #0x1                ; =1
10003c0e0:     	str	x26, [sp, #0x28]
10003c0e4:     	str	x27, [sp, #0x20]
10003c0e8:     	stp	x20, x8, [sp, #0x170]
10003c0ec:     	str	q0, [sp, #0x180]
10003c0f0:     	str	xzr, [sp, #0x190]
10003c0f4:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003c0f8:     	movi.2d	v0, #0000000000000000
10003c0fc:     	add	x0, sp, #0x198
10003c100:     	add	x1, sp, #0x198
10003c104:     	stp	q0, q0, [x22, #0xe0]
10003c108:     	stp	q0, q0, [x22, #0x100]
10003c10c:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10003c110:     	bl	0x100069e44 <_m34_container_begin>
10003c114:     	add	x0, sp, #0x198
10003c118:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10003c11c:     	ldr	x26, [sp, #0x28]
10003c120:     	add	x0, sp, #0x180
10003c124:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003c128:     	mov	x20, xzr
10003c12c:     	str	x26, [sp, #0x28]
10003c130:     	ldar	x8, [x23]
10003c134:     	ldar	w9, [x25]
10003c138:     	add	x11, x24, #0x8
10003c13c:     	ldar	w10, [x24]
10003c140:     	ldar	x11, [x11]
10003c144:     	cmp	w9, #0x0
10003c148:     	ccmp	w10, #0x1, #0x0, eq
10003c14c:     	ccmp	x11, x8, #0x0, eq
10003c150:     	b.eq	0x10003c168 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x304>
10003c154:     	ldr	x8, [sp, #0x28]
10003c158:     	str	x8, [sp, #0x8]
10003c15c:     	bl	0x100059084 <_scoop_rt_safepoint>
10003c160:     	ldr	x8, [sp, #0x8]
10003c164:     	str	x8, [sp, #0x28]
10003c168:     	cmp	x20, x19
10003c16c:     	b.ge	0x10003c1c0 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x35c>
10003c170:     	ldr	x8, [sp, #0x28]
10003c174:     	str	x8, [sp, #0x18]
10003c178:     	stp	x8, x8, [sp]
10003c17c:     	bl	0x1000497ac <_scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
10003c180:     	ldp	x9, x8, [sp]
10003c184:     	str	x8, [sp, #0x28]
10003c188:     	str	x9, [sp, #0x18]
10003c18c:     	ldr	x8, [x21]
10003c190:     	ldr	x9, [x8, #0x10]
10003c194:     	add	x9, x9, #0x1
10003c198:     	str	x9, [x8, #0x10]
10003c19c:     	ldr	x8, [sp, #0x28]
10003c1a0:     	ldr	x0, [sp, #0x18]
10003c1a4:     	stp	x0, x8, [sp]
10003c1a8:     	bl	0x10006b8d8 <dyld_stub_binder+0x10006b8d8>
10003c1ac:     	ldp	x9, x8, [sp]
10003c1b0:     	str	x8, [sp, #0x28]
10003c1b4:     	str	x9, [sp, #0x18]
10003c1b8:     	add	x20, x20, #0x1
10003c1bc:     	b	0x10003c130 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x2cc>
10003c1c0:     	movi.2d	v0, #0000000000000000
10003c1c4:     	add	x20, sp, #0x28
10003c1c8:     	adrp	x8, 0x10017f000 <_scoop$1$sr$9bc66fb5e6596307e7ecccaa8e62af2a26973f3e59a3033be911251d8c2ae020+0xa0>
10003c1cc:     	add	x8, x8, #0x1d0
10003c1d0:     	add	x0, sp, #0x1e8
10003c1d4:     	add	x1, sp, #0x1d8
10003c1d8:     	mov	w2, #0x1                ; =1
10003c1dc:     	stp	x20, x8, [sp, #0x1d8]
10003c1e0:     	str	xzr, [sp, #0x1f8]
10003c1e4:     	str	q0, [x22, #0x130]
10003c1e8:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003c1ec:     	movi.2d	v0, #0000000000000000
10003c1f0:     	add	x0, sp, #0x200
10003c1f4:     	add	x1, sp, #0x200
10003c1f8:     	stp	q0, q0, [sp, #0x200]
10003c1fc:     	stp	q0, q0, [sp, #0x220]
10003c200:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10003c204:     	mov	w0, #0x1                ; =1
10003c208:     	bl	0x100069ebc <_m34_container_end>
10003c20c:     	add	x0, sp, #0x200
10003c210:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10003c214:     	ldr	x23, [sp, #0x28]
10003c218:     	add	x0, sp, #0x1e8
10003c21c:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003c220:     	str	x23, [sp, #0x28]
10003c224:     	ldr	x8, [x23, #0x18]
10003c228:     	cmp	x8, x19
10003c22c:     	b.ne	0x10003c254 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3f0>
10003c230:     	str	x23, [sp, #0x8]
10003c234:     	bl	0x1000497ac <_scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
10003c238:     	ldr	x8, [sp, #0x8]
10003c23c:     	str	x8, [sp, #0x28]
10003c240:     	ldr	x8, [x21]
10003c244:     	ldr	x8, [x8, #0x10]
10003c248:     	cmp	x8, x19
10003c24c:     	cset	w0, eq
10003c250:     	b	0x10003c258 <_scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3f4>
10003c254:     	mov	w0, wzr
10003c258:     	ldr	x8, [sp, #0x28]
10003c25c:     	str	x8, [sp, #0x8]
10003c260:     	bl	0x10003f560 <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
10003c264:     	ldr	x8, [sp, #0x8]
10003c268:     	movi.2d	v0, #0000000000000000
10003c26c:     	str	x8, [sp, #0x28]
10003c270:     	adrp	x8, 0x10017f000 <_scoop$1$sr$9bc66fb5e6596307e7ecccaa8e62af2a26973f3e59a3033be911251d8c2ae020+0xa0>
10003c274:     	add	x8, x8, #0x1e0
10003c278:     	add	x0, sp, #0x250
10003c27c:     	add	x1, sp, #0x240
10003c280:     	mov	w2, #0x1                ; =1
10003c284:     	str	x20, [sp, #0x240]
10003c288:     	str	x8, [sp, #0x248]
10003c28c:     	str	q0, [sp, #0x250]
10003c290:     	str	xzr, [sp, #0x260]
10003c294:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003c298:     	movi.2d	v0, #0000000000000000
10003c29c:     	sub	x0, x29, #0xf8
10003c2a0:     	sub	x1, x29, #0xf8
10003c2a4:     	stp	q0, q0, [x22, #0x1b0]
10003c2a8:     	stp	q0, q0, [x22, #0x1d0]
10003c2ac:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10003c2b0:     	bl	0x100069e44 <_m34_container_begin>
10003c2b4:     	sub	x0, x29, #0xf8
10003c2b8:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10003c2bc:     	ldr	x19, [sp, #0x28]
10003c2c0:     	add	x0, sp, #0x250
10003c2c4:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003c2c8:     	mov	x0, x19
10003c2cc:     	str	x19, [sp, #0x28]
10003c2d0:     	str	x19, [sp, #0x8]
10003c2d4:     	bl	0x10006b9b0 <dyld_stub_binder+0x10006b9b0>
10003c2d8:     	ldr	x8, [sp, #0x8]
10003c2dc:     	str	x8, [sp, #0x28]
10003c2e0:     	movi.2d	v0, #0000000000000000
10003c2e4:     	str	x8, [sp, #0x10]
10003c2e8:     	adrp	x8, 0x10017f000 <_scoop$1$sr$9bc66fb5e6596307e7ecccaa8e62af2a26973f3e59a3033be911251d8c2ae020+0xa0>
10003c2ec:     	add	x8, x8, #0x1f0
10003c2f0:     	sub	x0, x29, #0xa8
10003c2f4:     	sub	x1, x29, #0xb8
10003c2f8:     	mov	w2, #0x1                ; =1
10003c2fc:     	stp	x20, x8, [x29, #-0xb8]
10003c300:     	stur	xzr, [x29, #-0x98]
10003c304:     	str	q0, [x22, #0x200]
10003c308:     	bl	0x100065c34 <_scoop_rt_push_caller_roots>
10003c30c:     	movi.2d	v0, #0000000000000000
10003c310:     	sub	x0, x29, #0x90
10003c314:     	sub	x1, x29, #0x90
10003c318:     	stp	q0, q0, [x29, #-0x90]
10003c31c:     	stp	q0, q0, [x29, #-0x70]
10003c320:     	bl	0x100059184 <_scoop_rt_enter_native_safe>
10003c324:     	mov	w0, #0x3                ; =3
10003c328:     	bl	0x100069ebc <_m34_container_end>
10003c32c:     	sub	x0, x29, #0x90
10003c330:     	bl	0x10006467c <_scoop_rt_leave_native_safe>
10003c334:     	ldr	x19, [sp, #0x28]
10003c338:     	sub	x0, x29, #0xa8
10003c33c:     	bl	0x100065cdc <_scoop_rt_pop_caller_roots>
10003c340:     	str	x19, [sp, #0x28]
10003c344:     	ldr	x8, [x19, #0x18]
10003c348:     	cmp	x8, #0x0
10003c34c:     	cset	w0, eq
10003c350:     	bl	0x10003f560 <_scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
10003c354:     	bl	0x1000497ac <_scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
10003c358:     	ldr	x8, [x21]
10003c35c:     	ldr	x0, [x8, #0x10]
10003c360:     	add	sp, sp, #0x310
10003c364:     	ldp	x29, x30, [sp, #0x50]
10003c368:     	ldp	x20, x19, [sp, #0x40]
10003c36c:     	ldp	x22, x21, [sp, #0x30]
10003c370:     	ldp	x24, x23, [sp, #0x20]
10003c374:     	ldp	x26, x25, [sp, #0x10]
10003c378:     	ldp	x28, x27, [sp], #0x60
10003c37c:     	ret
