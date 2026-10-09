000000010002d1ec <_scoop$1$bs$2c1181e985a870b945ab10fe130c8aa435e6278869edb19a0a4175072b0061e7>:
10002d1ec:     	stp	x20, x19, [sp, #-0x20]!
10002d1f0:     	stp	x29, x30, [sp, #0x10]
10002d1f4:     	add	x29, sp, #0x10
10002d1f8:     	mov	x19, x0
10002d1fc:     	ldp	x0, x1, [x1]
10002d200:     	bl	0x10004476c <_bench_pair>
10002d204:     	stp	x0, x1, [x19]
10002d208:     	ldp	x29, x30, [sp, #0x10]
10002d20c:     	ldp	x20, x19, [sp], #0x20
10002d210:     	ret


0000000100044cd4 <_run_worker>:
100044cd4:     	stp	x24, x23, [sp, #-0x40]!
100044cd8:     	stp	x22, x21, [sp, #0x10]
100044cdc:     	stp	x20, x19, [sp, #0x20]
100044ce0:     	stp	x29, x30, [sp, #0x30]
100044ce4:     	add	x29, sp, #0x30
100044ce8:     	adrp	x8, 0x1002db000 <_pauses+0xc3270>
100044cec:     	add	x8, x8, #0x2b4
100044cf0:     	mov	w9, #0x1                ; =1
100044cf4:     	ldaddl	w9, w8, [x8]
100044cf8:     	adrp	x19, 0x1002db000 <_pauses+0xc3270>
100044cfc:     	add	x19, x19, #0x2b8
100044d00:     	ldaprb	w8, [x19]
100044d04:     	tbnz	w8, #0x0, 0x100044d18 <_run_worker+0x44>
100044d08:     	bl	0x1000469d4 <dyld_stub_binder+0x1000469d4>
100044d0c:     	ldaprb	w8, [x19]
100044d10:     	cmp	w8, #0x1
100044d14:     	b.ne	0x100044d08 <_run_worker+0x34>
100044d18:     	adrp	x22, 0x100154000 <dyld_stub_binder+0x100154000>
100044d1c:     	ldr	w8, [x22, #0x870]
100044d20:     	cbz	w8, 0x100044dc4 <_run_worker+0xf0>
100044d24:     	cmp	w8, #0x3
100044d28:     	b.eq	0x100044d84 <_run_worker+0xb0>
100044d2c:     	cmp	w8, #0x6
100044d30:     	b.ne	0x100044e04 <_run_worker+0x130>
100044d34:     	adrp	x23, 0x1002db000 <_pauses+0xc3270>
100044d38:     	ldr	w8, [x23, #0x2b0]
100044d3c:     	cmp	w8, #0x1
100044d40:     	b.lt	0x100044dfc <_run_worker+0x128>
100044d44:     	mov	x20, #0x0               ; =0
100044d48:     	mov	x19, #0x0               ; =0
100044d4c:     	bl	0x100046794 <dyld_stub_binder+0x100046794>
100044d50:     	str	wzr, [x0]
100044d54:     	mov	x0, x20
100044d58:     	bl	0x100044764 <_bench_scalar>
100044d5c:     	mov	x21, x0
100044d60:     	bl	0x100046794 <dyld_stub_binder+0x100046794>
100044d64:     	ldrsw	x8, [x0]
100044d68:     	add	x9, x21, x19
100044d6c:     	add	x19, x9, x8
100044d70:     	add	x20, x20, #0x1
100044d74:     	ldrsw	x8, [x23, #0x2b0]
100044d78:     	cmp	x20, x8
100044d7c:     	b.lt	0x100044d4c <_run_worker+0x78>
100044d80:     	b	0x100044df8 <_run_worker+0x124>
100044d84:     	adrp	x21, 0x1002db000 <_pauses+0xc3270>
100044d88:     	ldr	w8, [x21, #0x2b0]
100044d8c:     	cmp	w8, #0x1
100044d90:     	b.lt	0x100044dfc <_run_worker+0x128>
100044d94:     	mov	x20, #0x0               ; =0
100044d98:     	mov	x19, #0x0               ; =0
100044d9c:     	mov	x0, x20
100044da0:     	mov	x1, x20
100044da4:     	bl	0x10004476c <_bench_pair>
100044da8:     	add	x8, x1, x19
100044dac:     	add	x19, x8, x0
100044db0:     	add	x20, x20, #0x1
100044db4:     	ldrsw	x8, [x21, #0x2b0]
100044db8:     	cmp	x20, x8
100044dbc:     	b.lt	0x100044d9c <_run_worker+0xc8>
100044dc0:     	b	0x100044df8 <_run_worker+0x124>
100044dc4:     	adrp	x21, 0x1002db000 <_pauses+0xc3270>
100044dc8:     	ldr	w8, [x21, #0x2b0]
100044dcc:     	cmp	w8, #0x1
100044dd0:     	b.lt	0x100044dfc <_run_worker+0x128>
100044dd4:     	mov	x20, #0x0               ; =0
100044dd8:     	mov	x19, #0x0               ; =0
100044ddc:     	mov	x0, x20
100044de0:     	bl	0x100044764 <_bench_scalar>
100044de4:     	add	x19, x0, x19
100044de8:     	add	x20, x20, #0x1
100044dec:     	ldrsw	x8, [x21, #0x2b0]
100044df0:     	cmp	x20, x8
100044df4:     	b.lt	0x100044ddc <_run_worker+0x108>
100044df8:     	b	0x100044e28 <_run_worker+0x154>
100044dfc:     	mov	x19, #0x0               ; =0
100044e00:     	b	0x100044e28 <_run_worker+0x154>
100044e04:     	adrp	x8, 0x1002db000 <_pauses+0xc3270>
100044e08:     	ldr	x8, [x8, #0x290]
100044e0c:     	adrp	x20, 0x1002db000 <_pauses+0xc3270>
100044e10:     	ldr	w0, [x20, #0x2b0]
100044e14:     	adrp	x9, 0x1002db000 <_pauses+0xc3270>
100044e18:     	ldr	x1, [x9, #0x298]
100044e1c:     	blr	x8
100044e20:     	mov	x19, x0
100044e24:     	ldr	w8, [x20, #0x2b0]
100044e28:     	sxtw	x8, w8
100044e2c:     	ldr	w9, [x22, #0x870]
100044e30:     	sub	w9, w9, #0x6
100044e34:     	cmn	w9, #0x4
100044e38:     	add	x9, x8, #0x2
100044e3c:     	mul	x9, x9, x8
100044e40:     	smaddl	x8, w8, w8, x8
100044e44:     	add	x8, x8, x8, lsr #63
100044e48:     	asr	x8, x8, #1
100044e4c:     	csel	x8, x9, x8, hi
100044e50:     	cmp	x19, x8
100044e54:     	b.ne	0x100044e80 <_run_worker+0x1ac>
100044e58:     	adrp	x8, 0x100154000 <dyld_stub_binder+0x100154000>
100044e5c:     	add	x8, x8, #0x874
100044e60:     	mov	w9, #0x1                ; =1
100044e64:     	ldaddl	w9, w8, [x8]
100044e68:     	mov	x0, #0x0                ; =0
100044e6c:     	ldp	x29, x30, [sp, #0x30]
100044e70:     	ldp	x20, x19, [sp, #0x20]
100044e74:     	ldp	x22, x21, [sp, #0x10]
100044e78:     	ldp	x24, x23, [sp], #0x40
100044e7c:     	ret
100044e80:     	bl	0x1000466cc <_run_worker.cold.1>


000000010002d34c <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49>:
10002d34c:     	sub	sp, sp, #0x60
10002d350:     	stp	x24, x23, [sp, #0x20]
10002d354:     	stp	x22, x21, [sp, #0x30]
10002d358:     	stp	x20, x19, [sp, #0x40]
10002d35c:     	stp	x29, x30, [sp, #0x50]
10002d360:     	add	x29, sp, #0x50
10002d364:     	adrp	x8, 0x100154000 <dyld_stub_binder+0x100154000>
10002d368:     	mov	w19, w0
10002d36c:     	add	x8, x8, #0x638
10002d370:     	ldr	x9, [x8]
10002d374:     	mov	x0, x8
10002d378:     	blr	x9
10002d37c:     	adrp	x21, 0x1002db000 <_pauses+0xc3270>
10002d380:     	adrp	x23, 0x1002db000 <_pauses+0xc3270>
10002d384:     	add	x21, x21, #0x2d8
10002d388:     	ldr	x22, [x0]
10002d38c:     	add	x23, x23, #0x2c4
10002d390:     	add	x10, x22, #0x8
10002d394:     	ldar	x8, [x21]
10002d398:     	ldar	w11, [x23]
10002d39c:     	ldar	w9, [x22]
10002d3a0:     	ldar	x10, [x10]
10002d3a4:     	cbnz	w11, 0x10002d3b4 <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x68>
10002d3a8:     	cmp	w9, #0x1
10002d3ac:     	ccmp	x10, x8, #0x0, eq
10002d3b0:     	b.eq	0x10002d3b8 <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x6c>
10002d3b4:     	bl	0x10003574c <_scoop_rt_safepoint>
10002d3b8:     	mov	x24, xzr
10002d3bc:     	mov	x20, xzr
10002d3c0:     	ldar	x8, [x21]
10002d3c4:     	ldar	w9, [x23]
10002d3c8:     	add	x11, x22, #0x8
10002d3cc:     	ldar	w10, [x22]
10002d3d0:     	ldar	x11, [x11]
10002d3d4:     	cmp	w9, #0x0
10002d3d8:     	ccmp	w10, #0x1, #0x0, eq
10002d3dc:     	ccmp	x11, x8, #0x0, eq
10002d3e0:     	b.eq	0x10002d3e8 <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x9c>
10002d3e4:     	bl	0x10003574c <_scoop_rt_safepoint>
10002d3e8:     	cmp	w24, w19
10002d3ec:     	b.ge	0x10002d414 <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0xc8>
10002d3f0:     	mov	x0, sp
10002d3f4:     	add	x1, sp, #0x10
10002d3f8:     	stp	x24, x24, [sp, #0x10]
10002d3fc:     	bl	0x10002d1ec <_scoop$1$bs$2c1181e985a870b945ab10fe130c8aa435e6278869edb19a0a4175072b0061e7>
10002d400:     	ldp	x8, x9, [sp]
10002d404:     	add	x24, x24, #0x1
10002d408:     	add	x8, x8, x9
10002d40c:     	add	x20, x20, x8
10002d410:     	b	0x10002d3c0 <_scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x74>
10002d414:     	mov	x0, x20
10002d418:     	ldp	x29, x30, [sp, #0x50]
10002d41c:     	ldp	x20, x19, [sp, #0x40]
10002d420:     	ldp	x22, x21, [sp, #0x30]
10002d424:     	ldp	x24, x23, [sp, #0x20]
10002d428:     	add	sp, sp, #0x60
10002d42c:     	ret


000000010002d95c <_scoop$1$cb$4802a025b776cfaa4b4d6f1276193dcff63e64f0f8cf99fc004b462585fe063a>:
10002d95c:     	sub	sp, sp, #0x30
10002d960:     	stp	x29, x30, [sp, #0x20]
10002d964:     	add	x29, sp, #0x20
10002d968:     	stp	x0, x1, [sp, #0x10]
10002d96c:     	mov	x0, sp
10002d970:     	add	x1, sp, #0x10
10002d974:     	bl	0x10002d1ec <_scoop$1$bs$2c1181e985a870b945ab10fe130c8aa435e6278869edb19a0a4175072b0061e7>
10002d978:     	ldp	x0, x1, [sp]
10002d97c:     	ldp	x29, x30, [sp, #0x20]
10002d980:     	add	sp, sp, #0x30
10002d984:     	ret


000000010002e300 <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e>:
10002e300:     	sub	sp, sp, #0xc0
10002e304:     	stp	x24, x23, [sp, #0x80]
10002e308:     	stp	x22, x21, [sp, #0x90]
10002e30c:     	stp	x20, x19, [sp, #0xa0]
10002e310:     	stp	x29, x30, [sp, #0xb0]
10002e314:     	add	x29, sp, #0xb0
10002e318:     	adrp	x8, 0x100154000 <dyld_stub_binder+0x100154000>
10002e31c:     	mov	w19, w0
10002e320:     	add	x8, x8, #0x638
10002e324:     	ldr	x9, [x8]
10002e328:     	mov	x0, x8
10002e32c:     	blr	x9
10002e330:     	adrp	x21, 0x1002db000 <_pauses+0xc3270>
10002e334:     	adrp	x23, 0x1002db000 <_pauses+0xc3270>
10002e338:     	add	x21, x21, #0x2d8
10002e33c:     	ldr	x22, [x0]
10002e340:     	add	x23, x23, #0x2c4
10002e344:     	add	x10, x22, #0x8
10002e348:     	ldar	x8, [x21]
10002e34c:     	ldar	w11, [x23]
10002e350:     	ldar	w9, [x22]
10002e354:     	ldar	x10, [x10]
10002e358:     	cbnz	w11, 0x10002e368 <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x68>
10002e35c:     	cmp	w9, #0x1
10002e360:     	ccmp	x10, x8, #0x0, eq
10002e364:     	b.eq	0x10002e36c <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x6c>
10002e368:     	bl	0x10003574c <_scoop_rt_safepoint>
10002e36c:     	mov	x24, xzr
10002e370:     	mov	x20, xzr
10002e374:     	ldar	x8, [x21]
10002e378:     	ldar	w9, [x23]
10002e37c:     	add	x11, x22, #0x8
10002e380:     	ldar	w10, [x22]
10002e384:     	ldar	x11, [x11]
10002e388:     	cmp	w9, #0x0
10002e38c:     	ccmp	w10, #0x1, #0x0, eq
10002e390:     	ccmp	x11, x8, #0x0, eq
10002e394:     	b.eq	0x10002e39c <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x9c>
10002e398:     	bl	0x10003574c <_scoop_rt_safepoint>
10002e39c:     	cmp	w24, w19
10002e3a0:     	b.ge	0x10002e40c <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x10c>
10002e3a4:     	movi.2d	v0, #0000000000000000
10002e3a8:     	add	x0, sp, #0x28
10002e3ac:     	mov	x1, xzr
10002e3b0:     	mov	x2, xzr
10002e3b4:     	stp	x24, x24, [sp, #0x18]
10002e3b8:     	str	xzr, [sp, #0x38]
10002e3bc:     	stur	q0, [sp, #0x28]
10002e3c0:     	bl	0x100040ea0 <_scoop_rt_push_caller_roots>
10002e3c4:     	movi.2d	v0, #0000000000000000
10002e3c8:     	add	x0, sp, #0x40
10002e3cc:     	add	x1, sp, #0x40
10002e3d0:     	stp	q0, q0, [sp, #0x40]
10002e3d4:     	stp	q0, q0, [sp, #0x60]
10002e3d8:     	bl	0x10003584c <_scoop_rt_enter_native_safe>
10002e3dc:     	add	x0, sp, #0x8
10002e3e0:     	add	x1, sp, #0x18
10002e3e4:     	bl	0x10002d1ec <_scoop$1$bs$2c1181e985a870b945ab10fe130c8aa435e6278869edb19a0a4175072b0061e7>
10002e3e8:     	add	x0, sp, #0x40
10002e3ec:     	bl	0x10003ffac <_scoop_rt_leave_native_safe>
10002e3f0:     	add	x0, sp, #0x28
10002e3f4:     	bl	0x100040f48 <_scoop_rt_pop_caller_roots>
10002e3f8:     	ldp	x8, x9, [sp, #0x8]
10002e3fc:     	add	x24, x24, #0x1
10002e400:     	add	x8, x8, x9
10002e404:     	add	x20, x20, x8
10002e408:     	b	0x10002e374 <_scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x74>
10002e40c:     	mov	x0, x20
10002e410:     	ldp	x29, x30, [sp, #0xb0]
10002e414:     	ldp	x20, x19, [sp, #0xa0]
10002e418:     	ldp	x22, x21, [sp, #0x90]
10002e41c:     	ldp	x24, x23, [sp, #0x80]
10002e420:     	add	sp, sp, #0xc0
10002e424:     	ret


000000010002e530 <_scoop$1$cb$3d4d2ef74f3e937ff2690df2a3f94a5b39561dbba24a97c4f84f983d86073fab>:
10002e530:     	sub	sp, sp, #0xa0
10002e534:     	stp	x20, x19, [sp, #0x80]
10002e538:     	stp	x29, x30, [sp, #0x90]
10002e53c:     	add	x29, sp, #0x90
10002e540:     	adrp	x8, 0x100154000 <dyld_stub_binder+0x100154000>
10002e544:     	mov	x19, x0
10002e548:     	mov	x20, x1
10002e54c:     	add	x8, x8, #0x638
10002e550:     	ldr	x9, [x8]
10002e554:     	mov	x0, x8
10002e558:     	blr	x9
10002e55c:     	adrp	x8, 0x1002db000 <_pauses+0xc3270>
10002e560:     	adrp	x10, 0x1002db000 <_pauses+0xc3270>
10002e564:     	add	x8, x8, #0x2d8
10002e568:     	ldr	x9, [x0]
10002e56c:     	add	x10, x10, #0x2c4
10002e570:     	ldar	x8, [x8]
10002e574:     	ldar	w11, [x10]
10002e578:     	add	x10, x9, #0x8
10002e57c:     	ldar	w9, [x9]
10002e580:     	ldar	x10, [x10]
10002e584:     	cbnz	w11, 0x10002e594 <_scoop$1$cb$3d4d2ef74f3e937ff2690df2a3f94a5b39561dbba24a97c4f84f983d86073fab+0x64>
10002e588:     	cmp	w9, #0x1
10002e58c:     	ccmp	x10, x8, #0x0, eq
10002e590:     	b.eq	0x10002e598 <_scoop$1$cb$3d4d2ef74f3e937ff2690df2a3f94a5b39561dbba24a97c4f84f983d86073fab+0x68>
10002e594:     	bl	0x10003574c <_scoop_rt_safepoint>
10002e598:     	movi.2d	v0, #0000000000000000
10002e59c:     	add	x0, sp, #0x28
10002e5a0:     	mov	x1, xzr
10002e5a4:     	mov	x2, xzr
10002e5a8:     	stp	x19, x20, [sp, #0x18]
10002e5ac:     	str	xzr, [sp, #0x38]
10002e5b0:     	stur	q0, [sp, #0x28]
10002e5b4:     	bl	0x100040ea0 <_scoop_rt_push_caller_roots>
10002e5b8:     	movi.2d	v0, #0000000000000000
10002e5bc:     	add	x0, sp, #0x40
10002e5c0:     	add	x1, sp, #0x40
10002e5c4:     	stp	q0, q0, [sp, #0x40]
10002e5c8:     	stp	q0, q0, [sp, #0x60]
10002e5cc:     	bl	0x10003584c <_scoop_rt_enter_native_safe>
10002e5d0:     	add	x0, sp, #0x8
10002e5d4:     	add	x1, sp, #0x18
10002e5d8:     	bl	0x10002d1ec <_scoop$1$bs$2c1181e985a870b945ab10fe130c8aa435e6278869edb19a0a4175072b0061e7>
10002e5dc:     	add	x0, sp, #0x40
10002e5e0:     	bl	0x10003ffac <_scoop_rt_leave_native_safe>
10002e5e4:     	add	x0, sp, #0x28
10002e5e8:     	bl	0x100040f48 <_scoop_rt_pop_caller_roots>
10002e5ec:     	ldp	x0, x1, [sp, #0x8]
10002e5f0:     	ldp	x29, x30, [sp, #0x90]
10002e5f4:     	ldp	x20, x19, [sp, #0x80]
10002e5f8:     	add	sp, sp, #0xa0
10002e5fc:     	ret


/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/direct-c-before-darwin/leaf.o:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000000000000 <ltmp0>:
       0:      	add	x0, x0, #0x1
       4:      	ret

0000000000000008 <_bench_pair>:
       8:      	add	x0, x0, #0x1
       c:      	add	x1, x1, #0x2
      10:      	ret
