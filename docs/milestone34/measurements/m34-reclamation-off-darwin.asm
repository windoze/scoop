
/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/reclamation-off-darwin/region-churn:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002ee54 <_scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
10002ee54:     	sub	sp, sp, #0x100
10002ee58:     	stp	x26, x25, [sp, #0xb0]
10002ee5c:     	stp	x24, x23, [sp, #0xc0]
10002ee60:     	stp	x22, x21, [sp, #0xd0]
10002ee64:     	stp	x20, x19, [sp, #0xe0]
10002ee68:     	stp	x29, x30, [sp, #0xf0]
10002ee6c:     	add	x29, sp, #0xf0
10002ee70:     	adrp	x0, 0x100158000 <dyld_stub_binder+0x100158000>
10002ee74:     	add	x0, x0, #0x610
10002ee78:     	ldr	x8, [x0]
10002ee7c:     	blr	x8
10002ee80:     	adrp	x23, 0x100158000 <dyld_stub_binder+0x100158000>
10002ee84:     	adrp	x25, 0x100158000 <dyld_stub_binder+0x100158000>
10002ee88:     	add	x23, x23, #0x860
10002ee8c:     	ldr	x24, [x0]
10002ee90:     	add	x25, x25, #0x84c
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
10002efbc:     	bl	 <dyld_stub_binder+0x1000476d4>
10002efc0:     	ldp	x29, x30, [sp, #0xf0]
10002efc4:     	ldp	x20, x19, [sp, #0xe0]
10002efc8:     	ldp	x22, x21, [sp, #0xd0]
10002efcc:     	ldp	x24, x23, [sp, #0xc0]
10002efd0:     	ldp	x26, x25, [sp, #0xb0]
10002efd4:     	add	sp, sp, #0x100
10002efd8:     	ret

0000000100030dec <_scoop_gc_heap_plan_moving_locked>:
100030dec:     	sub	sp, sp, #0x70
100030df0:     	stp	d9, d8, [sp, #0x20]
100030df4:     	stp	x24, x23, [sp, #0x30]
100030df8:     	stp	x22, x21, [sp, #0x40]
100030dfc:     	stp	x20, x19, [sp, #0x50]
100030e00:     	stp	x29, x30, [sp, #0x60]
100030e04:     	add	x29, sp, #0x60
100030e08:     	adrp	x22, 0x100158000 <dyld_stub_binder+0x100158000>
100030e0c:     	add	x22, x22, #0x348
100030e10:     	ldrb	w8, [x22, #0x9d]
100030e14:     	tbz	w8, #0x0,  <L36>
100030e18:     	mov	x20, x0
100030e1c:     	bl	 <_scoop_heap_first_block>
100030e20:     	cbz	x0,  <L9>
100030e24:     	mov	x21, x0
100030e28:     	mov	x19, #0x0               ; =0
100030e2c:     	mov	w23, #0x3               ; =3
100030e30:     	b	 <L2>
<L0>:
100030e34:     	mov	x19, x21
<L1>:
100030e38:     	mov	x0, x21
100030e3c:     	bl	 <_scoop_heap_next_block>
100030e40:     	mov	x21, x0
100030e44:     	cbz	x0,  <L8>
<L2>:
100030e48:     	mov	x0, x21
100030e4c:     	bl	 <_scoop_heap_active_head>
100030e50:     	cbz	w0,  <L1>
100030e54:     	ldr	x8, [x21, #0x70]
100030e58:     	cbz	x8,  <L1>
100030e5c:     	cbz	w20,  <L3>
100030e60:     	ldr	w8, [x21, #0x1c]
100030e64:     	cbnz	w8,  <L1>
100030e68:     	ldr	w8, [x21, #0x18]
100030e6c:     	cmp	w8, #0x2
100030e70:     	b.ne	 <L6>
100030e74:     	ldrb	w8, [x21, #0x82]
100030e78:     	tbnz	w8, #0x0,  <L1>
100030e7c:     	str	w23, [x21, #0x14]
100030e80:     	b	 <L1>
<L3>:
100030e84:     	ldrb	w8, [x22, #0x99]
100030e88:     	cmp	w8, #0x1
100030e8c:     	b.ne	 <L5>
100030e90:     	str	w23, [x21, #0x14]
100030e94:     	ldr	w8, [x21, #0x18]
100030e98:     	cmp	w8, #0x1
100030e9c:     	b.ne	 <L1>
<L4>:
100030ea0:     	mov	w0, #0x1000             ; =4096
100030ea4:     	mov	w1, #0x8                ; =8
100030ea8:     	bl	 <dyld_stub_binder+0x1000474e8>
100030eac:     	str	x0, [x21, #0x58]
100030eb0:     	cbnz	x0,  <L1>
100030eb4:     	b	 <L35>
<L5>:
100030eb8:     	cbz	x19,  <L0>
100030ebc:     	ldr	d0, [x21, #0x68]
100030ec0:     	ucvtf	d8, d0
100030ec4:     	mov	x0, x21
100030ec8:     	bl	 <_scoop_heap_block_bytes>
100030ecc:     	ucvtf	d0, x0
100030ed0:     	fdiv	d8, d8, d0
100030ed4:     	ldr	d0, [x19, #0x68]
100030ed8:     	ucvtf	d9, d0
100030edc:     	mov	x0, x19
100030ee0:     	bl	 <_scoop_heap_block_bytes>
100030ee4:     	ucvtf	d0, x0
100030ee8:     	fdiv	d0, d9, d0
100030eec:     	fcmp	d8, d0
100030ef0:     	b.mi	 <L0>
100030ef4:     	b	 <L1>
<L6>:
100030ef8:     	mov	x9, #0x0                ; =0
100030efc:     	ldr	x10, [x21, #0x30]
<L7>:
100030f00:     	ldr	x11, [x10, x9]
100030f04:     	cbnz	x11,  <L1>
100030f08:     	add	x9, x9, #0x8
100030f0c:     	cmp	x9, #0x200
100030f10:     	b.ne	 <L7>
100030f14:     	str	w23, [x21, #0x14]
100030f18:     	cmp	w8, #0x1
100030f1c:     	b.ne	 <L1>
100030f20:     	b	 <L4>
<L8>:
100030f24:     	tbnz	w20, #0x0,  <L9>
100030f28:     	ldrb	w8, [x22, #0x99]
100030f2c:     	tbnz	w8, #0x0,  <L9>
100030f30:     	cbz	x19,  <L9>
100030f34:     	mov	w8, #0x3                ; =3
100030f38:     	str	w8, [x19, #0x14]
100030f3c:     	ldr	w8, [x19, #0x18]
100030f40:     	cmp	w8, #0x1
100030f44:     	b.ne	 <L9>
100030f48:     	mov	w0, #0x1000             ; =4096
100030f4c:     	mov	w1, #0x8                ; =8
100030f50:     	bl	 <dyld_stub_binder+0x1000474e8>
100030f54:     	str	x0, [x19, #0x58]
100030f58:     	cbz	x0,  <L35>
<L9>:
100030f5c:     	stp	xzr, xzr, [sp, #0x10]
100030f60:     	str	xzr, [sp, #0x8]
100030f64:     	bl	 <_scoop_heap_first_block>
100030f68:     	cbz	x0,  <L21>
100030f6c:     	mov	x19, x0
<L10>:
100030f70:     	ldr	w8, [x19, #0x14]
100030f74:     	cmp	w8, #0x3
100030f78:     	b.ne	 <L11>
100030f7c:     	ldr	w8, [x19, #0x18]
100030f80:     	cmp	w8, #0x2
100030f84:     	b.ne	 <L13>
100030f88:     	add	x0, sp, #0x18
100030f8c:     	add	x1, sp, #0x10
100030f90:     	add	x2, sp, #0x8
100030f94:     	mov	x3, x19
100030f98:     	mov	w4, #0x10               ; =16
100030f9c:     	bl	 <_append_move>
100030fa0:     	mov	x21, x0
100030fa4:     	b	 <L12>
<L11>:
100030fa8:     	mov	w21, #0x1               ; =1
<L12>:
100030fac:     	mov	x0, x19
100030fb0:     	bl	 <_scoop_heap_next_block>
100030fb4:     	cbz	w21,  <L17>
100030fb8:     	mov	x19, x0
100030fbc:     	cbnz	x0,  <L10>
100030fc0:     	b	 <L17>
<L13>:
100030fc4:     	mov	w20, #0x10              ; =16
100030fc8:     	b	 <L16>
<L14>:
100030fcc:     	mov	w21, #0x1               ; =1
100030fd0:     	cbz	w21,  <L12>
<L15>:
100030fd4:     	cmp	x20, #0xfff
100030fd8:     	add	x20, x20, #0x1
100030fdc:     	b.hs	 <L12>
<L16>:
100030fe0:     	ldr	x0, [x19, #0x20]
100030fe4:     	mov	x1, x20
100030fe8:     	bl	 <_scoop_heap_bit_test>
100030fec:     	cbz	w0,  <L14>
100030ff0:     	ldr	x0, [x19, #0x28]
100030ff4:     	mov	x1, x20
100030ff8:     	bl	 <_scoop_heap_bit_test>
100030ffc:     	cbz	w0,  <L14>
100031000:     	ldr	x0, [x19, #0x30]
100031004:     	mov	x1, x20
100031008:     	bl	 <_scoop_heap_bit_test>
10003100c:     	tbnz	w0, #0x0,  <L14>
100031010:     	add	x0, sp, #0x18
100031014:     	add	x1, sp, #0x10
100031018:     	add	x2, sp, #0x8
10003101c:     	mov	x3, x19
100031020:     	mov	x4, x20
100031024:     	bl	 <_append_move>
100031028:     	mov	x21, x0
10003102c:     	cbnz	w21,  <L15>
100031030:     	b	 <L12>
<L17>:
100031034:     	cbz	w21,  <L22>
100031038:     	ldp	x19, x0, [sp, #0x10]
10003103c:     	cbz	x19,  <L33>
100031040:     	mov	x20, x0
100031044:     	add	x21, x0, #0x18
100031048:     	mov	w23, #0x7f80            ; =32640
10003104c:     	b	 <L20>
<L18>:
100031050:     	mov	w1, #0x1                ; =1
100031054:     	bl	 <_scoop_heap_publish_large_object>
100031058:     	ldur	x8, [x21, #-0x10]
10003105c:     	str	x8, [x24, #0x78]
<L19>:
100031060:     	ldr	x8, [x22, #0x1a8]
100031064:     	add	x8, x8, #0x1
100031068:     	str	x8, [x22, #0x1a8]
10003106c:     	add	x21, x21, #0x30
100031070:     	subs	x19, x19, #0x1
100031074:     	b.eq	 <L32>
<L20>:
100031078:     	ldp	x1, x0, [x21, #-0x18]
10003107c:     	ldur	x2, [x21, #-0x8]
100031080:     	bl	 <dyld_stub_binder+0x1000475a8>
100031084:     	ldur	x2, [x21, #-0x8]
100031088:     	ldr	x8, [x22, #0x128]
10003108c:     	add	x8, x8, x2
100031090:     	str	x8, [x22, #0x128]
100031094:     	ldp	x24, x0, [x21, #0x8]
100031098:     	cmp	x2, x23
10003109c:     	b.hi	 <L18>
1000310a0:     	ldur	x1, [x21, #-0x10]
1000310a4:     	mov	w3, #0x1                ; =1
1000310a8:     	bl	 <_scoop_heap_record_small_object>
1000310ac:     	ldur	x8, [x21, #-0x10]
1000310b0:     	ldr	x9, [x24, #0x58]
1000310b4:     	ldr	x10, [x21]
1000310b8:     	str	x8, [x9, x10, lsl #3]
1000310bc:     	b	 <L19>
<L21>:
1000310c0:     	ldr	x0, [sp, #0x18]
1000310c4:     	b	 <L33>
<L22>:
1000310c8:     	ldr	x0, [sp, #0x18]
1000310cc:     	bl	 <dyld_stub_binder+0x10004753c>
1000310d0:     	bl	 <_scoop_heap_first_block>
1000310d4:     	cbnz	x0,  <L27>
<L23>:
1000310d8:     	stp	xzr, xzr, [x22, #0x198]
1000310dc:     	str	xzr, [x22, #0x190]
1000310e0:     	ldrb	w8, [x22, #0x99]
1000310e4:     	cmp	w8, #0x1
1000310e8:     	b.eq	 <L37>
1000310ec:     	mov	w0, #0x0                ; =0
1000310f0:     	b	 <L34>
<L24>:
1000310f4:     	mov	w8, #0x5                ; =5
<L25>:
1000310f8:     	str	w8, [x0, #0x14]
1000310fc:     	ldr	x8, [x0, #0x58]
100031100:     	mov	x19, x0
100031104:     	mov	x0, x8
100031108:     	bl	 <dyld_stub_binder+0x10004753c>
10003110c:     	mov	x0, x19
100031110:     	str	xzr, [x19, #0x58]
<L26>:
100031114:     	bl	 <_scoop_heap_next_block>
100031118:     	cbz	x0,  <L23>
<L27>:
10003111c:     	ldr	w8, [x0, #0x14]
100031120:     	cmp	w8, #0x3
100031124:     	b.eq	 <L28>
100031128:     	cmp	w8, #0x4
10003112c:     	b.ne	 <L26>
100031130:     	mov	x19, x0
100031134:     	bl	 <_scoop_heap_release_block>
100031138:     	mov	x0, x19
10003113c:     	b	 <L26>
<L28>:
100031140:     	ldr	w8, [x0, #0x18]
100031144:     	cmp	w8, #0x2
100031148:     	b.ne	 <L29>
10003114c:     	ldrb	w8, [x0, #0x82]
100031150:     	cmp	w8, #0x1
100031154:     	b.eq	 <L24>
100031158:     	b	 <L31>
<L29>:
10003115c:     	mov	x8, #0x0                ; =0
100031160:     	ldr	x9, [x0, #0x30]
<L30>:
100031164:     	ldr	x10, [x9, x8]
100031168:     	cbnz	x10,  <L24>
10003116c:     	add	x8, x8, #0x8
100031170:     	cmp	x8, #0x200
100031174:     	b.ne	 <L30>
<L31>:
100031178:     	mov	w8, #0x2                ; =2
10003117c:     	b	 <L25>
<L32>:
100031180:     	mov	x0, x20
<L33>:
100031184:     	bl	 <dyld_stub_binder+0x10004753c>
100031188:     	mov	w0, #0x1                ; =1
<L34>:
10003118c:     	ldp	x29, x30, [sp, #0x60]
100031190:     	ldp	x20, x19, [sp, #0x50]
100031194:     	ldp	x22, x21, [sp, #0x40]
100031198:     	ldp	x24, x23, [sp, #0x30]
10003119c:     	ldp	d9, d8, [sp, #0x20]
1000311a0:     	add	sp, sp, #0x70
1000311a4:     	ret
<L35>:
1000311a8:     	adrp	x0, 0x100060000 <_scoop$1$bs$bb578786bf4240588934d5a9be3466c627f286f139ee556eb89aeb8e519f78db+0x20>
1000311ac:     	add	x0, x0, #0xed8
1000311b0:     	bl	 <_scoop_heap_fatal>
<L36>:
1000311b4:     	adrp	x0, 0x100060000 <_scoop$1$bs$bb578786bf4240588934d5a9be3466c627f286f139ee556eb89aeb8e519f78db+0x20>
1000311b8:     	add	x0, x0, #0xe7e
1000311bc:     	bl	 <_scoop_heap_fatal>
<L37>:
1000311c0:     	adrp	x0, 0x100060000 <_scoop$1$bs$bb578786bf4240588934d5a9be3466c627f286f139ee556eb89aeb8e519f78db+0x20>
1000311c4:     	add	x0, x0, #0xea4
1000311c8:     	bl	 <_scoop_heap_fatal>

0000000100031c38 <_scoop_gc_heap_finish_collection_locked>:
100031c38:     	sub	sp, sp, #0x90
100031c3c:     	stp	x28, x27, [sp, #0x30]
100031c40:     	stp	x26, x25, [sp, #0x40]
100031c44:     	stp	x24, x23, [sp, #0x50]
100031c48:     	stp	x22, x21, [sp, #0x60]
100031c4c:     	stp	x20, x19, [sp, #0x70]
100031c50:     	stp	x29, x30, [sp, #0x80]
100031c54:     	add	x29, sp, #0x80
100031c58:     	adrp	x25, 0x100158000 <dyld_stub_binder+0x100158000>
100031c5c:     	add	x25, x25, #0x348
100031c60:     	ldrb	w8, [x25, #0x9d]
100031c64:     	tbz	w8, #0x0,  <L43>
100031c68:     	mov	x19, x1
100031c6c:     	mov	x20, x0
100031c70:     	add	x8, sp, #0x10
100031c74:     	bl	 <_scoop_thread_allocation_totals_locked>
100031c78:     	cbz	w19,  <L0>
100031c7c:     	ldp	x8, x9, [sp, #0x10]
100031c80:     	ldp	x11, x10, [x25, #0x80]
100031c84:     	ldr	x12, [x25, #0x78]
100031c88:     	add	x8, x8, x20
100031c8c:     	sub	x8, x8, x9
100031c90:     	sub	x8, x8, x11
100031c94:     	add	x9, x10, x12
100031c98:     	add	x20, x8, x9
100031c9c:     	b	 <L1>
<L0>:
100031ca0:     	bl	 <_scoop_heap_free_run_nodes>
<L1>:
100031ca4:     	str	x20, [sp, #0x8]
100031ca8:     	bl	 <_scoop_heap_first_block>
100031cac:     	cbz	x0,  <L32>
100031cb0:     	mov	x21, x0
100031cb4:     	mov	w24, #0x1               ; =1
100031cb8:     	mov	w27, #0x4               ; =4
100031cbc:     	b	 <L4>
<L2>:
100031cc0:     	ldrb	w8, [x21, #0x82]
100031cc4:     	cmp	w8, #0x0
100031cc8:     	mov	w8, #0x2                ; =2
100031ccc:     	mov	w9, #0x5                ; =5
100031cd0:     	csel	w8, w9, w8, ne
100031cd4:     	str	w8, [x21, #0x14]
100031cd8:     	stp	xzr, xzr, [x21, #0x68]
100031cdc:     	str	xzr, [x21, #0x78]
100031ce0:     	strb	wzr, [x21, #0x81]
100031ce4:     	strb	wzr, [x21, #0x83]
<L3>:
100031ce8:     	mov	x0, x21
100031cec:     	bl	 <_scoop_heap_next_block>
100031cf0:     	mov	x21, x0
100031cf4:     	cbz	x0,  <L32>
<L4>:
100031cf8:     	mov	x0, x21
100031cfc:     	bl	 <_scoop_heap_active_head>
100031d00:     	cbz	w0,  <L3>
100031d04:     	ldr	w8, [x21, #0x1c]
100031d08:     	cbz	w19,  <L5>
100031d0c:     	cmp	w8, #0x1
100031d10:     	b.ne	 <L5>
100031d14:     	ldr	w8, [x21, #0x14]
100031d18:     	cmp	w8, #0x4
100031d1c:     	b.ne	 <L3>
100031d20:     	b	 <L6>
<L5>:
100031d24:     	cbnz	w8,  <L6>
100031d28:     	ldr	x8, [x21, #0x68]
100031d2c:     	ldr	x9, [x25, #0xd0]
100031d30:     	add	x8, x9, x8
100031d34:     	str	x8, [x25, #0xd0]
<L6>:
100031d38:     	str	w24, [x21, #0x1c]
100031d3c:     	ldr	w8, [x21, #0x18]
100031d40:     	cmp	w8, #0x2
100031d44:     	b.eq	 <L16>
100031d48:     	cmp	w8, #0x1
100031d4c:     	b.ne	 <L42>
100031d50:     	mov	w24, #0x0               ; =0
100031d54:     	mov	w20, #0x0               ; =0
100031d58:     	ldrb	w28, [x25, #0x99]
100031d5c:     	ldr	x8, [x21, #0x40]
100031d60:     	movi.2d	v0, #0000000000000000
100031d64:     	stp	q0, q0, [x8]
100031d68:     	mov	w22, #0x10              ; =16
100031d6c:     	b	 <L9>
<L7>:
100031d70:     	orr	w20, w20, w23
100031d74:     	mov	w24, #0x1               ; =1
<L8>:
100031d78:     	add	x22, x22, #0x1
100031d7c:     	cmp	x22, #0x1, lsl #12      ; =0x1000
100031d80:     	b.eq	 <L18>
<L9>:
100031d84:     	ldr	x0, [x21, #0x20]
100031d88:     	mov	x1, x22
100031d8c:     	bl	 <_scoop_heap_bit_test>
100031d90:     	cbz	w0,  <L8>
100031d94:     	ldr	x0, [x21, #0x30]
100031d98:     	mov	x1, x22
100031d9c:     	bl	 <_scoop_heap_bit_test>
100031da0:     	mov	x23, x0
100031da4:     	ldr	x0, [x21, #0x28]
100031da8:     	mov	x1, x22
100031dac:     	bl	 <_scoop_heap_bit_test>
100031db0:     	ldr	w8, [x21, #0x14]
100031db4:     	cmp	w8, #0x3
100031db8:     	cset	w8, eq
100031dbc:     	eor	w9, w23, #0x1
100031dc0:     	and	w8, w8, w9
100031dc4:     	cmp	w8, #0x1
100031dc8:     	ccmp	w0, #0x0, #0x4, eq
100031dcc:     	b.eq	 <L10>
100031dd0:     	ldr	x9, [x21, #0x58]
100031dd4:     	ldr	x9, [x9, x22, lsl #3]
100031dd8:     	cbz	x9,  <L36>
<L10>:
100031ddc:     	eor	w8, w8, #0x1
100031de0:     	and	w8, w0, w8
100031de4:     	ldr	x9, [x21, #0x50]
100031de8:     	ldrh	w26, [x9, x22, lsl #1]
100031dec:     	tbz	w8, #0x0,  <L12>
100031df0:     	cbz	w26,  <L38>
100031df4:     	add	x8, x22, x26
100031df8:     	lsl	x8, x8, #3
100031dfc:     	sub	x8, x8, #0x1
100031e00:     	lsr	x24, x22, #4
100031e04:     	cmp	x24, x8, lsr #7
100031e08:     	b.hi	 <L7>
100031e0c:     	lsr	x8, x8, #7
100031e10:     	add	x26, x8, #0x1
<L11>:
100031e14:     	ldr	x0, [x21, #0x40]
100031e18:     	mov	x1, x24
100031e1c:     	bl	 <_scoop_heap_bit_set>
100031e20:     	add	x24, x24, #0x1
100031e24:     	cmp	x26, x24
100031e28:     	b.ne	 <L11>
100031e2c:     	b	 <L7>
<L12>:
100031e30:     	cbz	w26,  <L37>
100031e34:     	tbnz	w0, #0x0,  <L14>
100031e38:     	mov	x0, x21
100031e3c:     	bl	 <_scoop_heap_block_base>
100031e40:     	add	x0, x0, x22, lsl #3
100031e44:     	ldr	x8, [x0]
100031e48:     	ldr	x8, [x8, #0x90]
100031e4c:     	cbz	x8,  <L13>
100031e50:     	add	x9, x0, #0x8
100031e54:     	ldclral	x27, x9, [x9]
100031e58:     	tbz	w9, #0x2,  <L14>
100031e5c:     	blr	x8
100031e60:     	b	 <L14>
<L13>:
100031e64:     	add	x8, x0, #0x8
100031e68:     	ldapr	x8, [x8]
100031e6c:     	tbnz	w8, #0x2,  <L41>
<L14>:
100031e70:     	cbz	w28,  <L15>
100031e74:     	lsl	x23, x26, #3
100031e78:     	mov	x0, x21
100031e7c:     	bl	 <_scoop_heap_block_base>
100031e80:     	add	x0, x0, x22, lsl #3
100031e84:     	mov	w1, #0xa5               ; =165
100031e88:     	mov	x2, x23
100031e8c:     	bl	 <dyld_stub_binder+0x1000475b4>
<L15>:
100031e90:     	ldr	x0, [x21, #0x20]
100031e94:     	mov	x1, x22
100031e98:     	bl	 <_scoop_heap_bit_clear>
100031e9c:     	ldr	x0, [x21, #0x30]
100031ea0:     	mov	x1, x22
100031ea4:     	bl	 <_scoop_heap_bit_clear>
100031ea8:     	ldr	x8, [x21, #0x50]
100031eac:     	strh	wzr, [x8, x22, lsl #1]
100031eb0:     	b	 <L8>
<L16>:
100031eb4:     	ldrb	w20, [x25, #0x99]
100031eb8:     	ldr	w8, [x21, #0x14]
100031ebc:     	cmp	w8, #0x3
100031ec0:     	b.ne	 <L17>
100031ec4:     	ldrb	w8, [x21, #0x82]
100031ec8:     	tbz	w8, #0x0,  <L25>
<L17>:
100031ecc:     	ldrb	w8, [x21, #0x81]
100031ed0:     	tbnz	w8, #0x0,  <L2>
100031ed4:     	b	 <L26>
<L18>:
100031ed8:     	ldr	x8, [x21, #0x28]
100031edc:     	movi.2d	v0, #0000000000000000
100031ee0:     	stp	q0, q0, [x8, #0x1e0]
100031ee4:     	stp	q0, q0, [x8, #0x1c0]
100031ee8:     	stp	q0, q0, [x8, #0x1a0]
100031eec:     	stp	q0, q0, [x8, #0x180]
100031ef0:     	stp	q0, q0, [x8, #0x160]
100031ef4:     	stp	q0, q0, [x8, #0x140]
100031ef8:     	stp	q0, q0, [x8, #0x120]
100031efc:     	stp	q0, q0, [x8, #0x100]
100031f00:     	stp	q0, q0, [x8, #0xe0]
100031f04:     	stp	q0, q0, [x8, #0xc0]
100031f08:     	stp	q0, q0, [x8, #0xa0]
100031f0c:     	stp	q0, q0, [x8, #0x80]
100031f10:     	stp	q0, q0, [x8, #0x60]
100031f14:     	stp	q0, q0, [x8, #0x40]
100031f18:     	stp	q0, q0, [x8, #0x20]
100031f1c:     	stp	q0, q0, [x8]
100031f20:     	ldr	x8, [x21, #0x38]
100031f24:     	stp	q0, q0, [x8, #0x1e0]
100031f28:     	stp	q0, q0, [x8, #0x1c0]
100031f2c:     	stp	q0, q0, [x8, #0x1a0]
100031f30:     	stp	q0, q0, [x8, #0x180]
100031f34:     	stp	q0, q0, [x8, #0x160]
100031f38:     	stp	q0, q0, [x8, #0x140]
100031f3c:     	stp	q0, q0, [x8, #0x120]
100031f40:     	stp	q0, q0, [x8, #0x100]
100031f44:     	stp	q0, q0, [x8, #0xe0]
100031f48:     	stp	q0, q0, [x8, #0xc0]
100031f4c:     	stp	q0, q0, [x8, #0xa0]
100031f50:     	stp	q0, q0, [x8, #0x80]
100031f54:     	stp	q0, q0, [x8, #0x60]
100031f58:     	stp	q0, q0, [x8, #0x40]
100031f5c:     	stp	q0, q0, [x8, #0x20]
100031f60:     	stp	q0, q0, [x8]
100031f64:     	ldr	x8, [x21, #0x48]
100031f68:     	stp	q0, q0, [x8]
100031f6c:     	ldr	x0, [x21, #0x58]
100031f70:     	bl	 <dyld_stub_binder+0x10004753c>
100031f74:     	str	xzr, [x21, #0x58]
100031f78:     	stp	xzr, xzr, [x21, #0x68]
100031f7c:     	tbz	w24, #0x0,  <L24>
100031f80:     	tst	w20, #0x1
100031f84:     	mov	w8, #0x2                ; =2
100031f88:     	mov	w9, #0x5                ; =5
100031f8c:     	csel	w8, w9, w8, ne
100031f90:     	str	w8, [x21, #0x14]
100031f94:     	mov	w24, #0x1               ; =1
100031f98:     	tbnz	w28, #0x0,  <L3>
100031f9c:     	mov	x20, #0x0               ; =0
100031fa0:     	mov	w22, #0x1               ; =1
100031fa4:     	b	 <L21>
<L19>:
100031fa8:     	mov	x1, x20
<L20>:
100031fac:     	add	x22, x22, #0x1
100031fb0:     	mov	x20, x1
100031fb4:     	cmp	x22, #0x101
100031fb8:     	b.eq	 <L3>
<L21>:
100031fbc:     	cmp	x22, #0x100
100031fc0:     	b.ne	 <L23>
100031fc4:     	cmp	x20, #0x0
100031fc8:     	cset	w8, ne
100031fcc:     	mov	w0, #0x1                ; =1
<L22>:
100031fd0:     	cmp	w0, #0x0
100031fd4:     	ccmp	w8, #0x0, #0x4, ne
100031fd8:     	b.eq	 <L19>
100031fdc:     	subs	x23, x22, x20
100031fe0:     	b.eq	 <L39>
100031fe4:     	mov	w0, #0x18               ; =24
100031fe8:     	bl	 <dyld_stub_binder+0x100047584>
100031fec:     	cbz	x0,  <L40>
100031ff0:     	mov	x1, #0x0                ; =0
100031ff4:     	ldr	x8, [x25, #0x58]
100031ff8:     	stp	x8, x21, [x0]
100031ffc:     	strh	w20, [x0, #0x10]
100032000:     	strh	w23, [x0, #0x12]
100032004:     	str	wzr, [x0, #0x14]
100032008:     	str	x0, [x25, #0x58]
10003200c:     	b	 <L20>
<L23>:
100032010:     	ldr	x0, [x21, #0x40]
100032014:     	mov	x1, x22
100032018:     	bl	 <_scoop_heap_bit_test>
10003201c:     	cmp	x20, #0x0
100032020:     	cset	w8, ne
100032024:     	tbnz	w0, #0x0,  <L22>
100032028:     	mov	x1, x22
10003202c:     	cbz	x20,  <L20>
100032030:     	b	 <L22>
<L24>:
100032034:     	mov	x0, x21
100032038:     	cbz	w28,  <L27>
10003203c:     	bl	 <_scoop_heap_quarantine_block>
100032040:     	b	 <L28>
<L25>:
100032044:     	ldrb	w8, [x21, #0x81]
100032048:     	cmp	w8, #0x1
10003204c:     	b.ne	 <L26>
100032050:     	ldr	x8, [x21, #0x78]
100032054:     	cbnz	x8,  <L30>
100032058:     	b	 <L44>
<L26>:
10003205c:     	mov	x0, x21
100032060:     	bl	 <_scoop_heap_block_base>
100032064:     	mov	x8, x0
100032068:     	ldr	x9, [x0, #0x80]!
10003206c:     	ldr	x9, [x9, #0x90]
100032070:     	cbz	x9,  <L29>
100032074:     	add	x8, x8, #0x88
100032078:     	ldclral	x27, x8, [x8]
10003207c:     	tbz	w8, #0x2,  <L30>
100032080:     	blr	x9
100032084:     	b	 <L30>
<L27>:
100032088:     	bl	 <_scoop_heap_release_block>
<L28>:
10003208c:     	mov	w24, #0x1               ; =1
100032090:     	b	 <L3>
<L29>:
100032094:     	add	x8, x8, #0x88
100032098:     	ldapr	x8, [x8]
10003209c:     	tbnz	w8, #0x2,  <L41>
<L30>:
1000320a0:     	mov	x0, x21
1000320a4:     	cbz	w20,  <L31>
1000320a8:     	bl	 <_scoop_heap_block_base>
1000320ac:     	ldr	x2, [x21, #0x60]
1000320b0:     	add	x0, x0, #0x80
1000320b4:     	mov	w1, #0xa5               ; =165
1000320b8:     	bl	 <dyld_stub_binder+0x1000475b4>
1000320bc:     	mov	x0, x21
1000320c0:     	bl	 <_scoop_heap_quarantine_block>
1000320c4:     	b	 <L3>
<L31>:
1000320c8:     	bl	 <_scoop_heap_release_block>
1000320cc:     	b	 <L3>
<L32>:
1000320d0:     	stp	xzr, xzr, [x25, #0x190]
1000320d4:     	str	xzr, [x25, #0x1a0]
1000320d8:     	bl	 <_scoop_heap_release_empty_large_regions>
1000320dc:     	ldr	x20, [x25, #0x40]
1000320e0:     	cbz	x20,  <L34>
<L33>:
1000320e4:     	ldp	x8, x0, [x20, #0x8]
1000320e8:     	lsr	x1, x8, #9
1000320ec:     	bl	 <dyld_stub_binder+0x1000474dc>
1000320f0:     	ldr	x20, [x20, #0x18]
1000320f4:     	cbnz	x20,  <L33>
<L34>:
1000320f8:     	str	xzr, [x25, #0xa0]
1000320fc:     	ldr	x8, [sp, #0x8]
100032100:     	str	x8, [x25, #0x78]
100032104:     	ldr	q0, [sp, #0x10]
100032108:     	str	q0, [x25, #0x80]
10003210c:     	ldr	x8, [x25, #0x1a8]
100032110:     	add	x9, x25, #0x90
100032114:     	stlr	x8, [x9]
100032118:     	tbnz	w19, #0x0,  <L35>
10003211c:     	ldr	x8, [x25, #0x60]
100032120:     	lsl	x9, x8, #1
100032124:     	mov	w10, #0x1000000         ; =16777216
100032128:     	cmp	x9, x10
10003212c:     	csel	x9, x9, x10, hi
100032130:     	cmn	x8, #0x1
100032134:     	csinv	x8, x9, xzr, gt
100032138:     	str	x8, [x25, #0x68]
<L35>:
10003213c:     	strb	wzr, [x25, #0x9d]
100032140:     	ldp	x29, x30, [sp, #0x80]
100032144:     	ldp	x20, x19, [sp, #0x70]
100032148:     	ldp	x22, x21, [sp, #0x60]
10003214c:     	ldp	x24, x23, [sp, #0x50]
100032150:     	ldp	x26, x25, [sp, #0x40]
100032154:     	ldp	x28, x27, [sp, #0x30]
100032158:     	add	sp, sp, #0x90
10003215c:     	ret
<L36>:
100032160:     	adrp	x0, 0x100061000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x3c0>
100032164:     	add	x0, x0, #0x118
100032168:     	bl	 <_scoop_heap_fatal>
<L37>:
10003216c:     	adrp	x0, 0x100061000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x3c0>
100032170:     	add	x0, x0, #0x147
100032174:     	bl	 <_scoop_heap_fatal>
<L38>:
100032178:     	adrp	x0, 0x100061000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x3c0>
10003217c:     	add	x0, x0, #0x16c
100032180:     	bl	 <_scoop_heap_fatal>
<L39>:
100032184:     	adrp	x0, 0x100061000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x3c0>
100032188:     	add	x0, x0, #0x1b5
10003218c:     	bl	 <_scoop_heap_fatal>
<L40>:
100032190:     	adrp	x0, 0x100061000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x3c0>
100032194:     	add	x0, x0, #0x1cb
100032198:     	bl	 <_scoop_heap_fatal>
<L41>:
10003219c:     	adrp	x0, 0x100061000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x3c0>
1000321a0:     	add	x0, x0, #0x18c
1000321a4:     	bl	 <_scoop_heap_fatal>
<L42>:
1000321a8:     	adrp	x0, 0x100061000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x3c0>
1000321ac:     	add	x0, x0, #0xf8
1000321b0:     	bl	 <_scoop_heap_fatal>
<L43>:
1000321b4:     	adrp	x0, 0x100061000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x3c0>
1000321b8:     	add	x0, x0, #0xd1
1000321bc:     	bl	 <_scoop_heap_fatal>
<L44>:
1000321c0:     	adrp	x0, 0x100061000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x3c0>
1000321c4:     	add	x0, x0, #0x1f3
1000321c8:     	bl	 <_scoop_heap_fatal>

0000000100033b20 <_scoop_heap_region_destroy>:
100033b20:     	sub	sp, sp, #0x60
100033b24:     	stp	x24, x23, [sp, #0x20]
100033b28:     	stp	x22, x21, [sp, #0x30]
100033b2c:     	stp	x20, x19, [sp, #0x40]
100033b30:     	stp	x29, x30, [sp, #0x50]
100033b34:     	add	x29, sp, #0x50
100033b38:     	mov	x19, x0
100033b3c:     	bl	 <_scoop_heap_page_map_remove>
100033b40:     	ldp	x20, x21, [x19]
100033b44:     	stp	xzr, xzr, [sp, #0x8]
100033b48:     	str	xzr, [sp, #0x18]
100033b4c:     	bl	 <_scoop_platform_bundle>
100033b50:     	ldr	x8, [x0, #0x8]
100033b54:     	ldr	x8, [x8, #0x20]
100033b58:     	add	x2, sp, #0x8
100033b5c:     	mov	x0, x20
100033b60:     	mov	x1, x21
100033b64:     	blr	x8
100033b68:     	tbz	w0, #0x0,  <L2>
100033b6c:     	mov	x20, #0x0               ; =0
100033b70:     	mov	x22, #0x0               ; =0
100033b74:     	ldrb	w21, [x19, #0x2a]
<L0>:
100033b78:     	ldr	x8, [x19, #0x20]
100033b7c:     	add	x23, x8, x20
100033b80:     	ldr	x0, [x23, #0x20]
100033b84:     	bl	 <dyld_stub_binder+0x10004753c>
100033b88:     	ldr	x0, [x23, #0x28]
100033b8c:     	bl	 <dyld_stub_binder+0x10004753c>
100033b90:     	ldr	x0, [x23, #0x30]
100033b94:     	bl	 <dyld_stub_binder+0x10004753c>
100033b98:     	ldr	x0, [x23, #0x38]
100033b9c:     	bl	 <dyld_stub_binder+0x10004753c>
100033ba0:     	ldr	x0, [x23, #0x40]
100033ba4:     	bl	 <dyld_stub_binder+0x10004753c>
100033ba8:     	ldr	x0, [x23, #0x48]
100033bac:     	bl	 <dyld_stub_binder+0x10004753c>
100033bb0:     	ldr	x0, [x23, #0x50]
100033bb4:     	bl	 <dyld_stub_binder+0x10004753c>
100033bb8:     	ldr	x0, [x23, #0x58]
100033bbc:     	bl	 <dyld_stub_binder+0x10004753c>
100033bc0:     	tbnz	w21, #0x0,  <L1>
100033bc4:     	add	x20, x20, #0x88
100033bc8:     	cmp	x22, #0x1fe
100033bcc:     	add	x22, x22, #0x1
100033bd0:     	b.ls	 <L0>
<L1>:
100033bd4:     	ldr	x0, [x19, #0x20]
100033bd8:     	bl	 <dyld_stub_binder+0x10004753c>
100033bdc:     	ldr	x0, [x19, #0x10]
100033be0:     	bl	 <dyld_stub_binder+0x10004753c>
100033be4:     	mov	x0, x19
100033be8:     	bl	 <dyld_stub_binder+0x10004753c>
100033bec:     	ldp	x29, x30, [sp, #0x50]
100033bf0:     	ldp	x20, x19, [sp, #0x40]
100033bf4:     	ldp	x22, x21, [sp, #0x30]
100033bf8:     	ldp	x24, x23, [sp, #0x20]
100033bfc:     	add	sp, sp, #0x60
100033c00:     	ret
<L2>:
100033c04:     	ldr	w0, [sp, #0x8]
100033c08:     	bl	 <_scoop_platform_error_message>
100033c0c:     	bl	 <_scoop_heap_fatal>
