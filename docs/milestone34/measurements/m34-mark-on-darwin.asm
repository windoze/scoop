
/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/marker-on-darwin/mark-graphs:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100031f14 <_scoop_gc_mark_slot>:
100031f14:     	sub	sp, sp, #0xa0
100031f18:     	stp	x28, x27, [sp, #0x40]
100031f1c:     	stp	x26, x25, [sp, #0x50]
100031f20:     	stp	x24, x23, [sp, #0x60]
100031f24:     	stp	x22, x21, [sp, #0x70]
100031f28:     	stp	x20, x19, [sp, #0x80]
100031f2c:     	stp	x29, x30, [sp, #0x90]
100031f30:     	add	x29, sp, #0x90
100031f34:     	ldr	w8, [x1, #0x8]
100031f38:     	ldr	x19, [x1]
100031f3c:     	cmp	w8, #0x1
100031f40:     	b.eq	 <L0>
100031f44:     	cbnz	w8,  <L1>
100031f48:     	ldr	x8, [x19, #0xe8]
100031f4c:     	add	x8, x8, #0x1
100031f50:     	str	x8, [x19, #0xe8]
100031f54:     	ldr	x20, [x0]
100031f58:     	cbnz	x20,  <L2>
100031f5c:     	b	 <L11>
<L0>:
100031f60:     	ldr	x8, [x19, #0xf0]
100031f64:     	add	x8, x8, #0x1
100031f68:     	str	x8, [x19, #0xf0]
100031f6c:     	ldr	x20, [x0]
100031f70:     	cbnz	x20,  <L2>
100031f74:     	b	 <L11>
<L1>:
100031f78:     	ldr	x8, [x19, #0xf8]
100031f7c:     	add	x8, x8, #0x1
100031f80:     	str	x8, [x19, #0xf8]
100031f84:     	ldr	x20, [x0]
100031f88:     	cbz	x20,  <L11>
<L2>:
100031f8c:     	add	x1, sp, #0x10
100031f90:     	add	x2, sp, #0x8
100031f94:     	mov	x0, x20
100031f98:     	bl	 <_scoop_heap_object_meta>
100031f9c:     	tbz	w0, #0x0,  <L10>
100031fa0:     	ldrb	w9, [x19, #0x98]
100031fa4:     	ldr	x8, [sp, #0x10]
100031fa8:     	cmp	w9, #0x1
100031fac:     	b.ne	 <L3>
100031fb0:     	ldr	w9, [x8, #0x1c]
100031fb4:     	cbnz	w9,  <L11>
<L3>:
100031fb8:     	ldr	w9, [x8, #0x18]
100031fbc:     	cmp	w9, #0x2
100031fc0:     	b.ne	 <L12>
100031fc4:     	add	x9, x8, #0x89
100031fc8:     	mov	w10, #0x1               ; =1
100031fcc:     	swpb	w10, w9, [x9]
100031fd0:     	tbnz	w9, #0x0,  <L11>
<L4>:
100031fd4:     	ldr	x21, [x8, #0x60]
<L5>:
100031fd8:     	mov	x0, x20
100031fdc:     	mov	x1, x21
100031fe0:     	bl	 <_scoop_shape_validate_object>
100031fe4:     	ldr	x8, [sp, #0x10]
100031fe8:     	ldr	w9, [x8, #0x18]
100031fec:     	cmp	w9, #0x1
100031ff0:     	b.ne	 <L7>
100031ff4:     	ldr	x10, [sp, #0x8]
100031ff8:     	lsl	x9, x10, #3
100031ffc:     	add	x11, x21, x9
100032000:     	sub	x12, x11, #0x1
100032004:     	ubfx	x11, x10, #10, #51
100032008:     	lsr	x10, x12, #13
10003200c:     	cmp	x10, x11
100032010:     	b.lo	 <L7>
100032014:     	mov	w13, #-0x1              ; =-1
100032018:     	eor	w12, w13, w12, lsr #7
10003201c:     	lsr	x9, x9, #7
100032020:     	mov	x13, #-0x1              ; =-1
100032024:     	lsl	x14, x13, x9
100032028:     	lsr	x9, x13, x12
10003202c:     	subs	x10, x10, x11
100032030:     	csinv	x12, x9, xzr, eq
100032034:     	and	x12, x12, x14
100032038:     	ldr	x13, [x8, #0x48]
10003203c:     	add	x13, x13, x11, lsl #3
100032040:     	ldset	x12, x12, [x13]
100032044:     	b.eq	 <L7>
100032048:     	lsl	x11, x11, #3
10003204c:     	add	x11, x11, #0x8
<L6>:
100032050:     	subs	x10, x10, #0x1
100032054:     	csinv	x12, x9, xzr, eq
100032058:     	ldr	x13, [x8, #0x48]
10003205c:     	add	x13, x13, x11
100032060:     	ldset	x12, x12, [x13]
100032064:     	add	x11, x11, #0x8
100032068:     	cbnz	x10,  <L6>
<L7>:
10003206c:     	ldr	x8, [x19, #0xa0]
100032070:     	ldr	x0, [sp, #0x10]
100032074:     	ldr	x9, [x0, #0x78]
100032078:     	add	x22, x8, x9, lsl #4
10003207c:     	ldr	x8, [x22]
100032080:     	add	x8, x8, x21
100032084:     	str	x8, [x22]
100032088:     	ldr	x1, [sp, #0x8]
10003208c:     	bl	 <_scoop_heap_object_pinned>
100032090:     	tbnz	w0, #0x0,  <L8>
100032094:     	ldr	x8, [x22, #0x8]
100032098:     	add	x8, x8, x21
10003209c:     	str	x8, [x22, #0x8]
<L8>:
1000320a0:     	ldur	q0, [x19, #0xd8]
1000320a4:     	mov	w8, #0x1                ; =1
1000320a8:     	dup.2d	v1, x8
1000320ac:     	add.2d	v0, v0, v1
1000320b0:     	stur	q0, [x19, #0xd8]
1000320b4:     	ldr	x8, [x20]
1000320b8:     	ldr	x21, [x8, #0x48]
1000320bc:     	cbz	x21,  <L13>
1000320c0:     	ldr	x22, [x21]
1000320c4:     	cmn	x22, #0x1
1000320c8:     	cset	w2, ne
1000320cc:     	mov	x0, x19
1000320d0:     	mov	x1, x20
1000320d4:     	bl	 <_append_live>
1000320d8:     	cmn	x22, #0x1
1000320dc:     	b.ne	 <L11>
1000320e0:     	ldr	x8, [x21, #0x8]
1000320e4:     	ldr	x22, [x20, x8]
1000320e8:     	cbz	x22,  <L11>
1000320ec:     	mov	x23, #0x0               ; =0
1000320f0:     	ldr	x8, [x21, #0x10]
1000320f4:     	add	x24, x8, x20
1000320f8:     	add	x8, sp, #0x18
1000320fc:     	orr	x25, x8, #0x1
100032100:     	mov	w26, #0x400             ; =1024
100032104:     	mov	w27, #0x1               ; =1
<L9>:
100032108:     	sub	x8, x22, x23
10003210c:     	cmp	x8, #0x400
100032110:     	csel	x8, x8, x26, lo
100032114:     	stur	wzr, [x25, #0x3]
100032118:     	str	wzr, [x25]
10003211c:     	ldr	x9, [x21, #0x18]
100032120:     	madd	x10, x9, x23, x24
100032124:     	add	x23, x8, x23
100032128:     	strb	w27, [sp, #0x18]
10003212c:     	madd	x8, x9, x23, x24
100032130:     	stp	x20, x21, [sp, #0x20]
100032134:     	stp	x10, x8, [sp, #0x30]
100032138:     	add	x1, sp, #0x18
10003213c:     	mov	x0, x19
100032140:     	bl	 <_scoop_gc_mark_queue_push>
100032144:     	cmp	x23, x22
100032148:     	b.lo	 <L9>
10003214c:     	b	 <L11>
<L10>:
100032150:     	mov	x0, x20
100032154:     	bl	 <_scoop_gc_stw_is_stable_object>
100032158:     	tbz	w0, #0x0,  <L14>
<L11>:
10003215c:     	ldp	x29, x30, [sp, #0x90]
100032160:     	ldp	x20, x19, [sp, #0x80]
100032164:     	ldp	x22, x21, [sp, #0x70]
100032168:     	ldp	x24, x23, [sp, #0x60]
10003216c:     	ldp	x26, x25, [sp, #0x50]
100032170:     	ldp	x28, x27, [sp, #0x40]
100032174:     	add	sp, sp, #0xa0
100032178:     	ret
<L12>:
10003217c:     	ldr	x10, [sp, #0x8]
100032180:     	mov	w9, #0x1                ; =1
100032184:     	lsl	x9, x9, x10
100032188:     	ldr	x8, [x8, #0x28]
10003218c:     	lsr	x10, x10, #6
100032190:     	ldr	x11, [x8, x10, lsl #3]
100032194:     	tst	x11, x9
100032198:     	b.ne	 <L11>
10003219c:     	add	x8, x8, x10, lsl #3
1000321a0:     	ldset	x9, x8, [x8]
1000321a4:     	tst	x8, x9
1000321a8:     	b.ne	 <L11>
1000321ac:     	ldr	x8, [sp, #0x10]
1000321b0:     	ldr	w9, [x8, #0x18]
1000321b4:     	cmp	w9, #0x2
1000321b8:     	b.eq	 <L4>
1000321bc:     	ldr	x8, [x8, #0x50]
1000321c0:     	ldr	x9, [sp, #0x8]
1000321c4:     	ldrh	w8, [x8, x9, lsl #1]
1000321c8:     	lsl	x21, x8, #3
1000321cc:     	b	 <L5>
<L13>:
1000321d0:     	mov	x0, x19
1000321d4:     	mov	x1, x20
1000321d8:     	mov	w2, #0x0                ; =0
1000321dc:     	bl	 <_append_live>
1000321e0:     	b	 <L11>
<L14>:
1000321e4:     	adrp	x0, 0x100064000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x170>
1000321e8:     	add	x0, x0, #0x1ab
1000321ec:     	bl	 <_scoop_heap_fatal>

00000001000321f0 <_scoop_gc_mark_run_task>:
1000321f0:     	sub	sp, sp, #0x60
1000321f4:     	stp	x26, x25, [sp, #0x10]
1000321f8:     	stp	x24, x23, [sp, #0x20]
1000321fc:     	stp	x22, x21, [sp, #0x30]
100032200:     	stp	x20, x19, [sp, #0x40]
100032204:     	stp	x29, x30, [sp, #0x50]
100032208:     	add	x29, sp, #0x50
10003220c:     	mov	x20, x1
100032210:     	mov	x19, x0
100032214:     	str	x0, [sp]
100032218:     	adrp	x8, 0x10005d000 <_scoop$1$bs$3480990329fbe1abf09bfc8ccb1ebe7f4e0c2cef8d0758668762f0b5ce56316e>
10003221c:     	ldr	d0, [x8, #0x440]
100032220:     	str	d0, [sp, #0x8]
100032224:     	ldr	x8, [x0, #0x100]
100032228:     	add	x8, x8, #0x1
10003222c:     	str	x8, [x0, #0x100]
100032230:     	ldrb	w8, [x1]
100032234:     	cmp	w8, #0x1
100032238:     	b.ne	 <L0>
10003223c:     	ldr	x8, [x19, #0x108]
100032240:     	add	x8, x8, #0x1
100032244:     	str	x8, [x19, #0x108]
100032248:     	ldp	x0, x1, [x20, #0x8]
10003224c:     	ldp	x4, x5, [x20, #0x18]
100032250:     	adrp	x2, 0x100031000 <_scoop_eh_decode_lsda+0x32c>
100032254:     	add	x2, x2, #0xf14
100032258:     	mov	x3, sp
10003225c:     	bl	 <_scoop_gc_scan_descriptor>
100032260:     	b	 <L5>
<L0>:
100032264:     	ldr	x22, [x20, #0x10]
100032268:     	cbz	x22,  <L2>
10003226c:     	fmov	d0, x22
100032270:     	cnt.8b	v0, v0
100032274:     	addv.8b	b0, v0
100032278:     	fmov	x23, d0
10003227c:     	adrp	x21, 0x100031000 <_scoop_eh_decode_lsda+0x32c>
100032280:     	add	x21, x21, #0xf14
100032284:     	mov	x24, x23
100032288:     	mov	x25, x22
<L1>:
10003228c:     	rbit	x8, x25
100032290:     	clz	x8, x8
100032294:     	ldr	x9, [x20, #0x8]
100032298:     	ldr	x0, [x9, x8, lsl #3]
10003229c:     	ldr	x8, [x0]
1000322a0:     	ldr	x1, [x8, #0x48]
1000322a4:     	mov	x3, sp
1000322a8:     	mov	x2, x21
1000322ac:     	mov	x4, #0x0                ; =0
1000322b0:     	mov	x5, #-0x1               ; =-1
1000322b4:     	bl	 <_scoop_gc_scan_descriptor>
1000322b8:     	sub	x8, x25, #0x1
1000322bc:     	and	x25, x8, x25
1000322c0:     	subs	x24, x24, #0x1
1000322c4:     	b.ne	 <L1>
1000322c8:     	cmn	x22, #0x1
1000322cc:     	b.ne	 <L3>
1000322d0:     	b	 <L5>
<L2>:
1000322d4:     	mov	x23, #0x0               ; =0
<L3>:
1000322d8:     	sub	x21, x23, #0x40
1000322dc:     	adrp	x20, 0x100031000 <_scoop_eh_decode_lsda+0x32c>
1000322e0:     	add	x20, x20, #0xf14
<L4>:
1000322e4:     	ldr	x8, [x19, #0xd0]
1000322e8:     	cbz	x8,  <L5>
1000322ec:     	rbit	x9, x8
1000322f0:     	clz	x9, x9
1000322f4:     	ldr	x10, [x19, #0xc0]
1000322f8:     	ldr	x0, [x10, x9, lsl #3]
1000322fc:     	sub	x9, x8, #0x1
100032300:     	and	x8, x9, x8
100032304:     	str	x8, [x19, #0xd0]
100032308:     	ldr	x8, [x0]
10003230c:     	ldr	x1, [x8, #0x48]
100032310:     	mov	x3, sp
100032314:     	mov	x2, x20
100032318:     	mov	x4, #0x0                ; =0
10003231c:     	mov	x5, #-0x1               ; =-1
100032320:     	bl	 <_scoop_gc_scan_descriptor>
100032324:     	adds	x21, x21, #0x1
100032328:     	b.lo	 <L4>
<L5>:
10003232c:     	ldp	x29, x30, [sp, #0x50]
100032330:     	ldp	x20, x19, [sp, #0x40]
100032334:     	ldp	x22, x21, [sp, #0x30]
100032338:     	ldp	x24, x23, [sp, #0x20]
10003233c:     	ldp	x26, x25, [sp, #0x10]
100032340:     	add	sp, sp, #0x60
100032344:     	ret

000000010003247c <_scoop_gc_heap_plan_moving_locked>:
10003247c:     	sub	sp, sp, #0x70
100032480:     	stp	x26, x25, [sp, #0x20]
100032484:     	stp	x24, x23, [sp, #0x30]
100032488:     	stp	x22, x21, [sp, #0x40]
10003248c:     	stp	x20, x19, [sp, #0x50]
100032490:     	stp	x29, x30, [sp, #0x60]
100032494:     	add	x29, sp, #0x60
100032498:     	adrp	x21, 0x100160000 <dyld_stub_binder+0x100160000>
10003249c:     	add	x21, x21, #0x468
1000324a0:     	ldrb	w8, [x21, #0x9d]
1000324a4:     	tbz	w8, #0x0,  <L20>
1000324a8:     	mov	x20, x0
1000324ac:     	bl	 <_scoop_heap_select_evacuation_sources>
1000324b0:     	mov	x19, x0
1000324b4:     	ldrb	w8, [x21, #0x99]
1000324b8:     	orr	w20, w20, w8
1000324bc:     	tbnz	w20, #0x0,  <L0>
1000324c0:     	add	x8, x21, #0x268
1000324c4:     	movi.2d	v0, #0000000000000000
1000324c8:     	str	q0, [x8]
1000324cc:     	cbz	x19,  <L15>
1000324d0:     	bl	 <_scoop_heap_prepare_evacuation_targets>
<L0>:
1000324d4:     	mov	w8, #0x1                ; =1
1000324d8:     	bic	w9, w8, w20
1000324dc:     	movi.2d	v0, #0000000000000000
1000324e0:     	stp	q0, q0, [sp]
1000324e4:     	strb	w9, [sp, #0x18]
1000324e8:     	cmp	x19, #0x1
1000324ec:     	cset	w9, hi
1000324f0:     	orr	w9, w20, w9
1000324f4:     	and	w9, w9, #0x1
1000324f8:     	strb	w9, [sp, #0x19]
1000324fc:     	strb	w8, [sp, #0x1a]
100032500:     	adrp	x0, 0x100032000 <_scoop_gc_mark_slot+0xec>
100032504:     	add	x0, x0, #0x72c
100032508:     	mov	x1, sp
10003250c:     	bl	 <_scoop_gc_mark_visit_live>
100032510:     	tbz	w20, #0x0,  <L1>
100032514:     	ldrb	w8, [sp, #0x1a]
100032518:     	tbnz	w8, #0x0,  <L6>
10003251c:     	b	 <L10>
<L1>:
100032520:     	ldr	x10, [sp, #0x8]
100032524:     	cbz	x10,  <L4>
100032528:     	mov	x8, #0x0                ; =0
10003252c:     	mov	x9, #0x0                ; =0
100032530:     	ldr	x11, [sp]
100032534:     	add	x11, x11, #0x28
100032538:     	mov	w12, #0x1               ; =1
10003253c:     	b	 <L3>
<L2>:
100032540:     	subs	x10, x10, #0x1
100032544:     	b.eq	 <L5>
<L3>:
100032548:     	ldr	x13, [x11], #0x30
10003254c:     	ldr	x13, [x13]
100032550:     	ldrb	w14, [x13, #0x2c]
100032554:     	tbnz	w14, #0x0,  <L2>
100032558:     	strb	w12, [x13, #0x2c]
10003255c:     	add	x8, x8, #0x1
100032560:     	ldr	x13, [x13, #0x30]
100032564:     	cmp	x13, #0x0
100032568:     	cinc	x9, x9, eq
10003256c:     	b	 <L2>
<L4>:
100032570:     	mov	x9, #0x0                ; =0
100032574:     	mov	x8, #0x0                ; =0
<L5>:
100032578:     	cmp	x9, x19
10003257c:     	ldrb	w9, [sp, #0x1a]
100032580:     	csel	w9, wzr, w9, hs
100032584:     	strb	w9, [sp, #0x1a]
100032588:     	tbz	w9, #0x0,  <L10>
10003258c:     	str	x19, [x21, #0x268]
100032590:     	str	x8, [x21, #0x270]
<L6>:
100032594:     	bl	 <_scoop_gc_monotonic_ns>
100032598:     	mov	x19, x0
10003259c:     	ldr	x8, [sp, #0x8]
1000325a0:     	cbz	x8,  <L14>
1000325a4:     	mov	x20, #0x0               ; =0
1000325a8:     	mov	x22, #0x0               ; =0
1000325ac:     	mov	w23, #0x7f80            ; =32640
1000325b0:     	b	 <L9>
<L7>:
1000325b4:     	mov	w1, #0x1                ; =1
1000325b8:     	bl	 <_scoop_heap_publish_large_object>
1000325bc:     	ldr	x8, [x24, #0x8]
1000325c0:     	str	x8, [x25, #0x80]
<L8>:
1000325c4:     	ldr	x8, [x21, #0x300]
1000325c8:     	add	x8, x8, #0x1
1000325cc:     	str	x8, [x21, #0x300]
1000325d0:     	add	x22, x22, #0x1
1000325d4:     	ldr	x8, [sp, #0x8]
1000325d8:     	add	x20, x20, #0x30
1000325dc:     	cmp	x22, x8
1000325e0:     	b.hs	 <L14>
<L9>:
1000325e4:     	ldr	x8, [sp]
1000325e8:     	add	x24, x8, x20
1000325ec:     	ldp	x1, x0, [x24]
1000325f0:     	ldr	x2, [x24, #0x10]
1000325f4:     	bl	 <dyld_stub_binder+0x10004a824>
1000325f8:     	ldr	x2, [x24, #0x10]
1000325fc:     	ldr	x8, [x21, #0x280]
100032600:     	add	x8, x8, x2
100032604:     	str	x8, [x21, #0x280]
100032608:     	ldp	x25, x0, [x24, #0x20]
10003260c:     	cmp	x2, x23
100032610:     	b.hi	 <L7>
100032614:     	ldr	x1, [x24, #0x8]
100032618:     	mov	w3, #0x1                ; =1
10003261c:     	bl	 <_scoop_heap_record_small_object>
100032620:     	ldr	x8, [x24, #0x8]
100032624:     	ldr	x9, [x25, #0x58]
100032628:     	ldr	x10, [x24, #0x18]
10003262c:     	str	x8, [x9, x10, lsl #3]
100032630:     	b	 <L8>
<L10>:
100032634:     	ldr	x0, [sp]
100032638:     	bl	 <dyld_stub_binder+0x10004a7ac>
10003263c:     	bl	 <_scoop_heap_first_block>
100032640:     	cbz	x0,  <L16>
100032644:     	mov	w20, #0x2               ; =2
100032648:     	mov	w22, #0x5               ; =5
10003264c:     	b	 <L13>
<L11>:
100032650:     	mov	x19, x0
100032654:     	bl	 <_scoop_heap_block_has_pins>
100032658:     	cmp	w0, #0x0
10003265c:     	csel	w8, w22, w20, ne
100032660:     	str	w8, [x19, #0x14]
100032664:     	ldr	x0, [x19, #0x58]
100032668:     	bl	 <dyld_stub_binder+0x10004a7ac>
10003266c:     	mov	x0, x19
100032670:     	str	xzr, [x19, #0x58]
<L12>:
100032674:     	bl	 <_scoop_heap_next_block>
100032678:     	cbz	x0,  <L16>
<L13>:
10003267c:     	ldr	w8, [x0, #0x14]
100032680:     	cmp	w8, #0x3
100032684:     	b.eq	 <L11>
100032688:     	cmp	w8, #0x4
10003268c:     	b.ne	 <L12>
100032690:     	mov	x19, x0
100032694:     	bl	 <_scoop_heap_release_block>
100032698:     	mov	x0, x19
10003269c:     	b	 <L12>
<L14>:
1000326a0:     	bl	 <_scoop_gc_monotonic_ns>
1000326a4:     	ldr	x8, [x21, #0x180]
1000326a8:     	sub	x9, x0, x19
1000326ac:     	add	x8, x9, x8
1000326b0:     	str	x8, [x21, #0x180]
1000326b4:     	ldr	x0, [sp]
1000326b8:     	bl	 <dyld_stub_binder+0x10004a7ac>
<L15>:
1000326bc:     	mov	w0, #0x1                ; =1
1000326c0:     	b	 <L19>
<L16>:
1000326c4:     	ldr	x8, [x21, #0x40]
1000326c8:     	cbz	x8,  <L18>
<L17>:
1000326cc:     	sturh	wzr, [x8, #0x2b]
1000326d0:     	ldr	x8, [x8, #0x18]
1000326d4:     	cbnz	x8,  <L17>
<L18>:
1000326d8:     	str	xzr, [x21, #0x2f8]
1000326dc:     	add	x8, x21, #0x2e8
1000326e0:     	movi.2d	v0, #0000000000000000
1000326e4:     	str	q0, [x8]
1000326e8:     	ldrb	w8, [x21, #0x99]
1000326ec:     	cmp	w8, #0x1
1000326f0:     	b.eq	 <L21>
1000326f4:     	mov	w0, #0x0                ; =0
<L19>:
1000326f8:     	ldp	x29, x30, [sp, #0x60]
1000326fc:     	ldp	x20, x19, [sp, #0x50]
100032700:     	ldp	x22, x21, [sp, #0x40]
100032704:     	ldp	x24, x23, [sp, #0x30]
100032708:     	ldp	x26, x25, [sp, #0x20]
10003270c:     	add	sp, sp, #0x70
100032710:     	ret
<L20>:
100032714:     	adrp	x0, 0x100064000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x170>
100032718:     	add	x0, x0, #0x21f
10003271c:     	bl	 <_scoop_heap_fatal>
<L21>:
100032720:     	adrp	x0, 0x100064000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x170>
100032724:     	add	x0, x0, #0x245
100032728:     	bl	 <_scoop_heap_fatal>

0000000100041bd8 <_scoop_gc_mark_pool_prepare>:
100041bd8:     	sub	sp, sp, #0x60
100041bdc:     	stp	x26, x25, [sp, #0x10]
100041be0:     	stp	x24, x23, [sp, #0x20]
100041be4:     	stp	x22, x21, [sp, #0x30]
100041be8:     	stp	x20, x19, [sp, #0x40]
100041bec:     	stp	x29, x30, [sp, #0x50]
100041bf0:     	add	x29, sp, #0x50
100041bf4:     	mov	x19, x0
100041bf8:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
100041bfc:     	ldrb	w8, [x8, #0x900]
100041c00:     	cmp	w8, #0x1
100041c04:     	b.ne	 <L0>
100041c08:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
100041c0c:     	ldr	x20, [x8, #0x8d8]
100041c10:     	b	 <L2>
<L0>:
100041c14:     	bl	 <_scoop_platform_bundle>
100041c18:     	ldr	x8, [x0, #0x8]
100041c1c:     	ldr	x8, [x8, #0x18]
100041c20:     	blr	x8
100041c24:     	mov	w8, #0x4                ; =4
100041c28:     	cmp	x0, #0x4
100041c2c:     	csel	x20, x0, x8, lo
100041c30:     	adrp	x0, 0x100067000 <_check_release_hook.domain+0x460>
100041c34:     	add	x0, x0, #0x8f3
100041c38:     	bl	 <dyld_stub_binder+0x10004a7c4>
100041c3c:     	cbz	x0,  <L1>
100041c40:     	str	xzr, [sp, #0x8]
100041c44:     	add	x1, sp, #0x8
100041c48:     	mov	x21, x0
100041c4c:     	mov	w2, #0xa                ; =10
100041c50:     	bl	 <dyld_stub_binder+0x10004aa64>
100041c54:     	ldr	x8, [sp, #0x8]
100041c58:     	cmp	x21, x8
100041c5c:     	b.eq	 <L10>
100041c60:     	mov	x20, x0
100041c64:     	ldrb	w8, [x8]
100041c68:     	sub	x9, x0, #0x9
100041c6c:     	cmp	w8, #0x0
100041c70:     	ccmn	x9, #0x9, #0x0, eq
100041c74:     	b.ls	 <L10>
100041c78:     	mov	w8, #0x1                ; =1
100041c7c:     	adrp	x9, 0x100160000 <dyld_stub_binder+0x100160000>
100041c80:     	strb	w8, [x9, #0x901]
<L1>:
100041c84:     	adrp	x0, 0x100160000 <dyld_stub_binder+0x100160000>
100041c88:     	add	x0, x0, #0x980
100041c8c:     	str	xzr, [x0, #0x80]
100041c90:     	mov	x1, #0x0                ; =0
100041c94:     	bl	 <dyld_stub_binder+0x10004a8f0>
100041c98:     	cbnz	w0,  <L9>
100041c9c:     	adrp	x0, 0x100160000 <dyld_stub_binder+0x100160000>
100041ca0:     	add	x0, x0, #0xb00
100041ca4:     	mov	w8, #0x1                ; =1
100041ca8:     	str	x8, [x0, #0x80]
100041cac:     	mov	x1, #0x0                ; =0
100041cb0:     	bl	 <dyld_stub_binder+0x10004a8f0>
100041cb4:     	cbnz	w0,  <L9>
100041cb8:     	adrp	x0, 0x100160000 <dyld_stub_binder+0x100160000>
100041cbc:     	add	x0, x0, #0xc80
100041cc0:     	mov	w8, #0x2                ; =2
100041cc4:     	str	x8, [x0, #0x80]
100041cc8:     	mov	x1, #0x0                ; =0
100041ccc:     	bl	 <dyld_stub_binder+0x10004a8f0>
100041cd0:     	cbnz	w0,  <L9>
100041cd4:     	adrp	x0, 0x100160000 <dyld_stub_binder+0x100160000>
100041cd8:     	add	x0, x0, #0xe00
100041cdc:     	mov	w8, #0x3                ; =3
100041ce0:     	str	x8, [x0, #0x80]
100041ce4:     	mov	x1, #0x0                ; =0
100041ce8:     	bl	 <dyld_stub_binder+0x10004a8f0>
100041cec:     	cbnz	w0,  <L9>
100041cf0:     	adrp	x0, 0x100160000 <dyld_stub_binder+0x100160000>
100041cf4:     	add	x0, x0, #0xf80
100041cf8:     	mov	w8, #0x4                ; =4
100041cfc:     	str	x8, [x0, #0x80]
100041d00:     	mov	x1, #0x0                ; =0
100041d04:     	bl	 <dyld_stub_binder+0x10004a8f0>
100041d08:     	cbnz	w0,  <L9>
100041d0c:     	adrp	x0, 0x100161000 <_scoop_gc_marker+0x800>
100041d10:     	add	x0, x0, #0x100
100041d14:     	mov	w8, #0x5                ; =5
100041d18:     	str	x8, [x0, #0x80]
100041d1c:     	mov	x1, #0x0                ; =0
100041d20:     	bl	 <dyld_stub_binder+0x10004a8f0>
100041d24:     	cbnz	w0,  <L9>
100041d28:     	adrp	x0, 0x100161000 <_scoop_gc_marker+0x800>
100041d2c:     	add	x0, x0, #0x280
100041d30:     	mov	w8, #0x6                ; =6
100041d34:     	str	x8, [x0, #0x80]
100041d38:     	mov	x1, #0x0                ; =0
100041d3c:     	bl	 <dyld_stub_binder+0x10004a8f0>
100041d40:     	cbnz	w0,  <L9>
100041d44:     	adrp	x0, 0x100161000 <_scoop_gc_marker+0x800>
100041d48:     	add	x0, x0, #0x400
100041d4c:     	mov	w8, #0x7                ; =7
100041d50:     	str	x8, [x0, #0x80]
100041d54:     	mov	x1, #0x0                ; =0
100041d58:     	bl	 <dyld_stub_binder+0x10004a8f0>
100041d5c:     	cbnz	w0,  <L9>
100041d60:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
100041d64:     	add	x8, x8, #0x8d8
100041d68:     	str	x20, [x8]
100041d6c:     	mov	w9, #0x1                ; =1
100041d70:     	strb	w9, [x8, #0x28]
<L2>:
100041d74:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
100041d78:     	add	x8, x8, #0x8e8
100041d7c:     	ldrb	w9, [x8, #0x19]
100041d80:     	adrp	x22, 0x100160000 <dyld_stub_binder+0x100160000>
100041d84:     	add	x22, x22, #0x468
100041d88:     	ldr	x10, [x22, #0x60]
100041d8c:     	cmp	x10, #0x400, lsl #12    ; =0x400000
100041d90:     	csinc	w10, w19, wzr, hs
100041d94:     	cmp	w10, #0x0
100041d98:     	csinc	x10, x20, xzr, eq
100041d9c:     	cmp	w9, #0x0
100041da0:     	csel	x24, x20, x10, ne
100041da4:     	ldr	x8, [x8]
100041da8:     	cmp	x24, #0x2
100041dac:     	ccmp	x8, #0x0, #0x0, hs
100041db0:     	adrp	x19, 0x100160000 <dyld_stub_binder+0x100160000>
100041db4:     	add	x19, x19, #0x800
100041db8:     	b.ne	 <L7>
100041dbc:     	sub	x25, x24, #0x1
100041dc0:     	adrp	x20, 0x100160000 <dyld_stub_binder+0x100160000>
100041dc4:     	add	x20, x20, #0xb00
100041dc8:     	adrp	x26, 0x100160000 <dyld_stub_binder+0x100160000>
100041dcc:     	adrp	x23, 0x100160000 <dyld_stub_binder+0x100160000>
100041dd0:     	adrp	x21, 0x100041000 <_scoop_rt_thread_debug_mode+0x14>
100041dd4:     	add	x21, x21, #0xee4
<L3>:
100041dd8:     	ldr	x8, [x26, #0x8f8]
100041ddc:     	str	x8, [x20, #0x90]
100041de0:     	add	x0, x20, #0x88
100041de4:     	mov	x1, #0x0                ; =0
100041de8:     	mov	x2, x21
100041dec:     	mov	x3, x20
100041df0:     	bl	 <dyld_stub_binder+0x10004a8b4>
100041df4:     	cbnz	w0,  <L4>
100041df8:     	ldr	x8, [x23, #0x8e8]
100041dfc:     	add	x8, x8, #0x1
100041e00:     	str	x8, [x23, #0x8e8]
100041e04:     	add	x20, x20, #0x180
100041e08:     	subs	x25, x25, #0x1
100041e0c:     	b.ne	 <L3>
100041e10:     	b	 <L7>
<L4>:
100041e14:     	mov	x0, x19
100041e18:     	bl	 <dyld_stub_binder+0x10004a8fc>
100041e1c:     	mov	w8, #0x1                ; =1
100041e20:     	strb	w8, [x19, #0x102]
100041e24:     	add	x0, x19, #0x40
100041e28:     	bl	 <dyld_stub_binder+0x10004a884>
100041e2c:     	mov	x0, x19
100041e30:     	bl	 <dyld_stub_binder+0x10004a908>
100041e34:     	ldr	x8, [x19, #0xe8]
100041e38:     	cbz	x8,  <L6>
100041e3c:     	adrp	x20, 0x100160000 <dyld_stub_binder+0x100160000>
100041e40:     	add	x20, x20, #0xb88
100041e44:     	mov	w21, #0x1               ; =1
<L5>:
100041e48:     	ldr	x0, [x20]
100041e4c:     	mov	x1, #0x0                ; =0
100041e50:     	bl	 <dyld_stub_binder+0x10004a8d8>
100041e54:     	cbnz	w0,  <L8>
100041e58:     	add	x20, x20, #0x180
100041e5c:     	add	x21, x21, #0x1
100041e60:     	ldr	x8, [x23, #0x8e8]
100041e64:     	cmp	x21, x8
100041e68:     	b.ls	 <L5>
100041e6c:     	str	xzr, [x23, #0x8e8]
<L6>:
100041e70:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
100041e74:     	add	x8, x8, #0x8d8
100041e78:     	strb	wzr, [x8, #0x2a]
100041e7c:     	mov	w24, #0x1               ; =1
100041e80:     	str	x24, [x8]
100041e84:     	ldr	x8, [x22, #0x1b0]
100041e88:     	add	x8, x8, #0x1
100041e8c:     	str	x8, [x22, #0x1b0]
<L7>:
100041e90:     	mov	x0, x19
100041e94:     	bl	 <dyld_stub_binder+0x10004a8fc>
100041e98:     	str	x24, [x19, #0xe0]
100041e9c:     	mov	x0, x19
100041ea0:     	bl	 <dyld_stub_binder+0x10004a908>
100041ea4:     	ldp	x29, x30, [sp, #0x50]
100041ea8:     	ldp	x20, x19, [sp, #0x40]
100041eac:     	ldp	x22, x21, [sp, #0x30]
100041eb0:     	ldp	x24, x23, [sp, #0x20]
100041eb4:     	ldp	x26, x25, [sp, #0x10]
100041eb8:     	add	sp, sp, #0x60
100041ebc:     	ret
<L8>:
100041ec0:     	adrp	x0, 0x100067000 <_check_release_hook.domain+0x460>
100041ec4:     	add	x0, x0, #0x956
100041ec8:     	bl	 <_scoop_heap_fatal>
<L9>:
100041ecc:     	adrp	x0, 0x100067000 <_check_release_hook.domain+0x460>
100041ed0:     	add	x0, x0, #0x934
100041ed4:     	bl	 <_scoop_heap_fatal>
<L10>:
100041ed8:     	adrp	x0, 0x100067000 <_check_release_hook.domain+0x460>
100041edc:     	add	x0, x0, #0x904
100041ee0:     	bl	 <_scoop_heap_fatal>

0000000100042048 <_run_worker>:
100042048:     	sub	sp, sp, #0x80
10004204c:     	stp	x22, x21, [sp, #0x50]
100042050:     	stp	x20, x19, [sp, #0x60]
100042054:     	stp	x29, x30, [sp, #0x70]
100042058:     	add	x29, sp, #0x70
10004205c:     	mov	x19, x0
100042060:     	bl	 <_scoop_gc_thread_cpu_ns>
100042064:     	mov	x20, x0
100042068:     	adrp	x21, 0x100160000 <dyld_stub_binder+0x100160000>
10004206c:     	add	x21, x21, #0x800
100042070:     	b	 <L2>
<L0>:
100042074:     	mov	x0, x21
100042078:     	bl	 <dyld_stub_binder+0x10004a908>
<L1>:
10004207c:     	ldur	q0, [sp, #0x28]
100042080:     	ldur	q1, [sp, #0x38]
100042084:     	stp	q0, q1, [sp]
100042088:     	ldr	x8, [sp, #0x48]
10004208c:     	str	x8, [sp, #0x20]
100042090:     	mov	x1, sp
100042094:     	mov	x0, x19
100042098:     	bl	 <_scoop_gc_mark_run_task>
10004209c:     	mov	x0, x19
1000420a0:     	bl	 <_scoop_gc_mark_flush>
1000420a4:     	bl	 <_scoop_gc_mark_task_done>
<L2>:
1000420a8:     	add	x1, sp, #0x28
1000420ac:     	mov	x0, x19
1000420b0:     	bl	 <_scoop_gc_mark_queue_take>
1000420b4:     	tbnz	w0, #0x0,  <L1>
1000420b8:     	mov	x0, x21
1000420bc:     	bl	 <dyld_stub_binder+0x10004a8fc>
<L3>:
1000420c0:     	add	x8, x21, #0xd0
1000420c4:     	ldapr	x8, [x8]
1000420c8:     	cbz	x8,  <L4>
1000420cc:     	add	x1, sp, #0x28
1000420d0:     	mov	x0, x19
1000420d4:     	bl	 <_scoop_gc_mark_queue_take>
1000420d8:     	tbnz	w0, #0x0,  <L0>
1000420dc:     	add	x0, x21, #0x70
1000420e0:     	mov	x1, x21
1000420e4:     	bl	 <dyld_stub_binder+0x10004a8a8>
1000420e8:     	b	 <L3>
<L4>:
1000420ec:     	adrp	x0, 0x100160000 <dyld_stub_binder+0x100160000>
1000420f0:     	add	x0, x0, #0x800
1000420f4:     	bl	 <dyld_stub_binder+0x10004a908>
1000420f8:     	bl	 <_scoop_gc_thread_cpu_ns>
1000420fc:     	ldr	x8, [x19, #0x118]
100042100:     	sub	x9, x0, x20
100042104:     	add	x8, x9, x8
100042108:     	str	x8, [x19, #0x118]
10004210c:     	ldp	x29, x30, [sp, #0x70]
100042110:     	ldp	x20, x19, [sp, #0x60]
100042114:     	ldp	x22, x21, [sp, #0x50]
100042118:     	add	sp, sp, #0x80
10004211c:     	ret

0000000100043e5c <_collect>:
100043e5c:     	sub	sp, sp, #0x90
100043e60:     	stp	x28, x27, [sp, #0x30]
100043e64:     	stp	x26, x25, [sp, #0x40]
100043e68:     	stp	x24, x23, [sp, #0x50]
100043e6c:     	stp	x22, x21, [sp, #0x60]
100043e70:     	stp	x20, x19, [sp, #0x70]
100043e74:     	stp	x29, x30, [sp, #0x80]
100043e78:     	add	x29, sp, #0x80
100043e7c:     	mov	x20, x0
100043e80:     	bl	 <_scoop_gc_monotonic_ns>
100043e84:     	mov	x21, x0
100043e88:     	bl	 <_scoop_thread_begin_collection>
100043e8c:     	cbz	w0,  <L18>
100043e90:     	mov	x24, x0
100043e94:     	bl	 <_scoop_gc_monotonic_ns>
100043e98:     	mov	x19, x0
100043e9c:     	bl	 <_scoop_gc_heap_lock>
100043ea0:     	bl	 <_scoop_gc_roots_lock>
100043ea4:     	adrp	x25, 0x100160000 <dyld_stub_binder+0x100160000>
100043ea8:     	add	x25, x25, #0x468
100043eac:     	ldr	x8, [x25, #0x158]
100043eb0:     	sub	x9, x19, x21
100043eb4:     	add	x8, x9, x8
100043eb8:     	str	x8, [x25, #0x158]
100043ebc:     	mov	w0, #0x1                ; =1
100043ec0:     	bl	 <_scoop_gc_set_pin_frames_locked>
100043ec4:     	bl	 <_scoop_thread_collection_registry_head>
100043ec8:     	cbz	x0,  <L1>
<L0>:
100043ecc:     	stp	xzr, xzr, [x0, #0xc0]
100043ed0:     	str	xzr, [x0, #0xd0]
100043ed4:     	ldr	x0, [x0, #0xe0]
100043ed8:     	cbnz	x0,  <L0>
<L1>:
100043edc:     	mov	x0, x20
100043ee0:     	bl	 <_scoop_gc_heap_begin_collection_locked>
100043ee4:     	mov	x1, x0
100043ee8:     	mov	x0, x20
100043eec:     	bl	 <_scoop_gc_mark_begin>
100043ef0:     	bl	 <_scoop_gc_monotonic_ns>
100043ef4:     	mov	x21, x0
100043ef8:     	bl	 <_scoop_gc_mark_roots>
100043efc:     	bl	 <_scoop_gc_monotonic_ns>
100043f00:     	ldr	x8, [x25, #0x160]
100043f04:     	sub	x9, x0, x21
100043f08:     	add	x8, x9, x8
100043f0c:     	str	x8, [x25, #0x160]
100043f10:     	cbz	w20,  <L2>
100043f14:     	bl	 <_scoop_gc_monotonic_ns>
100043f18:     	mov	x21, x0
100043f1c:     	bl	 <_scoop_gc_mark_remembered>
100043f20:     	bl	 <_scoop_gc_monotonic_ns>
100043f24:     	ldr	x8, [x25, #0x168]
100043f28:     	sub	x9, x0, x21
100043f2c:     	add	x8, x9, x8
100043f30:     	str	x8, [x25, #0x168]
<L2>:
100043f34:     	bl	 <_scoop_gc_monotonic_ns>
100043f38:     	mov	x22, x0
100043f3c:     	mov	x0, x20
100043f40:     	bl	 <_scoop_gc_mark_finish>
100043f44:     	mov	x21, x0
100043f48:     	bl	 <_scoop_gc_monotonic_ns>
100043f4c:     	ldr	x8, [x25, #0x170]
100043f50:     	sub	x9, x0, x22
100043f54:     	add	x8, x9, x8
100043f58:     	str	x8, [x25, #0x170]
100043f5c:     	bl	 <_scoop_gc_monotonic_ns>
100043f60:     	mov	x23, x0
100043f64:     	ldr	x26, [x25, #0x180]
100043f68:     	mov	x0, x20
100043f6c:     	bl	 <_scoop_gc_heap_plan_moving_locked>
100043f70:     	mov	x22, x0
100043f74:     	bl	 <_scoop_gc_monotonic_ns>
100043f78:     	ldp	x9, x8, [x25, #0x178]
100043f7c:     	add	x10, x26, x0
100043f80:     	add	x8, x23, x8
100043f84:     	sub	x8, x10, x8
100043f88:     	add	x8, x8, x9
100043f8c:     	str	x8, [x25, #0x178]
100043f90:     	cbz	w20,  <L4>
100043f94:     	cbnz	w22,  <L4>
100043f98:     	bl	 <_scoop_gc_mark_dispose>
100043f9c:     	ldr	x8, [x25, #0xb8]
100043fa0:     	add	x8, x8, #0x1
100043fa4:     	str	x8, [x25, #0xb8]
100043fa8:     	strb	wzr, [x25, #0x9d]
100043fac:     	mov	w0, #0x0                ; =0
100043fb0:     	bl	 <_scoop_gc_heap_begin_collection_locked>
100043fb4:     	mov	x1, x0
100043fb8:     	mov	w0, #0x0                ; =0
100043fbc:     	bl	 <_scoop_gc_mark_begin>
100043fc0:     	bl	 <_scoop_gc_monotonic_ns>
100043fc4:     	mov	x20, x0
100043fc8:     	bl	 <_scoop_gc_mark_roots>
100043fcc:     	bl	 <_scoop_gc_monotonic_ns>
100043fd0:     	ldr	x8, [x25, #0x160]
100043fd4:     	sub	x9, x0, x20
100043fd8:     	add	x8, x9, x8
100043fdc:     	str	x8, [x25, #0x160]
100043fe0:     	bl	 <_scoop_gc_monotonic_ns>
100043fe4:     	mov	x20, x0
100043fe8:     	mov	w0, #0x0                ; =0
100043fec:     	bl	 <_scoop_gc_mark_finish>
100043ff0:     	mov	x21, x0
100043ff4:     	bl	 <_scoop_gc_monotonic_ns>
100043ff8:     	ldr	x8, [x25, #0x170]
100043ffc:     	sub	x9, x0, x20
<L3>:
100044000:     	add	x8, x9, x8
100044004:     	str	x8, [x25, #0x170]
100044008:     	bl	 <_scoop_gc_monotonic_ns>
10004400c:     	mov	x22, x0
100044010:     	ldr	x23, [x25, #0x180]
100044014:     	mov	w0, #0x0                ; =0
100044018:     	bl	 <_scoop_gc_heap_plan_moving_locked>
10004401c:     	bl	 <_scoop_gc_monotonic_ns>
100044020:     	mov	w20, #0x0               ; =0
100044024:     	ldp	x9, x8, [x25, #0x178]
100044028:     	add	x10, x23, x0
10004402c:     	add	x8, x22, x8
100044030:     	sub	x8, x10, x8
100044034:     	add	x8, x8, x9
100044038:     	str	x8, [x25, #0x178]
<L4>:
10004403c:     	bl	 <_scoop_gc_monotonic_ns>
100044040:     	mov	x22, x0
100044044:     	strb	w20, [sp, #0xe]
100044048:     	strb	wzr, [sp, #0xf]
10004404c:     	adrp	x23, 0x100044000 <L3>
100044050:     	add	x23, x23, #0x408
100044054:     	adrp	x26, 0x100044000 <L3>
100044058:     	add	x26, x26, #0x4c8
10004405c:     	stp	x23, x26, [sp, #0x10]
100044060:     	adrp	x27, 0x100044000 <L3>
100044064:     	add	x27, x27, #0x4f8
100044068:     	add	x8, sp, #0xe
10004406c:     	stp	x27, x8, [sp, #0x20]
100044070:     	add	x0, sp, #0x10
100044074:     	bl	 <_scoop_gc_scan_roots>
100044078:     	cbz	w20,  <L5>
10004407c:     	adrp	x0, 0x100044000 <L3>
100044080:     	add	x0, x0, #0x328
100044084:     	add	x1, sp, #0xe
100044088:     	mov	w2, #0x0                ; =0
10004408c:     	bl	 <_scoop_gc_scan_remembered>
<L5>:
100044090:     	adrp	x0, 0x100044000 <L3>
100044094:     	add	x0, x0, #0x354
100044098:     	add	x1, sp, #0xe
10004409c:     	bl	 <_scoop_gc_mark_visit_live>
1000440a0:     	bl	 <_scoop_gc_monotonic_ns>
1000440a4:     	ldr	x8, [x25, #0x188]
1000440a8:     	sub	x9, x0, x22
1000440ac:     	add	x8, x9, x8
1000440b0:     	str	x8, [x25, #0x188]
1000440b4:     	ldp	x8, x9, [x25, #0xa8]
1000440b8:     	add	x8, x8, w20, uxtw
1000440bc:     	eor	w10, w20, #0x1
1000440c0:     	add	x9, x9, x10
1000440c4:     	stp	x8, x9, [x25, #0xa8]
1000440c8:     	bl	 <_scoop_gc_stress_move_enabled>
1000440cc:     	cbz	w0,  <L6>
1000440d0:     	strb	w20, [sp, #0xc]
1000440d4:     	mov	w8, #0x1                ; =1
1000440d8:     	strb	w8, [sp, #0xd]
1000440dc:     	stp	x23, x26, [sp, #0x10]
1000440e0:     	add	x8, sp, #0xc
1000440e4:     	stp	x27, x8, [sp, #0x20]
1000440e8:     	add	x0, sp, #0x10
1000440ec:     	bl	 <_scoop_gc_scan_roots>
1000440f0:     	adrp	x0, 0x100044000 <L3>
1000440f4:     	add	x0, x0, #0x3b0
1000440f8:     	add	x1, sp, #0xc
1000440fc:     	bl	 <_scoop_gc_visit_current_objects_locked>
<L6>:
100044100:     	bl	 <_scoop_gc_stress_move_enabled>
100044104:     	cbz	w0,  <L7>
100044108:     	bl	 <_scoop_gc_heap_verify_stress_moved_locked>
<L7>:
10004410c:     	bl	 <_scoop_gc_mark_dispose>
100044110:     	bl	 <_scoop_gc_monotonic_ns>
100044114:     	mov	x22, x0
100044118:     	ldr	x23, [x25, #0x198]
10004411c:     	mov	x0, x21
100044120:     	mov	x1, x20
100044124:     	bl	 <_scoop_gc_heap_finish_collection_locked>
100044128:     	bl	 <_scoop_gc_monotonic_ns>
10004412c:     	ldp	x9, x8, [x25, #0x190]
100044130:     	add	x10, x23, x0
100044134:     	add	x8, x22, x8
100044138:     	sub	x8, x10, x8
10004413c:     	add	x8, x8, x9
100044140:     	str	x8, [x25, #0x190]
100044144:     	mov	w0, #0x0                ; =0
100044148:     	bl	 <_scoop_gc_set_pin_frames_locked>
10004414c:     	bl	 <_scoop_gc_monotonic_ns>
100044150:     	ldp	x9, x10, [x25, #0xf8]
100044154:     	sub	x8, x0, x19
100044158:     	add	x9, x9, x8
10004415c:     	str	x9, [x25, #0xf8]
100044160:     	cmp	x8, x10
100044164:     	b.ls	 <L8>
100044168:     	str	x8, [x25, #0x100]
<L8>:
10004416c:     	cbz	w20,  <L9>
100044170:     	ldr	x9, [x25, #0x288]
100044174:     	add	x9, x9, x8
100044178:     	str	x9, [x25, #0x288]
10004417c:     	b	 <L10>
<L9>:
100044180:     	ldr	x9, [x25, #0x290]
100044184:     	add	x9, x9, x8
100044188:     	str	x9, [x25, #0x290]
<L10>:
10004418c:     	mov	w9, #0x2711             ; =10001
100044190:     	cmp	x8, x9
100044194:     	b.lo	 <L11>
100044198:     	mov	w9, #0xc351             ; =50001
10004419c:     	cmp	x8, x9
1000441a0:     	b.lo	 <L12>
1000441a4:     	mov	w9, #0x86a1             ; =34465
1000441a8:     	movk	w9, #0x1, lsl #16
1000441ac:     	cmp	x8, x9
1000441b0:     	b.lo	 <L13>
1000441b4:     	mov	w9, #0xa121             ; =41249
1000441b8:     	movk	w9, #0x7, lsl #16
1000441bc:     	cmp	x8, x9
1000441c0:     	b.lo	 <L14>
1000441c4:     	mov	w9, #0x4241             ; =16961
1000441c8:     	movk	w9, #0xf, lsl #16
1000441cc:     	cmp	x8, x9
1000441d0:     	b.lo	 <L15>
1000441d4:     	mov	w9, #0x4b41             ; =19265
1000441d8:     	movk	w9, #0x4c, lsl #16
1000441dc:     	cmp	x8, x9
1000441e0:     	b.lo	 <L16>
1000441e4:     	mov	w9, #0x9680             ; =38528
1000441e8:     	movk	w9, #0x98, lsl #16
1000441ec:     	cmp	x8, x9
1000441f0:     	mov	w8, #0x6                ; =6
1000441f4:     	cinc	x8, x8, hi
1000441f8:     	b	 <L17>
<L11>:
1000441fc:     	mov	x8, #0x0                ; =0
100044200:     	b	 <L17>
<L12>:
100044204:     	mov	w8, #0x1                ; =1
100044208:     	b	 <L17>
<L13>:
10004420c:     	mov	w8, #0x2                ; =2
100044210:     	b	 <L17>
<L14>:
100044214:     	mov	w8, #0x3                ; =3
100044218:     	b	 <L17>
<L15>:
10004421c:     	mov	w8, #0x4                ; =4
100044220:     	b	 <L17>
<L16>:
100044224:     	mov	w8, #0x5                ; =5
<L17>:
100044228:     	add	x8, x25, x8, lsl #3
10004422c:     	ldr	x9, [x8, #0x298]
100044230:     	add	x9, x9, #0x1
100044234:     	str	x9, [x8, #0x298]
100044238:     	bl	 <_scoop_gc_roots_unlock>
10004423c:     	bl	 <_scoop_gc_heap_unlock>
100044240:     	bl	 <_scoop_thread_end_collection>
100044244:     	mov	x0, x24
<L18>:
100044248:     	ldp	x29, x30, [sp, #0x80]
10004424c:     	ldp	x20, x19, [sp, #0x70]
100044250:     	ldp	x22, x21, [sp, #0x60]
100044254:     	ldp	x24, x23, [sp, #0x50]
100044258:     	ldp	x26, x25, [sp, #0x40]
10004425c:     	ldp	x28, x27, [sp, #0x30]
100044260:     	add	sp, sp, #0x90
100044264:     	ret

00000001000468c0 <_scoop_gc_mark_visit_live>:
1000468c0:     	stp	x26, x25, [sp, #-0x50]!
1000468c4:     	stp	x24, x23, [sp, #0x10]
1000468c8:     	stp	x22, x21, [sp, #0x20]
1000468cc:     	stp	x20, x19, [sp, #0x30]
1000468d0:     	stp	x29, x30, [sp, #0x40]
1000468d4:     	add	x29, sp, #0x40
1000468d8:     	adrp	x21, 0x100160000 <dyld_stub_binder+0x100160000>
1000468dc:     	add	x21, x21, #0x800
1000468e0:     	ldr	x8, [x21, #0xe0]
1000468e4:     	cbz	x8,  <L6>
1000468e8:     	mov	x19, x1
1000468ec:     	mov	x20, x0
1000468f0:     	mov	x22, #0x0               ; =0
1000468f4:     	mov	w23, #0x180             ; =384
1000468f8:     	b	 <L2>
<L0>:
1000468fc:     	ldr	x8, [x21, #0xe0]
<L1>:
100046900:     	add	x22, x22, #0x1
100046904:     	cmp	x22, x8
100046908:     	b.hs	 <L6>
<L2>:
10004690c:     	madd	x9, x22, x23, x21
100046910:     	ldr	x24, [x9, #0x230]
100046914:     	cbnz	x24,  <L4>
100046918:     	b	 <L1>
<L3>:
10004691c:     	ldr	x24, [x24]
100046920:     	cbz	x24,  <L0>
<L4>:
100046924:     	ldr	x8, [x24, #0x8]
100046928:     	cbz	x8,  <L3>
10004692c:     	mov	x25, #0x0               ; =0
100046930:     	add	x26, x24, #0x10
<L5>:
100046934:     	ldr	x0, [x26, x25, lsl #3]
100046938:     	mov	x1, x19
10004693c:     	blr	x20
100046940:     	add	x25, x25, #0x1
100046944:     	ldr	x8, [x24, #0x8]
100046948:     	cmp	x25, x8
10004694c:     	b.lo	 <L5>
100046950:     	b	 <L3>
<L6>:
100046954:     	ldp	x29, x30, [sp, #0x40]
100046958:     	ldp	x20, x19, [sp, #0x30]
10004695c:     	ldp	x22, x21, [sp, #0x20]
100046960:     	ldp	x24, x23, [sp, #0x10]
100046964:     	ldp	x26, x25, [sp], #0x50
100046968:     	ret

0000000100047148 <_scoop_gc_mark_queue_push>:
100047148:     	stp	x24, x23, [sp, #-0x40]!
10004714c:     	stp	x22, x21, [sp, #0x10]
100047150:     	stp	x20, x19, [sp, #0x20]
100047154:     	stp	x29, x30, [sp, #0x30]
100047158:     	add	x29, sp, #0x30
10004715c:     	mov	x20, x1
100047160:     	mov	x19, x0
100047164:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
100047168:     	add	x8, x8, #0x800
10004716c:     	add	x8, x8, #0xd0
100047170:     	mov	w9, #0x1                ; =1
100047174:     	ldadd	x9, x22, [x8]
100047178:     	bl	 <dyld_stub_binder+0x10004a8fc>
10004717c:     	ldr	x8, [x19, #0x58]
100047180:     	ldr	x23, [x19, #0x48]
100047184:     	cmp	x8, x23
100047188:     	b.ne	 <L2>
10004718c:     	lsl	x9, x8, #1
100047190:     	mov	w10, #0x20              ; =32
100047194:     	cmp	x8, #0x0
100047198:     	csel	x23, x10, x9, eq
10004719c:     	cmp	x23, x8
1000471a0:     	mov	x8, #0x6666666666666666 ; =7378697629483820646
1000471a4:     	movk	x8, #0x6667
1000471a8:     	movk	x8, #0x666, lsl #48
1000471ac:     	ccmp	x23, x8, #0x2, hs
1000471b0:     	b.hs	 <L5>
1000471b4:     	add	x8, x23, x23, lsl #2
1000471b8:     	lsl	x0, x8, #3
1000471bc:     	bl	 <dyld_stub_binder+0x10004a800>
1000471c0:     	cbz	x0,  <L6>
1000471c4:     	mov	x21, x0
1000471c8:     	ldr	x8, [x19, #0x58]
1000471cc:     	cbz	x8,  <L1>
1000471d0:     	mov	x8, #0x0                ; =0
1000471d4:     	mov	w9, #0x28               ; =40
1000471d8:     	mov	x10, x21
<L0>:
1000471dc:     	ldp	x13, x11, [x19, #0x48]
1000471e0:     	add	x11, x8, x11
1000471e4:     	ldr	x12, [x19, #0x40]
1000471e8:     	udiv	x14, x11, x13
1000471ec:     	msub	x11, x14, x13, x11
1000471f0:     	madd	x11, x11, x9, x12
1000471f4:     	ldp	q0, q1, [x11]
1000471f8:     	ldr	x11, [x11, #0x20]
1000471fc:     	str	x11, [x10, #0x20]
100047200:     	stp	q0, q1, [x10]
100047204:     	add	x8, x8, #0x1
100047208:     	ldr	x11, [x19, #0x58]
10004720c:     	add	x10, x10, #0x28
100047210:     	cmp	x8, x11
100047214:     	b.lo	 <L0>
<L1>:
100047218:     	ldr	x0, [x19, #0x40]
10004721c:     	bl	 <dyld_stub_binder+0x10004a7ac>
100047220:     	mov	x9, #0x0                ; =0
100047224:     	stp	x21, x23, [x19, #0x40]
100047228:     	str	xzr, [x19, #0x50]
10004722c:     	ldr	x8, [x19, #0x58]
100047230:     	b	 <L3>
<L2>:
100047234:     	ldr	x21, [x19, #0x40]
100047238:     	ldr	x9, [x19, #0x50]
<L3>:
10004723c:     	add	x8, x8, x9
100047240:     	udiv	x9, x8, x23
100047244:     	msub	x8, x9, x23, x8
100047248:     	mov	w9, #0x28               ; =40
10004724c:     	madd	x8, x8, x9, x21
100047250:     	ldp	q0, q1, [x20]
100047254:     	ldr	x9, [x20, #0x20]
100047258:     	str	x9, [x8, #0x20]
10004725c:     	stp	q0, q1, [x8]
100047260:     	ldr	x8, [x19, #0x58]
100047264:     	add	x8, x8, #0x1
100047268:     	str	x8, [x19, #0x58]
10004726c:     	mov	x0, x19
100047270:     	bl	 <dyld_stub_binder+0x10004a908>
100047274:     	cmp	x22, #0x2
100047278:     	b.lo	 <L4>
10004727c:     	bl	 <_scoop_gc_mark_notify_work>
<L4>:
100047280:     	ldp	x29, x30, [sp, #0x30]
100047284:     	ldp	x20, x19, [sp, #0x20]
100047288:     	ldp	x22, x21, [sp, #0x10]
10004728c:     	ldp	x24, x23, [sp], #0x40
100047290:     	ret
<L5>:
100047294:     	adrp	x0, 0x100068000 <_check_release_hook.domain+0x1460>
100047298:     	add	x0, x0, #0xc8d
10004729c:     	bl	 <_scoop_heap_fatal>
<L6>:
1000472a0:     	adrp	x0, 0x100068000 <_check_release_hook.domain+0x1460>
1000472a4:     	add	x0, x0, #0xca6
1000472a8:     	bl	 <_scoop_heap_fatal>

00000001000472ac <_scoop_gc_mark_queue_take>:
1000472ac:     	stp	x24, x23, [sp, #-0x40]!
1000472b0:     	stp	x22, x21, [sp, #0x10]
1000472b4:     	stp	x20, x19, [sp, #0x20]
1000472b8:     	stp	x29, x30, [sp, #0x30]
1000472bc:     	add	x29, sp, #0x30
1000472c0:     	mov	x20, x1
1000472c4:     	mov	x19, x0
1000472c8:     	bl	 <dyld_stub_binder+0x10004a8fc>
1000472cc:     	ldr	x8, [x19, #0x58]
1000472d0:     	cbz	x8,  <L1>
1000472d4:     	ldp	x9, x10, [x19, #0x48]
1000472d8:     	add	x8, x8, x10
1000472dc:     	sub	x8, x8, #0x1
1000472e0:     	ldr	x10, [x19, #0x40]
1000472e4:     	udiv	x11, x8, x9
1000472e8:     	msub	x8, x11, x9, x8
1000472ec:     	mov	w9, #0x28               ; =40
1000472f0:     	madd	x8, x8, x9, x10
1000472f4:     	ldp	q0, q1, [x8]
1000472f8:     	ldr	x8, [x8, #0x20]
1000472fc:     	str	x8, [x20, #0x20]
100047300:     	stp	q0, q1, [x20]
100047304:     	ldr	x8, [x19, #0x58]
100047308:     	sub	x8, x8, #0x1
10004730c:     	str	x8, [x19, #0x58]
100047310:     	mov	x0, x19
100047314:     	bl	 <dyld_stub_binder+0x10004a908>
<L0>:
100047318:     	mov	w0, #0x1                ; =1
10004731c:     	b	 <L4>
<L1>:
100047320:     	mov	x0, x19
100047324:     	bl	 <dyld_stub_binder+0x10004a908>
100047328:     	adrp	x21, 0x100160000 <dyld_stub_binder+0x100160000>
10004732c:     	add	x21, x21, #0x800
100047330:     	ldr	x8, [x21, #0xe0]
100047334:     	cmp	x8, #0x2
100047338:     	b.lo	 <L3>
10004733c:     	mov	w23, #0x1               ; =1
100047340:     	mov	w24, #0x180             ; =384
<L2>:
100047344:     	ldr	x9, [x19, #0x80]
100047348:     	add	x9, x23, x9
10004734c:     	udiv	x10, x9, x8
100047350:     	msub	x8, x10, x8, x9
100047354:     	madd	x22, x8, x24, x21
100047358:     	add	x0, x22, #0x180
10004735c:     	bl	 <dyld_stub_binder+0x10004a8fc>
100047360:     	ldr	x8, [x22, #0x1d8]
100047364:     	cbnz	x8,  <L5>
100047368:     	add	x0, x22, #0x180
10004736c:     	bl	 <dyld_stub_binder+0x10004a908>
100047370:     	add	x23, x23, #0x1
100047374:     	ldr	x8, [x21, #0xe0]
100047378:     	cmp	x23, x8
10004737c:     	b.lo	 <L2>
<L3>:
100047380:     	mov	w0, #0x0                ; =0
<L4>:
100047384:     	ldp	x29, x30, [sp, #0x30]
100047388:     	ldp	x20, x19, [sp, #0x20]
10004738c:     	ldp	x22, x21, [sp, #0x10]
100047390:     	ldp	x24, x23, [sp], #0x40
100047394:     	ret
<L5>:
100047398:     	ldr	x8, [x22, #0x1d0]
10004739c:     	ldr	x9, [x22, #0x1c0]
1000473a0:     	mov	w10, #0x28              ; =40
1000473a4:     	madd	x8, x8, x10, x9
1000473a8:     	ldp	q0, q1, [x8]
1000473ac:     	ldr	x8, [x8, #0x20]
1000473b0:     	str	x8, [x20, #0x20]
1000473b4:     	stp	q0, q1, [x20]
1000473b8:     	ldp	x9, x8, [x22, #0x1c8]
1000473bc:     	add	x8, x8, #0x1
1000473c0:     	udiv	x10, x8, x9
1000473c4:     	msub	x8, x10, x9, x8
1000473c8:     	ldr	x9, [x22, #0x1d8]
1000473cc:     	sub	x9, x9, #0x1
1000473d0:     	stp	x8, x9, [x22, #0x1d0]
1000473d4:     	add	x0, x22, #0x180
1000473d8:     	bl	 <dyld_stub_binder+0x10004a908>
1000473dc:     	ldr	x8, [x19, #0x110]
1000473e0:     	add	x8, x8, #0x1
1000473e4:     	str	x8, [x19, #0x110]
1000473e8:     	b	 <L0>

00000001000473ec <_scoop_gc_mark_task_done>:
1000473ec:     	stp	x29, x30, [sp, #-0x10]!
1000473f0:     	mov	x29, sp
1000473f4:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
1000473f8:     	add	x8, x8, #0x800
1000473fc:     	add	x8, x8, #0xd0
100047400:     	mov	x9, #-0x1               ; =-1
100047404:     	ldaddal	x9, x8, [x8]
100047408:     	cmp	x8, #0x1
10004740c:     	b.eq	 <L0>
100047410:     	cbnz	x8,  <L1>
100047414:     	adrp	x0, 0x100068000 <_check_release_hook.domain+0x1460>
100047418:     	add	x0, x0, #0xc60
10004741c:     	bl	 <_scoop_heap_fatal>
<L0>:
100047420:     	bl	 <_scoop_gc_mark_notify_work>
<L1>:
100047424:     	ldp	x29, x30, [sp], #0x10
100047428:     	ret
