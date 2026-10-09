
/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/marker-off-darwin/mark-graphs:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100038118 <_collect>:
100038118:     	sub	sp, sp, #0xc0
10003811c:     	stp	x28, x27, [sp, #0x60]
100038120:     	stp	x26, x25, [sp, #0x70]
100038124:     	stp	x24, x23, [sp, #0x80]
100038128:     	stp	x22, x21, [sp, #0x90]
10003812c:     	stp	x20, x19, [sp, #0xa0]
100038130:     	stp	x29, x30, [sp, #0xb0]
100038134:     	add	x29, sp, #0xb0
100038138:     	mov	x20, x0
10003813c:     	bl	 <_scoop_thread_begin_collection>
100038140:     	cbz	w0,  <L24>
100038144:     	str	w0, [sp, #0x1c]
100038148:     	add	x1, sp, #0x40
10003814c:     	mov	w0, #0x6                ; =6
100038150:     	bl	 <dyld_stub_binder+0x100049044>
100038154:     	cbnz	w0,  <L25>
100038158:     	ldp	x8, x19, [sp, #0x40]
10003815c:     	str	x8, [sp, #0x10]
100038160:     	bl	 <_scoop_gc_heap_lock>
100038164:     	bl	 <_scoop_gc_roots_lock>
100038168:     	mov	w0, #0x1                ; =1
10003816c:     	bl	 <_scoop_gc_set_pin_frames_locked>
100038170:     	bl	 <_scoop_thread_collection_registry_head>
100038174:     	cbz	x0,  <L1>
<L0>:
100038178:     	stp	xzr, xzr, [x0, #0xc0]
10003817c:     	str	xzr, [x0, #0xd0]
100038180:     	ldr	x0, [x0, #0xe0]
100038184:     	cbnz	x0,  <L0>
<L1>:
100038188:     	adrp	x26, 0x100160000 <dyld_stub_binder+0x100160000>
10003818c:     	ldr	x8, [x26, #0x7e8]
100038190:     	cbnz	x8,  <L26>
100038194:     	str	x19, [sp, #0x8]
100038198:     	mov	x0, x20
10003819c:     	bl	 <_scoop_gc_heap_begin_collection_locked>
1000381a0:     	adrp	x28, 0x100160000 <dyld_stub_binder+0x100160000>
1000381a4:     	mov	w22, #0x1               ; =1
1000381a8:     	adrp	x25, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
1000381ac:     	add	x25, x25, #0x92c
1000381b0:     	add	x24, sp, #0x30
1000381b4:     	adrp	x27, 0x100160000 <dyld_stub_binder+0x100160000>
1000381b8:     	adrp	x23, 0x100160000 <dyld_stub_binder+0x100160000>
1000381bc:     	add	x23, x23, #0x428
1000381c0:     	adrp	x21, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
1000381c4:     	add	x21, x21, #0x62c
<L2>:
1000381c8:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
1000381cc:     	str	xzr, [x8, #0x7f0]
1000381d0:     	str	xzr, [x28, #0x7f8]
1000381d4:     	str	xzr, [x26, #0x7e8]
1000381d8:     	str	wzr, [sp, #0x30]
1000381dc:     	and	w19, w20, #0x1
1000381e0:     	strb	w19, [sp, #0x34]
1000381e4:     	strb	wzr, [sp, #0x37]
1000381e8:     	sturh	w22, [sp, #0x35]
1000381ec:     	adrp	x8, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
1000381f0:     	add	x8, x8, #0x788
1000381f4:     	stp	xzr, x8, [sp, #0x38]
1000381f8:     	adrp	x8, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
1000381fc:     	add	x8, x8, #0x940
100038200:     	stp	x25, x8, [sp, #0x48]
100038204:     	str	x24, [sp, #0x58]
100038208:     	add	x0, sp, #0x40
10003820c:     	bl	 <_scoop_gc_scan_roots>
100038210:     	strb	wzr, [sp, #0x35]
100038214:     	tbz	w20, #0x0,  <L4>
100038218:     	add	x1, sp, #0x30
10003821c:     	mov	x0, x21
100038220:     	mov	w2, #0x1                ; =1
100038224:     	bl	 <_scoop_gc_scan_remembered>
100038228:     	b	 <L4>
<L3>:
10003822c:     	ldr	x9, [x27, #0x800]
100038230:     	add	x10, x8, #0x1
100038234:     	str	x10, [x28, #0x7f8]
100038238:     	ldr	x0, [x9, x8, lsl #3]
10003823c:     	add	x1, sp, #0x30
100038240:     	bl	 <_visit_object>
<L4>:
100038244:     	ldr	x8, [x28, #0x7f8]
100038248:     	ldr	x9, [x26, #0x7e8]
10003824c:     	cmp	x8, x9
100038250:     	b.lo	 <L3>
100038254:     	mov	x0, x19
100038258:     	bl	 <_scoop_gc_heap_plan_moving_locked>
10003825c:     	cbz	w19,  <L5>
100038260:     	cbnz	w0,  <L5>
100038264:     	ldr	x8, [x23, #0xb8]
100038268:     	add	x8, x8, #0x1
10003826c:     	str	x8, [x23, #0xb8]
100038270:     	strb	wzr, [x23, #0x9d]
100038274:     	bl	 <_scoop_gc_heap_begin_collection_locked>
100038278:     	mov	w20, #0x0               ; =0
10003827c:     	b	 <L2>
<L5>:
100038280:     	mov	w8, #0x1                ; =1
100038284:     	str	w8, [sp, #0x30]
100038288:     	strb	w19, [sp, #0x34]
10003828c:     	strh	wzr, [sp, #0x36]
100038290:     	str	xzr, [sp, #0x38]
100038294:     	cbz	w19,  <L8>
100038298:     	strb	w8, [sp, #0x35]
10003829c:     	adrp	x8, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
1000382a0:     	add	x8, x8, #0x788
1000382a4:     	stp	x8, x25, [sp, #0x40]
1000382a8:     	add	x8, sp, #0x30
1000382ac:     	adrp	x9, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
1000382b0:     	add	x9, x9, #0x940
1000382b4:     	stp	x9, x8, [sp, #0x50]
1000382b8:     	add	x0, sp, #0x40
1000382bc:     	bl	 <_scoop_gc_scan_roots>
1000382c0:     	strb	wzr, [sp, #0x35]
1000382c4:     	adrp	x0, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
1000382c8:     	add	x0, x0, #0x62c
1000382cc:     	add	x1, sp, #0x30
1000382d0:     	mov	w2, #0x0                ; =0
1000382d4:     	bl	 <_scoop_gc_scan_remembered>
1000382d8:     	ldr	x8, [x26, #0x7e8]
1000382dc:     	cbz	x8,  <L7>
1000382e0:     	mov	x21, #0x0               ; =0
<L6>:
1000382e4:     	ldr	x8, [x27, #0x800]
1000382e8:     	ldr	x0, [x8, x21, lsl #3]
1000382ec:     	bl	 <_scoop_gc_forward_object_locked>
1000382f0:     	mov	x20, x0
1000382f4:     	bl	 <_scoop_gc_claim_object_scan_locked>
1000382f8:     	add	x1, sp, #0x30
1000382fc:     	mov	x0, x20
100038300:     	bl	 <_visit_object>
100038304:     	add	x21, x21, #0x1
100038308:     	ldr	x8, [x26, #0x7e8]
10003830c:     	cmp	x21, x8
100038310:     	b.lo	 <L6>
<L7>:
100038314:     	ldr	x8, [x23, #0xa8]
100038318:     	add	x8, x8, #0x1
10003831c:     	str	x8, [x23, #0xa8]
100038320:     	bl	 <_scoop_gc_stress_move_enabled>
100038324:     	cbnz	w0,  <L11>
100038328:     	b	 <L12>
<L8>:
10003832c:     	str	xzr, [x28, #0x7f8]
100038330:     	str	xzr, [x26, #0x7e8]
100038334:     	strb	w8, [sp, #0x35]
100038338:     	adrp	x8, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
10003833c:     	add	x8, x8, #0x788
100038340:     	stp	x8, x25, [sp, #0x40]
100038344:     	add	x8, sp, #0x30
100038348:     	adrp	x9, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
10003834c:     	add	x9, x9, #0x940
100038350:     	stp	x9, x8, [sp, #0x50]
100038354:     	add	x0, sp, #0x40
100038358:     	bl	 <_scoop_gc_scan_roots>
10003835c:     	strb	wzr, [sp, #0x35]
<L9>:
100038360:     	ldr	x8, [x28, #0x7f8]
100038364:     	ldr	x9, [x26, #0x7e8]
100038368:     	cmp	x8, x9
10003836c:     	b.hs	 <L10>
100038370:     	ldr	x9, [x27, #0x800]
100038374:     	add	x10, x8, #0x1
100038378:     	str	x10, [x28, #0x7f8]
10003837c:     	ldr	x0, [x9, x8, lsl #3]
100038380:     	add	x1, sp, #0x30
100038384:     	bl	 <_visit_object>
100038388:     	b	 <L9>
<L10>:
10003838c:     	ldr	x8, [x23, #0xb0]
100038390:     	add	x8, x8, #0x1
100038394:     	str	x8, [x23, #0xb0]
100038398:     	bl	 <_scoop_gc_stress_move_enabled>
10003839c:     	cbz	w0,  <L12>
<L11>:
1000383a0:     	mov	w8, #0x2                ; =2
1000383a4:     	str	w8, [sp, #0x20]
1000383a8:     	strb	w19, [sp, #0x24]
1000383ac:     	strb	wzr, [sp, #0x27]
1000383b0:     	str	xzr, [sp, #0x28]
1000383b4:     	mov	w8, #0x1                ; =1
1000383b8:     	sturh	w8, [sp, #0x25]
1000383bc:     	adrp	x8, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
1000383c0:     	add	x8, x8, #0x788
1000383c4:     	stp	x8, x25, [sp, #0x40]
1000383c8:     	add	x8, sp, #0x20
1000383cc:     	adrp	x9, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
1000383d0:     	add	x9, x9, #0x940
1000383d4:     	stp	x9, x8, [sp, #0x50]
1000383d8:     	add	x0, sp, #0x40
1000383dc:     	bl	 <_scoop_gc_scan_roots>
1000383e0:     	strb	wzr, [sp, #0x25]
1000383e4:     	adrp	x0, 0x100038000 <_scoop_runtime_callback_debug_active_count+0x98>
1000383e8:     	add	x0, x0, #0x750
1000383ec:     	add	x1, sp, #0x20
1000383f0:     	bl	 <_scoop_gc_visit_current_objects_locked>
<L12>:
1000383f4:     	bl	 <_scoop_gc_stress_move_enabled>
1000383f8:     	cbz	w0,  <L13>
1000383fc:     	bl	 <_scoop_gc_heap_verify_stress_moved_locked>
<L13>:
100038400:     	str	xzr, [x28, #0x7f8]
100038404:     	str	xzr, [x26, #0x7e8]
100038408:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
10003840c:     	ldr	x0, [x8, #0x7f0]
100038410:     	mov	x1, x19
100038414:     	bl	 <_scoop_gc_heap_finish_collection_locked>
100038418:     	mov	w0, #0x0                ; =0
10003841c:     	bl	 <_scoop_gc_set_pin_frames_locked>
100038420:     	add	x1, sp, #0x40
100038424:     	mov	w0, #0x6                ; =6
100038428:     	bl	 <dyld_stub_binder+0x100049044>
10003842c:     	cbnz	w0,  <L27>
100038430:     	ldp	x8, x9, [sp, #0x40]
100038434:     	ldp	x11, x10, [sp, #0x8]
100038438:     	sub	x8, x8, x10
10003843c:     	mov	w10, #0xca00            ; =51712
100038440:     	movk	w10, #0x3b9a, lsl #16
100038444:     	sub	x9, x9, x11
100038448:     	madd	x8, x8, x10, x9
10003844c:     	ldp	x9, x10, [x23, #0xf8]
100038450:     	add	x9, x9, x8
100038454:     	str	x9, [x23, #0xf8]
100038458:     	cmp	x8, x10
10003845c:     	b.ls	 <L14>
100038460:     	str	x8, [x23, #0x100]
<L14>:
100038464:     	cbz	w19,  <L15>
100038468:     	ldr	x9, [x23, #0x160]
10003846c:     	add	x9, x9, x8
100038470:     	str	x9, [x23, #0x160]
100038474:     	b	 <L16>
<L15>:
100038478:     	ldr	x9, [x23, #0x168]
10003847c:     	add	x9, x9, x8
100038480:     	str	x9, [x23, #0x168]
<L16>:
100038484:     	mov	w9, #0x2711             ; =10001
100038488:     	cmp	x8, x9
10003848c:     	b.lo	 <L17>
100038490:     	mov	w9, #0xc351             ; =50001
100038494:     	cmp	x8, x9
100038498:     	b.lo	 <L18>
10003849c:     	mov	w9, #0x86a1             ; =34465
1000384a0:     	movk	w9, #0x1, lsl #16
1000384a4:     	cmp	x8, x9
1000384a8:     	b.lo	 <L19>
1000384ac:     	mov	w9, #0xa121             ; =41249
1000384b0:     	movk	w9, #0x7, lsl #16
1000384b4:     	cmp	x8, x9
1000384b8:     	b.lo	 <L20>
1000384bc:     	mov	w9, #0x4241             ; =16961
1000384c0:     	movk	w9, #0xf, lsl #16
1000384c4:     	cmp	x8, x9
1000384c8:     	b.lo	 <L21>
1000384cc:     	mov	w9, #0x4b41             ; =19265
1000384d0:     	movk	w9, #0x4c, lsl #16
1000384d4:     	cmp	x8, x9
1000384d8:     	b.lo	 <L22>
1000384dc:     	mov	w9, #0x9680             ; =38528
1000384e0:     	movk	w9, #0x98, lsl #16
1000384e4:     	cmp	x8, x9
1000384e8:     	mov	w8, #0x6                ; =6
1000384ec:     	cinc	x8, x8, hi
1000384f0:     	b	 <L23>
<L17>:
1000384f4:     	mov	x8, #0x0                ; =0
1000384f8:     	b	 <L23>
<L18>:
1000384fc:     	mov	w8, #0x1                ; =1
100038500:     	b	 <L23>
<L19>:
100038504:     	mov	w8, #0x2                ; =2
100038508:     	b	 <L23>
<L20>:
10003850c:     	mov	w8, #0x3                ; =3
100038510:     	b	 <L23>
<L21>:
100038514:     	mov	w8, #0x4                ; =4
100038518:     	b	 <L23>
<L22>:
10003851c:     	mov	w8, #0x5                ; =5
<L23>:
100038520:     	add	x8, x23, x8, lsl #3
100038524:     	ldr	x9, [x8, #0x170]
100038528:     	add	x9, x9, #0x1
10003852c:     	str	x9, [x8, #0x170]
100038530:     	bl	 <_scoop_gc_roots_unlock>
100038534:     	bl	 <_scoop_gc_heap_unlock>
100038538:     	bl	 <_scoop_thread_end_collection>
10003853c:     	ldr	w0, [sp, #0x1c]
<L24>:
100038540:     	ldp	x29, x30, [sp, #0xb0]
100038544:     	ldp	x20, x19, [sp, #0xa0]
100038548:     	ldp	x22, x21, [sp, #0x90]
10003854c:     	ldp	x24, x23, [sp, #0x80]
100038550:     	ldp	x26, x25, [sp, #0x70]
100038554:     	ldp	x28, x27, [sp, #0x60]
100038558:     	add	sp, sp, #0xc0
10003855c:     	ret
<L25>:
100038560:     	bl	 <_collect.cold.1>
<L26>:
100038564:     	bl	 <_collect.cold.2>
<L27>:
100038568:     	bl	 <_collect.cold.3>

0000000100038788 <_visit_managed_slot>:
100038788:     	stp	x24, x23, [sp, #-0x40]!
10003878c:     	stp	x22, x21, [sp, #0x10]
100038790:     	stp	x20, x19, [sp, #0x20]
100038794:     	stp	x29, x30, [sp, #0x30]
100038798:     	add	x29, sp, #0x30
10003879c:     	mov	x20, x1
1000387a0:     	mov	x21, x0
1000387a4:     	ldr	w8, [x1]
1000387a8:     	cbnz	w8,  <L1>
1000387ac:     	ldrb	w8, [x20, #0x5]
1000387b0:     	cmp	w8, #0x1
1000387b4:     	b.ne	 <L0>
1000387b8:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
1000387bc:     	add	x8, x8, #0x428
1000387c0:     	ldr	x9, [x8, #0xe8]
1000387c4:     	add	x9, x9, #0x1
1000387c8:     	str	x9, [x8, #0xe8]
1000387cc:     	b	 <L1>
<L0>:
1000387d0:     	ldrb	w8, [x20, #0x6]
1000387d4:     	cmp	w8, #0x1
1000387d8:     	b.ne	 <L1>
1000387dc:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
1000387e0:     	add	x8, x8, #0x428
1000387e4:     	ldr	x9, [x8, #0xe0]
1000387e8:     	add	x9, x9, #0x1
1000387ec:     	str	x9, [x8, #0xe0]
<L1>:
1000387f0:     	ldr	x19, [x21]
1000387f4:     	cbz	x19,  <L7>
1000387f8:     	mov	x0, x19
1000387fc:     	bl	 <_scoop_gc_is_object_start_locked>
100038800:     	tbz	w0, #0x0,  <L4>
100038804:     	ldrb	w8, [x20, #0x4]
100038808:     	cmp	w8, #0x1
10003880c:     	b.ne	 <L2>
100038810:     	mov	x0, x19
100038814:     	bl	 <_scoop_gc_is_young_object_locked>
100038818:     	cbz	w0,  <L7>
<L2>:
10003881c:     	ldr	w8, [x20]
100038820:     	cmp	w8, #0x2
100038824:     	b.eq	 <L6>
100038828:     	cmp	w8, #0x1
10003882c:     	b.eq	 <L5>
100038830:     	cbnz	w8,  <L8>
100038834:     	mov	x0, x19
100038838:     	bl	 <_scoop_gc_mark_object_locked>
10003883c:     	cbz	w0,  <L7>
100038840:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
100038844:     	ldr	x9, [x8, #0x7f0]
100038848:     	add	x9, x9, #0x1
10003884c:     	str	x9, [x8, #0x7f0]
100038850:     	adrp	x20, 0x100160000 <dyld_stub_binder+0x100160000>
100038854:     	ldr	x8, [x20, #0x7e8]
100038858:     	adrp	x21, 0x100160000 <dyld_stub_binder+0x100160000>
10003885c:     	ldr	x9, [x21, #0x808]
100038860:     	adrp	x22, 0x100160000 <dyld_stub_binder+0x100160000>
100038864:     	ldr	x0, [x22, #0x800]
100038868:     	cmp	x8, x9
10003886c:     	b.ne	 <L3>
100038870:     	lsl	x9, x8, #1
100038874:     	mov	w10, #0x100             ; =256
100038878:     	cmp	x8, #0x0
10003887c:     	csel	x23, x10, x9, eq
100038880:     	lsl	x1, x23, #3
100038884:     	bl	 <dyld_stub_binder+0x1000491dc>
100038888:     	cbz	x0,  <L11>
10003888c:     	str	x0, [x22, #0x800]
100038890:     	str	x23, [x21, #0x808]
100038894:     	ldr	x8, [x20, #0x7e8]
<L3>:
100038898:     	add	x9, x8, #0x1
10003889c:     	str	x9, [x20, #0x7e8]
1000388a0:     	str	x19, [x0, x8, lsl #3]
1000388a4:     	b	 <L7>
<L4>:
1000388a8:     	mov	x0, x19
1000388ac:     	bl	 <_scoop_gc_is_immortal_object_locked>
1000388b0:     	tbnz	w0, #0x0,  <L7>
1000388b4:     	mov	x0, x19
1000388b8:     	bl	 <_scoop_gc_is_external_object_locked>
1000388bc:     	tbnz	w0, #0x0,  <L7>
1000388c0:     	bl	 <_visit_managed_slot.cold.1>
<L5>:
1000388c4:     	mov	x0, x19
1000388c8:     	bl	 <_scoop_gc_forward_object_locked>
1000388cc:     	str	x0, [x21]
1000388d0:     	ldrb	w8, [x20, #0x4]
1000388d4:     	tbnz	w8, #0x0,  <L7>
1000388d8:     	mov	x19, x0
1000388dc:     	bl	 <_scoop_gc_claim_object_scan_locked>
1000388e0:     	cbz	w0,  <L7>
1000388e4:     	mov	x0, x19
1000388e8:     	bl	 <_work_push>
1000388ec:     	b	 <L7>
<L6>:
1000388f0:     	mov	x0, x19
1000388f4:     	bl	 <_scoop_gc_is_forwarded_old_locked>
1000388f8:     	cbnz	w0,  <L9>
1000388fc:     	mov	x0, x19
100038900:     	bl	 <_scoop_gc_is_current_live_object_locked>
100038904:     	tbz	w0, #0x0,  <L10>
<L7>:
100038908:     	ldp	x29, x30, [sp, #0x30]
10003890c:     	ldp	x20, x19, [sp, #0x20]
100038910:     	ldp	x22, x21, [sp, #0x10]
100038914:     	ldp	x24, x23, [sp], #0x40
100038918:     	ret
<L8>:
10003891c:     	bl	 <_visit_managed_slot.cold.5>
<L9>:
100038920:     	bl	 <_visit_managed_slot.cold.3>
<L10>:
100038924:     	bl	 <_visit_managed_slot.cold.2>
<L11>:
100038928:     	bl	 <_visit_managed_slot.cold.4>

00000001000391ac <_scoop_gc_mark_object_locked>:
1000391ac:     	sub	sp, sp, #0x60
1000391b0:     	stp	x24, x23, [sp, #0x20]
1000391b4:     	stp	x22, x21, [sp, #0x30]
1000391b8:     	stp	x20, x19, [sp, #0x40]
1000391bc:     	stp	x29, x30, [sp, #0x50]
1000391c0:     	add	x29, sp, #0x50
1000391c4:     	adrp	x8, 0x100160000 <dyld_stub_binder+0x100160000>
1000391c8:     	add	x8, x8, #0x428
1000391cc:     	ldrb	w8, [x8, #0x9d]
1000391d0:     	tbz	w8, #0x0,  <L12>
1000391d4:     	mov	x20, x0
1000391d8:     	add	x1, sp, #0x8
1000391dc:     	mov	x2, sp
1000391e0:     	bl	 <_scoop_heap_object_meta>
1000391e4:     	tbz	w0, #0x0,  <L13>
1000391e8:     	ldp	x21, x19, [sp]
1000391ec:     	ldr	w8, [x19, #0x18]
1000391f0:     	cmp	w8, #0x2
1000391f4:     	b.ne	 <L1>
1000391f8:     	ldrb	w8, [x19, #0x81]
1000391fc:     	cbz	w8,  <L2>
<L0>:
100039200:     	mov	w0, #0x0                ; =0
100039204:     	b	 <L11>
<L1>:
100039208:     	ldr	x0, [x19, #0x28]
10003920c:     	mov	x1, x21
100039210:     	bl	 <_scoop_heap_bit_test>
100039214:     	tbnz	w0, #0x0,  <L0>
<L2>:
100039218:     	add	x1, sp, #0x18
10003921c:     	add	x2, sp, #0x10
100039220:     	mov	x0, x20
100039224:     	bl	 <_scoop_heap_object_meta>
100039228:     	tbz	w0, #0x0,  <L14>
10003922c:     	ldr	x8, [sp, #0x18]
100039230:     	ldr	w9, [x8, #0x18]
100039234:     	cmp	w9, #0x2
100039238:     	b.ne	 <L3>
10003923c:     	ldr	x23, [x8, #0x60]
100039240:     	b	 <L4>
<L3>:
100039244:     	ldr	x8, [x8, #0x50]
100039248:     	ldr	x9, [sp, #0x10]
10003924c:     	ldrh	w8, [x8, x9, lsl #1]
100039250:     	cbz	x8,  <L15>
100039254:     	lsl	x23, x8, #3
<L4>:
100039258:     	ldr	w8, [x19, #0x18]
10003925c:     	cmp	w8, #0x2
100039260:     	b.ne	 <L5>
100039264:     	ldrb	w8, [x19, #0x82]
100039268:     	mov	w0, #0x1                ; =1
10003926c:     	strb	w0, [x19, #0x81]
100039270:     	ldr	x9, [x19, #0x68]
100039274:     	add	x9, x9, x23
100039278:     	str	x9, [x19, #0x68]
10003927c:     	cbnz	w8,  <L11>
100039280:     	b	 <L9>
<L5>:
100039284:     	ldr	x0, [x19, #0x30]
100039288:     	mov	x1, x21
10003928c:     	bl	 <_scoop_heap_bit_test>
100039290:     	mov	x22, x0
100039294:     	ldr	w8, [x19, #0x18]
100039298:     	cmp	w8, #0x2
10003929c:     	b.ne	 <L6>
1000392a0:     	mov	w0, #0x1                ; =1
1000392a4:     	strb	w0, [x19, #0x81]
1000392a8:     	ldr	x8, [x19, #0x68]
1000392ac:     	add	x8, x8, x23
1000392b0:     	str	x8, [x19, #0x68]
1000392b4:     	tbz	w22, #0x0,  <L9>
1000392b8:     	b	 <L11>
<L6>:
1000392bc:     	ldr	x0, [x19, #0x28]
1000392c0:     	mov	x1, x21
1000392c4:     	bl	 <_scoop_heap_bit_set>
1000392c8:     	mov	x0, x19
1000392cc:     	bl	 <_scoop_heap_block_base>
1000392d0:     	sub	x8, x20, x0
1000392d4:     	add	x9, x23, x8
1000392d8:     	sub	x9, x9, #0x1
1000392dc:     	lsr	x20, x8, #7
1000392e0:     	lsr	x8, x9, #7
1000392e4:     	cmp	x20, x8
1000392e8:     	b.hi	 <L8>
1000392ec:     	add	x21, x8, #0x1
<L7>:
1000392f0:     	ldr	x0, [x19, #0x48]
1000392f4:     	mov	x1, x20
1000392f8:     	bl	 <_scoop_heap_bit_set>
1000392fc:     	add	x20, x20, #0x1
100039300:     	cmp	x21, x20
100039304:     	b.ne	 <L7>
<L8>:
100039308:     	ldr	x8, [x19, #0x68]
10003930c:     	add	x8, x8, x23
100039310:     	str	x8, [x19, #0x68]
100039314:     	tbnz	w22, #0x0,  <L10>
<L9>:
100039318:     	ldr	x8, [x19, #0x70]
10003931c:     	add	x8, x8, x23
100039320:     	str	x8, [x19, #0x70]
<L10>:
100039324:     	mov	w0, #0x1                ; =1
<L11>:
100039328:     	ldp	x29, x30, [sp, #0x50]
10003932c:     	ldp	x20, x19, [sp, #0x40]
100039330:     	ldp	x22, x21, [sp, #0x30]
100039334:     	ldp	x24, x23, [sp, #0x20]
100039338:     	add	sp, sp, #0x60
10003933c:     	ret
<L12>:
100039340:     	adrp	x0, 0x100064000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1920>
100039344:     	add	x0, x0, #0x138
100039348:     	bl	 <_scoop_heap_fatal>
<L13>:
10003934c:     	adrp	x0, 0x100064000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1920>
100039350:     	add	x0, x0, #0x159
100039354:     	bl	 <_scoop_heap_fatal>
<L14>:
100039358:     	adrp	x0, 0x100063000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x920>
10003935c:     	add	x0, x0, #0xfea
100039360:     	bl	 <_scoop_heap_fatal>
<L15>:
100039364:     	adrp	x0, 0x100064000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x1920>
100039368:     	add	x0, x0, #0x18
10003936c:     	bl	 <_scoop_heap_fatal>

00000001000434b4 <_scoop_gc_heap_plan_moving_locked>:
1000434b4:     	sub	sp, sp, #0x70
1000434b8:     	stp	x26, x25, [sp, #0x20]
1000434bc:     	stp	x24, x23, [sp, #0x30]
1000434c0:     	stp	x22, x21, [sp, #0x40]
1000434c4:     	stp	x20, x19, [sp, #0x50]
1000434c8:     	stp	x29, x30, [sp, #0x60]
1000434cc:     	add	x29, sp, #0x60
1000434d0:     	adrp	x23, 0x100160000 <dyld_stub_binder+0x100160000>
1000434d4:     	add	x23, x23, #0x428
1000434d8:     	ldrb	w8, [x23, #0x9d]
1000434dc:     	tbz	w8, #0x0,  <L44>
1000434e0:     	mov	x20, x0
1000434e4:     	bl	 <_scoop_heap_select_evacuation_sources>
1000434e8:     	mov	x19, x0
1000434ec:     	ldrb	w8, [x23, #0x99]
1000434f0:     	orr	w24, w20, w8
1000434f4:     	tbnz	w24, #0x0,  <L0>
1000434f8:     	cbz	x19,  <L35>
<L0>:
1000434fc:     	tbnz	w24, #0x0,  <L1>
100043500:     	bl	 <_scoop_heap_prepare_evacuation_targets>
<L1>:
100043504:     	cmp	x19, #0x1
100043508:     	stp	xzr, xzr, [sp, #0x10]
10004350c:     	cset	w20, hi
100043510:     	str	xzr, [sp, #0x8]
100043514:     	bl	 <_scoop_heap_first_block>
100043518:     	cbz	x0,  <L10>
10004351c:     	mov	x21, x0
100043520:     	eor	w25, w24, #0x1
100043524:     	orr	w26, w24, w20
<L2>:
100043528:     	ldr	w8, [x21, #0x14]
10004352c:     	cmp	w8, #0x3
100043530:     	b.ne	 <L3>
100043534:     	ldr	w8, [x21, #0x18]
100043538:     	cmp	w8, #0x2
10004353c:     	b.ne	 <L5>
100043540:     	add	x0, sp, #0x18
100043544:     	add	x1, sp, #0x10
100043548:     	add	x2, sp, #0x8
10004354c:     	and	w5, w25, #0x1
100043550:     	and	w6, w26, #0x1
100043554:     	mov	x3, x21
100043558:     	mov	w4, #0x10               ; =16
10004355c:     	bl	 <_append_move>
100043560:     	mov	x20, x0
100043564:     	b	 <L4>
<L3>:
100043568:     	mov	w20, #0x1               ; =1
<L4>:
10004356c:     	mov	x0, x21
100043570:     	bl	 <_scoop_heap_next_block>
100043574:     	cbz	w20,  <L9>
100043578:     	mov	x21, x0
10004357c:     	cbnz	x0,  <L2>
100043580:     	b	 <L9>
<L5>:
100043584:     	mov	w22, #0x10              ; =16
100043588:     	b	 <L8>
<L6>:
10004358c:     	mov	w20, #0x1               ; =1
100043590:     	cbz	w20,  <L4>
<L7>:
100043594:     	cmp	x22, #0xfff
100043598:     	add	x22, x22, #0x1
10004359c:     	b.hs	 <L4>
<L8>:
1000435a0:     	ldr	x0, [x21, #0x20]
1000435a4:     	mov	x1, x22
1000435a8:     	bl	 <_scoop_heap_bit_test>
1000435ac:     	cbz	w0,  <L6>
1000435b0:     	ldr	x0, [x21, #0x28]
1000435b4:     	mov	x1, x22
1000435b8:     	bl	 <_scoop_heap_bit_test>
1000435bc:     	cbz	w0,  <L6>
1000435c0:     	ldr	x0, [x21, #0x30]
1000435c4:     	mov	x1, x22
1000435c8:     	bl	 <_scoop_heap_bit_test>
1000435cc:     	tbnz	w0, #0x0,  <L6>
1000435d0:     	add	x0, sp, #0x18
1000435d4:     	add	x1, sp, #0x10
1000435d8:     	add	x2, sp, #0x8
1000435dc:     	and	w5, w25, #0x1
1000435e0:     	and	w6, w26, #0x1
1000435e4:     	mov	x3, x21
1000435e8:     	mov	x4, x22
1000435ec:     	bl	 <_append_move>
1000435f0:     	mov	x20, x0
1000435f4:     	cbnz	w20,  <L7>
1000435f8:     	b	 <L4>
<L9>:
1000435fc:     	tbz	w24, #0x0,  <L13>
100043600:     	cbnz	w20,  <L11>
100043604:     	b	 <L36>
<L10>:
100043608:     	tbz	w24, #0x0,  <L12>
<L11>:
10004360c:     	ldr	x21, [sp, #0x10]
100043610:     	b	 <L30>
<L12>:
100043614:     	mov	w20, #0x1               ; =1
<L13>:
100043618:     	ldr	x21, [sp, #0x10]
10004361c:     	cbz	x21,  <L14>
100043620:     	ldr	x10, [sp, #0x18]
100043624:     	and	x8, x21, #0x7
100043628:     	cmp	x21, #0x8
10004362c:     	b.hs	 <L15>
100043630:     	mov	x11, #0x0               ; =0
100043634:     	mov	x9, #0x0                ; =0
100043638:     	cbnz	x8,  <L26>
10004363c:     	b	 <L29>
<L14>:
100043640:     	mov	x9, #0x0                ; =0
100043644:     	b	 <L29>
<L15>:
100043648:     	mov	x11, #0x0               ; =0
10004364c:     	mov	x9, #0x0                ; =0
100043650:     	and	x12, x21, #0xfffffffffffffff8
100043654:     	neg	x12, x12
100043658:     	add	x13, x10, #0xe8
10004365c:     	mov	w14, #0x1               ; =1
100043660:     	b	 <L17>
<L16>:
100043664:     	sub	x11, x11, #0x8
100043668:     	add	x13, x13, #0x180
10004366c:     	cmp	x12, x11
100043670:     	b.eq	 <L25>
<L17>:
100043674:     	ldur	x15, [x13, #-0xc0]
100043678:     	ldr	x15, [x15]
10004367c:     	ldr	x16, [x15, #0x30]
100043680:     	cbnz	x16,  <L18>
100043684:     	ldrb	w16, [x15, #0x2c]
100043688:     	tbnz	w16, #0x0,  <L18>
10004368c:     	strb	w14, [x15, #0x2c]
100043690:     	add	x9, x9, #0x1
<L18>:
100043694:     	ldur	x15, [x13, #-0x90]
100043698:     	ldr	x15, [x15]
10004369c:     	ldr	x16, [x15, #0x30]
1000436a0:     	cbnz	x16,  <L19>
1000436a4:     	ldrb	w16, [x15, #0x2c]
1000436a8:     	tbnz	w16, #0x0,  <L19>
1000436ac:     	strb	w14, [x15, #0x2c]
1000436b0:     	add	x9, x9, #0x1
<L19>:
1000436b4:     	ldur	x15, [x13, #-0x60]
1000436b8:     	ldr	x15, [x15]
1000436bc:     	ldr	x16, [x15, #0x30]
1000436c0:     	cbnz	x16,  <L20>
1000436c4:     	ldrb	w16, [x15, #0x2c]
1000436c8:     	tbnz	w16, #0x0,  <L20>
1000436cc:     	strb	w14, [x15, #0x2c]
1000436d0:     	add	x9, x9, #0x1
<L20>:
1000436d4:     	ldur	x15, [x13, #-0x30]
1000436d8:     	ldr	x15, [x15]
1000436dc:     	ldr	x16, [x15, #0x30]
1000436e0:     	cbnz	x16,  <L21>
1000436e4:     	ldrb	w16, [x15, #0x2c]
1000436e8:     	tbnz	w16, #0x0,  <L21>
1000436ec:     	strb	w14, [x15, #0x2c]
1000436f0:     	add	x9, x9, #0x1
<L21>:
1000436f4:     	ldr	x15, [x13]
1000436f8:     	ldr	x15, [x15]
1000436fc:     	ldr	x16, [x15, #0x30]
100043700:     	cbnz	x16,  <L22>
100043704:     	ldrb	w16, [x15, #0x2c]
100043708:     	tbnz	w16, #0x0,  <L22>
10004370c:     	strb	w14, [x15, #0x2c]
100043710:     	add	x9, x9, #0x1
<L22>:
100043714:     	ldr	x15, [x13, #0x30]
100043718:     	ldr	x15, [x15]
10004371c:     	ldr	x16, [x15, #0x30]
100043720:     	cbnz	x16,  <L23>
100043724:     	ldrb	w16, [x15, #0x2c]
100043728:     	tbnz	w16, #0x0,  <L23>
10004372c:     	strb	w14, [x15, #0x2c]
100043730:     	add	x9, x9, #0x1
<L23>:
100043734:     	ldr	x15, [x13, #0x60]
100043738:     	ldr	x15, [x15]
10004373c:     	ldr	x16, [x15, #0x30]
100043740:     	cbnz	x16,  <L24>
100043744:     	ldrb	w16, [x15, #0x2c]
100043748:     	tbnz	w16, #0x0,  <L24>
10004374c:     	strb	w14, [x15, #0x2c]
100043750:     	add	x9, x9, #0x1
<L24>:
100043754:     	ldr	x15, [x13, #0x90]
100043758:     	ldr	x15, [x15]
10004375c:     	ldr	x16, [x15, #0x30]
100043760:     	cbnz	x16,  <L16>
100043764:     	ldrb	w16, [x15, #0x2c]
100043768:     	tbnz	w16, #0x0,  <L16>
10004376c:     	strb	w14, [x15, #0x2c]
100043770:     	add	x9, x9, #0x1
100043774:     	b	 <L16>
<L25>:
100043778:     	neg	x11, x11
10004377c:     	cbz	x8,  <L29>
<L26>:
100043780:     	mov	w12, #0x30              ; =48
100043784:     	madd	x10, x11, x12, x10
100043788:     	add	x10, x10, #0x28
10004378c:     	mov	w11, #0x1               ; =1
100043790:     	b	 <L28>
<L27>:
100043794:     	subs	x8, x8, #0x1
100043798:     	b.eq	 <L29>
<L28>:
10004379c:     	ldr	x12, [x10], #0x30
1000437a0:     	ldr	x12, [x12]
1000437a4:     	ldr	x13, [x12, #0x30]
1000437a8:     	cbnz	x13,  <L27>
1000437ac:     	ldrb	w13, [x12, #0x2c]
1000437b0:     	tbnz	w13, #0x0,  <L27>
1000437b4:     	strb	w11, [x12, #0x2c]
1000437b8:     	add	x9, x9, #0x1
1000437bc:     	b	 <L27>
<L29>:
1000437c0:     	cmp	x9, x19
1000437c4:     	eor	w8, w20, #0x1
1000437c8:     	csinc	w8, w8, wzr, lo
1000437cc:     	tbnz	w8, #0x0,  <L36>
<L30>:
1000437d0:     	ldr	x19, [sp, #0x18]
1000437d4:     	cbz	x21,  <L34>
1000437d8:     	add	x20, x19, #0x18
1000437dc:     	mov	w22, #0x7f80            ; =32640
1000437e0:     	b	 <L33>
<L31>:
1000437e4:     	mov	w1, #0x1                ; =1
1000437e8:     	bl	 <_scoop_heap_publish_large_object>
1000437ec:     	ldur	x8, [x20, #-0x10]
1000437f0:     	str	x8, [x24, #0x78]
<L32>:
1000437f4:     	ldr	x8, [x23, #0x1d8]
1000437f8:     	add	x8, x8, #0x1
1000437fc:     	str	x8, [x23, #0x1d8]
100043800:     	add	x20, x20, #0x30
100043804:     	subs	x21, x21, #0x1
100043808:     	b.eq	 <L34>
<L33>:
10004380c:     	ldp	x1, x0, [x20, #-0x18]
100043810:     	ldur	x2, [x20, #-0x8]
100043814:     	bl	 <dyld_stub_binder+0x100049104>
100043818:     	ldur	x2, [x20, #-0x8]
10004381c:     	ldr	x8, [x23, #0x158]
100043820:     	add	x8, x8, x2
100043824:     	str	x8, [x23, #0x158]
100043828:     	ldp	x24, x0, [x20, #0x8]
10004382c:     	cmp	x2, x22
100043830:     	b.hi	 <L31>
100043834:     	ldur	x1, [x20, #-0x10]
100043838:     	mov	w3, #0x1                ; =1
10004383c:     	bl	 <_scoop_heap_record_small_object>
100043840:     	ldur	x8, [x20, #-0x10]
100043844:     	ldr	x9, [x24, #0x58]
100043848:     	ldr	x10, [x20]
10004384c:     	str	x8, [x9, x10, lsl #3]
100043850:     	b	 <L32>
<L34>:
100043854:     	mov	x0, x19
100043858:     	bl	 <dyld_stub_binder+0x10004908c>
<L35>:
10004385c:     	mov	w0, #0x1                ; =1
100043860:     	b	 <L43>
<L36>:
100043864:     	ldr	x0, [sp, #0x18]
100043868:     	bl	 <dyld_stub_binder+0x10004908c>
10004386c:     	bl	 <_scoop_heap_first_block>
100043870:     	cbz	x0,  <L40>
100043874:     	mov	w20, #0x2               ; =2
100043878:     	mov	w21, #0x5               ; =5
10004387c:     	b	 <L39>
<L37>:
100043880:     	mov	x19, x0
100043884:     	bl	 <_scoop_heap_block_has_pins>
100043888:     	cmp	w0, #0x0
10004388c:     	csel	w8, w21, w20, ne
100043890:     	str	w8, [x19, #0x14]
100043894:     	ldr	x0, [x19, #0x58]
100043898:     	bl	 <dyld_stub_binder+0x10004908c>
10004389c:     	mov	x0, x19
1000438a0:     	str	xzr, [x19, #0x58]
<L38>:
1000438a4:     	bl	 <_scoop_heap_next_block>
1000438a8:     	cbz	x0,  <L40>
<L39>:
1000438ac:     	ldr	w8, [x0, #0x14]
1000438b0:     	cmp	w8, #0x3
1000438b4:     	b.eq	 <L37>
1000438b8:     	cmp	w8, #0x4
1000438bc:     	b.ne	 <L38>
1000438c0:     	mov	x19, x0
1000438c4:     	bl	 <_scoop_heap_release_block>
1000438c8:     	mov	x0, x19
1000438cc:     	b	 <L38>
<L40>:
1000438d0:     	ldr	x8, [x23, #0x40]
1000438d4:     	cbz	x8,  <L42>
<L41>:
1000438d8:     	sturh	wzr, [x8, #0x2b]
1000438dc:     	ldr	x8, [x8, #0x18]
1000438e0:     	cbnz	x8,  <L41>
<L42>:
1000438e4:     	stp	xzr, xzr, [x23, #0x1c8]
1000438e8:     	str	xzr, [x23, #0x1c0]
1000438ec:     	ldrb	w8, [x23, #0x99]
1000438f0:     	cmp	w8, #0x1
1000438f4:     	b.eq	 <L45>
1000438f8:     	mov	w0, #0x0                ; =0
<L43>:
1000438fc:     	ldp	x29, x30, [sp, #0x60]
100043900:     	ldp	x20, x19, [sp, #0x50]
100043904:     	ldp	x22, x21, [sp, #0x40]
100043908:     	ldp	x24, x23, [sp, #0x30]
10004390c:     	ldp	x26, x25, [sp, #0x20]
100043910:     	add	sp, sp, #0x70
100043914:     	ret
<L44>:
100043918:     	adrp	x0, 0x100066000 <_check_release_hook.domain+0x731>
10004391c:     	add	x0, x0, #0xa76
100043920:     	bl	 <_scoop_heap_fatal>
<L45>:
100043924:     	adrp	x0, 0x100066000 <_check_release_hook.domain+0x731>
100043928:     	add	x0, x0, #0xa9c
10004392c:     	bl	 <_scoop_heap_fatal>
