000000010002d324 <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49>:
10002d324:     	stp	x24, x23, [sp, #-0x40]!
10002d328:     	stp	x22, x21, [sp, #0x10]
10002d32c:     	stp	x20, x19, [sp, #0x20]
10002d330:     	stp	x29, x30, [sp, #0x30]
10002d334:     	add	x29, sp, #0x30
10002d338:     	adrp	x8, 0x100154000 <dyld_stub_binder+0x100154000>
10002d33c:     	mov	w19, w0
10002d340:     	add	x8, x8, #0x638
10002d344:     	ldr	x9, [x8]
10002d348:     	mov	x0, x8
10002d34c:     	blr	x9
10002d350:     	adrp	x22, 0x1002db000 <_pauses+0xc3270>
10002d354:     	adrp	x24, 0x1002db000 <_pauses+0xc3270>
10002d358:     	add	x22, x22, #0x2d8
10002d35c:     	ldr	x23, [x0]
10002d360:     	add	x24, x24, #0x2c4
10002d364:     	add	x10, x23, #0x8
10002d368:     	ldar	x8, [x22]
10002d36c:     	ldar	w11, [x24]
10002d370:     	ldar	w9, [x23]
10002d374:     	ldar	x10, [x10]
10002d378:     	cbnz	w11, 0x10002d388 <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x64>
10002d37c:     	cmp	w9, #0x1
10002d380:     	ccmp	x10, x8, #0x0, eq
10002d384:     	b.eq	0x10002d38c <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x68>
10002d388:     	bl	0x10003570c <_scoop_rt_safepoint>
10002d38c:     	mov	x21, xzr
10002d390:     	mov	x20, xzr
10002d394:     	ldar	x8, [x22]
10002d398:     	ldar	w9, [x24]
10002d39c:     	add	x11, x23, #0x8
10002d3a0:     	ldar	w10, [x23]
10002d3a4:     	ldar	x11, [x11]
10002d3a8:     	cmp	w9, #0x0
10002d3ac:     	ccmp	w10, #0x1, #0x0, eq
10002d3b0:     	ccmp	x11, x8, #0x0, eq
10002d3b4:     	b.eq	0x10002d3bc <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x98>
10002d3b8:     	bl	0x10003570c <_scoop_rt_safepoint>
10002d3bc:     	cmp	w21, w19
10002d3c0:     	b.ge	0x10002d3e0 <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0xbc>
10002d3c4:     	mov	x0, x21
10002d3c8:     	mov	x1, x21
10002d3cc:     	bl	0x10004472c <_bench_pair>
10002d3d0:     	add	x8, x0, x1
10002d3d4:     	add	x21, x21, #0x1
10002d3d8:     	add	x20, x20, x8
10002d3dc:     	b	0x10002d394 <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x70>
10002d3e0:     	mov	x0, x20
10002d3e4:     	ldp	x29, x30, [sp, #0x30]
10002d3e8:     	ldp	x20, x19, [sp, #0x20]
10002d3ec:     	ldp	x22, x21, [sp, #0x10]
10002d3f0:     	ldp	x24, x23, [sp], #0x40
10002d3f4:     	ret


000000010002d924 <_scoop$1$cb$4802a025b776cfaa4b4d6f1276193dcff63e64f0f8cf99fc004b462585fe063a>:
10002d924:     	stp	x29, x30, [sp, #-0x10]!
10002d928:     	mov	x29, sp
10002d92c:     	bl	0x10004472c <_bench_pair>
10002d930:     	ldp	x29, x30, [sp], #0x10
10002d934:     	ret


000000010002e2b0 <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e>:
10002e2b0:     	sub	sp, sp, #0xb0
10002e2b4:     	stp	x26, x25, [sp, #0x60]
10002e2b8:     	stp	x24, x23, [sp, #0x70]
10002e2bc:     	stp	x22, x21, [sp, #0x80]
10002e2c0:     	stp	x20, x19, [sp, #0x90]
10002e2c4:     	stp	x29, x30, [sp, #0xa0]
10002e2c8:     	add	x29, sp, #0xa0
10002e2cc:     	adrp	x8, 0x100154000 <dyld_stub_binder+0x100154000>
10002e2d0:     	mov	w19, w0
10002e2d4:     	add	x8, x8, #0x638
10002e2d8:     	ldr	x9, [x8]
10002e2dc:     	mov	x0, x8
10002e2e0:     	blr	x9
10002e2e4:     	adrp	x24, 0x1002db000 <_pauses+0xc3270>
10002e2e8:     	adrp	x26, 0x1002db000 <_pauses+0xc3270>
10002e2ec:     	add	x24, x24, #0x2d8
10002e2f0:     	ldr	x25, [x0]
10002e2f4:     	add	x26, x26, #0x2c4
10002e2f8:     	add	x10, x25, #0x8
10002e2fc:     	ldar	x8, [x24]
10002e300:     	ldar	w11, [x26]
10002e304:     	ldar	w9, [x25]
10002e308:     	ldar	x10, [x10]
10002e30c:     	cbnz	w11, 0x10002e31c <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x6c>
10002e310:     	cmp	w9, #0x1
10002e314:     	ccmp	x10, x8, #0x0, eq
10002e318:     	b.eq	0x10002e320 <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x70>
10002e31c:     	bl	0x10003570c <_scoop_rt_safepoint>
10002e320:     	mov	x20, xzr
10002e324:     	mov	x21, xzr
10002e328:     	ldar	x8, [x24]
10002e32c:     	ldar	w9, [x26]
10002e330:     	add	x11, x25, #0x8
10002e334:     	ldar	w10, [x25]
10002e338:     	ldar	x11, [x11]
10002e33c:     	cmp	w9, #0x0
10002e340:     	ccmp	w10, #0x1, #0x0, eq
10002e344:     	ccmp	x11, x8, #0x0, eq
10002e348:     	b.eq	0x10002e350 <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0xa0>
10002e34c:     	bl	0x10003570c <_scoop_rt_safepoint>
10002e350:     	cmp	w20, w19
10002e354:     	b.ge	0x10002e3c0 <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x110>
10002e358:     	movi.2d	v0, #0000000000000000
10002e35c:     	add	x0, sp, #0x8
10002e360:     	mov	x1, xzr
10002e364:     	mov	x2, xzr
10002e368:     	str	xzr, [sp, #0x18]
10002e36c:     	stur	q0, [sp, #0x8]
10002e370:     	bl	0x100040e60 <_scoop_rt_push_caller_roots>
10002e374:     	movi.2d	v0, #0000000000000000
10002e378:     	add	x0, sp, #0x20
10002e37c:     	add	x1, sp, #0x20
10002e380:     	stp	q0, q0, [sp, #0x20]
10002e384:     	stp	q0, q0, [sp, #0x40]
10002e388:     	bl	0x10003580c <_scoop_rt_enter_native_safe>
10002e38c:     	mov	x0, x20
10002e390:     	mov	x1, x20
10002e394:     	bl	0x10004472c <_bench_pair>
10002e398:     	mov	x22, x0
10002e39c:     	add	x0, sp, #0x20
10002e3a0:     	mov	x23, x1
10002e3a4:     	bl	0x10003ff6c <_scoop_rt_leave_native_safe>
10002e3a8:     	add	x0, sp, #0x8
10002e3ac:     	bl	0x100040f08 <_scoop_rt_pop_caller_roots>
10002e3b0:     	add	x8, x22, x23
10002e3b4:     	add	x20, x20, #0x1
10002e3b8:     	add	x21, x21, x8
10002e3bc:     	b	0x10002e328 <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x78>
10002e3c0:     	mov	x0, x21
10002e3c4:     	ldp	x29, x30, [sp, #0xa0]
10002e3c8:     	ldp	x20, x19, [sp, #0x90]
10002e3cc:     	ldp	x22, x21, [sp, #0x80]
10002e3d0:     	ldp	x24, x23, [sp, #0x70]
10002e3d4:     	ldp	x26, x25, [sp, #0x60]
10002e3d8:     	add	sp, sp, #0xb0
10002e3dc:     	ret


000000010002e4e8 <_scoop$1$cb$3d4d2ef74f3e937ff2690df2a3f94a5b39561dbba24a97c4f84f983d86073fab>:
10002e4e8:     	sub	sp, sp, #0x80
10002e4ec:     	stp	x20, x19, [sp, #0x60]
10002e4f0:     	stp	x29, x30, [sp, #0x70]
10002e4f4:     	add	x29, sp, #0x70
10002e4f8:     	adrp	x8, 0x100154000 <dyld_stub_binder+0x100154000>
10002e4fc:     	mov	x19, x0
10002e500:     	mov	x20, x1
10002e504:     	add	x8, x8, #0x638
10002e508:     	ldr	x9, [x8]
10002e50c:     	mov	x0, x8
10002e510:     	blr	x9
10002e514:     	adrp	x8, 0x1002db000 <_pauses+0xc3270>
10002e518:     	adrp	x10, 0x1002db000 <_pauses+0xc3270>
10002e51c:     	add	x8, x8, #0x2d8
10002e520:     	ldr	x9, [x0]
10002e524:     	add	x10, x10, #0x2c4
10002e528:     	ldar	x8, [x8]
10002e52c:     	ldar	w11, [x10]
10002e530:     	add	x10, x9, #0x8
10002e534:     	ldar	w9, [x9]
10002e538:     	ldar	x10, [x10]
10002e53c:     	cbnz	w11, 0x10002e54c <_scoop$1$cb$3d4d2ef74f3e937ff2690df2a3f94a5b39561dbba24a97c4f84f983d86073fab+0x64>
10002e540:     	cmp	w9, #0x1
10002e544:     	ccmp	x10, x8, #0x0, eq
10002e548:     	b.eq	0x10002e550 <_scoop$1$cb$3d4d2ef74f3e937ff2690df2a3f94a5b39561dbba24a97c4f84f983d86073fab+0x68>
10002e54c:     	bl	0x10003570c <_scoop_rt_safepoint>
10002e550:     	movi.2d	v0, #0000000000000000
10002e554:     	add	x0, sp, #0x8
10002e558:     	mov	x1, xzr
10002e55c:     	mov	x2, xzr
10002e560:     	str	xzr, [sp, #0x18]
10002e564:     	stur	q0, [sp, #0x8]
10002e568:     	bl	0x100040e60 <_scoop_rt_push_caller_roots>
10002e56c:     	movi.2d	v0, #0000000000000000
10002e570:     	add	x0, sp, #0x20
10002e574:     	add	x1, sp, #0x20
10002e578:     	stp	q0, q0, [sp, #0x20]
10002e57c:     	stp	q0, q0, [sp, #0x40]
10002e580:     	bl	0x10003580c <_scoop_rt_enter_native_safe>
10002e584:     	mov	x0, x19
10002e588:     	mov	x1, x20
10002e58c:     	bl	0x10004472c <_bench_pair>
10002e590:     	mov	x19, x0
10002e594:     	add	x0, sp, #0x20
10002e598:     	mov	x20, x1
10002e59c:     	bl	0x10003ff6c <_scoop_rt_leave_native_safe>
10002e5a0:     	add	x0, sp, #0x8
10002e5a4:     	bl	0x100040f08 <_scoop_rt_pop_caller_roots>
10002e5a8:     	mov	x0, x19
10002e5ac:     	mov	x1, x20
10002e5b0:     	ldp	x29, x30, [sp, #0x70]
10002e5b4:     	ldp	x20, x19, [sp, #0x60]
10002e5b8:     	add	sp, sp, #0x80
10002e5bc:     	ret


0000000100044c94 <_run_worker>:
100044c94:     	stp	x24, x23, [sp, #-0x40]!
100044c98:     	stp	x22, x21, [sp, #0x10]
100044c9c:     	stp	x20, x19, [sp, #0x20]
100044ca0:     	stp	x29, x30, [sp, #0x30]
100044ca4:     	add	x29, sp, #0x30
100044ca8:     	adrp	x8, 0x1002db000 <_pauses+0xc3270>
100044cac:     	add	x8, x8, #0x2b4
100044cb0:     	mov	w9, #0x1                ; =1
100044cb4:     	ldaddl	w9, w8, [x8]
100044cb8:     	adrp	x19, 0x1002db000 <_pauses+0xc3270>
100044cbc:     	add	x19, x19, #0x2b8
100044cc0:     	ldaprb	w8, [x19]
100044cc4:     	tbnz	w8, #0x0, 0x100044cd8 <_run_worker+0x44>
100044cc8:     	bl	0x100046994 <dyld_stub_binder+0x100046994>
100044ccc:     	ldaprb	w8, [x19]
100044cd0:     	cmp	w8, #0x1
100044cd4:     	b.ne	0x100044cc8 <_run_worker+0x34>
100044cd8:     	adrp	x22, 0x100154000 <dyld_stub_binder+0x100154000>
100044cdc:     	ldr	w8, [x22, #0x870]
100044ce0:     	cbz	w8, 0x100044d84 <_run_worker+0xf0>
100044ce4:     	cmp	w8, #0x3
100044ce8:     	b.eq	0x100044d44 <_run_worker+0xb0>
100044cec:     	cmp	w8, #0x6
100044cf0:     	b.ne	0x100044dc4 <_run_worker+0x130>
100044cf4:     	adrp	x23, 0x1002db000 <_pauses+0xc3270>
100044cf8:     	ldr	w8, [x23, #0x2b0]
100044cfc:     	cmp	w8, #0x1
100044d00:     	b.lt	0x100044dbc <_run_worker+0x128>
100044d04:     	mov	x20, #0x0               ; =0
100044d08:     	mov	x19, #0x0               ; =0
100044d0c:     	bl	0x100046754 <dyld_stub_binder+0x100046754>
100044d10:     	str	wzr, [x0]
100044d14:     	mov	x0, x20
100044d18:     	bl	0x100044724 <_bench_scalar>
100044d1c:     	mov	x21, x0
100044d20:     	bl	0x100046754 <dyld_stub_binder+0x100046754>
100044d24:     	ldrsw	x8, [x0]
100044d28:     	add	x9, x21, x19
100044d2c:     	add	x19, x9, x8
100044d30:     	add	x20, x20, #0x1
100044d34:     	ldrsw	x8, [x23, #0x2b0]
100044d38:     	cmp	x20, x8
100044d3c:     	b.lt	0x100044d0c <_run_worker+0x78>
100044d40:     	b	0x100044db8 <_run_worker+0x124>
100044d44:     	adrp	x21, 0x1002db000 <_pauses+0xc3270>
100044d48:     	ldr	w8, [x21, #0x2b0]
100044d4c:     	cmp	w8, #0x1
100044d50:     	b.lt	0x100044dbc <_run_worker+0x128>
100044d54:     	mov	x20, #0x0               ; =0
100044d58:     	mov	x19, #0x0               ; =0
100044d5c:     	mov	x0, x20
100044d60:     	mov	x1, x20
100044d64:     	bl	0x10004472c <_bench_pair>
100044d68:     	add	x8, x1, x19
100044d6c:     	add	x19, x8, x0
100044d70:     	add	x20, x20, #0x1
100044d74:     	ldrsw	x8, [x21, #0x2b0]
100044d78:     	cmp	x20, x8
100044d7c:     	b.lt	0x100044d5c <_run_worker+0xc8>
100044d80:     	b	0x100044db8 <_run_worker+0x124>
100044d84:     	adrp	x21, 0x1002db000 <_pauses+0xc3270>
100044d88:     	ldr	w8, [x21, #0x2b0]
100044d8c:     	cmp	w8, #0x1
100044d90:     	b.lt	0x100044dbc <_run_worker+0x128>
100044d94:     	mov	x20, #0x0               ; =0
100044d98:     	mov	x19, #0x0               ; =0
100044d9c:     	mov	x0, x20
100044da0:     	bl	0x100044724 <_bench_scalar>
100044da4:     	add	x19, x0, x19
100044da8:     	add	x20, x20, #0x1
100044dac:     	ldrsw	x8, [x21, #0x2b0]
100044db0:     	cmp	x20, x8
100044db4:     	b.lt	0x100044d9c <_run_worker+0x108>
100044db8:     	b	0x100044de8 <_run_worker+0x154>
100044dbc:     	mov	x19, #0x0               ; =0
100044dc0:     	b	0x100044de8 <_run_worker+0x154>
100044dc4:     	adrp	x8, 0x1002db000 <_pauses+0xc3270>
100044dc8:     	ldr	x8, [x8, #0x290]
100044dcc:     	adrp	x20, 0x1002db000 <_pauses+0xc3270>
100044dd0:     	ldr	w0, [x20, #0x2b0]
100044dd4:     	adrp	x9, 0x1002db000 <_pauses+0xc3270>
100044dd8:     	ldr	x1, [x9, #0x298]
100044ddc:     	blr	x8
100044de0:     	mov	x19, x0
100044de4:     	ldr	w8, [x20, #0x2b0]
100044de8:     	sxtw	x8, w8
100044dec:     	ldr	w9, [x22, #0x870]
100044df0:     	sub	w9, w9, #0x6
100044df4:     	cmn	w9, #0x4
100044df8:     	add	x9, x8, #0x2
100044dfc:     	mul	x9, x9, x8
100044e00:     	smaddl	x8, w8, w8, x8
100044e04:     	add	x8, x8, x8, lsr #63
100044e08:     	asr	x8, x8, #1
100044e0c:     	csel	x8, x9, x8, hi
100044e10:     	cmp	x19, x8
100044e14:     	b.ne	0x100044e40 <_run_worker+0x1ac>
100044e18:     	adrp	x8, 0x100154000 <dyld_stub_binder+0x100154000>
100044e1c:     	add	x8, x8, #0x874
100044e20:     	mov	w9, #0x1                ; =1
100044e24:     	ldaddl	w9, w8, [x8]
100044e28:     	mov	x0, #0x0                ; =0
100044e2c:     	ldp	x29, x30, [sp, #0x30]
100044e30:     	ldp	x20, x19, [sp, #0x20]
100044e34:     	ldp	x22, x21, [sp, #0x10]
100044e38:     	ldp	x24, x23, [sp], #0x40
100044e3c:     	ret
100044e40:     	bl	0x10004668c <_run_worker.cold.1>


/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/direct-c-after-darwin/leaf.o:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000000000000 <ltmp0>:
       0:      	add	x0, x0, #0x1
       4:      	ret

0000000000000008 <_bench_pair>:
       8:      	add	x0, x0, #0x1
       c:      	add	x1, x1, #0x2
      10:      	ret
