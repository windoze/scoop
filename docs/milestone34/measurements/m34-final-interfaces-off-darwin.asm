; Cell.next, MIR fn0, LIR scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001db40 <_scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0>:
10001db40: d100c3ff    	sub	sp, sp, #0x30
10001db44: a9014ff4    	stp	x20, x19, [sp, #0x10]
10001db48: a9027bfd    	stp	x29, x30, [sp, #0x20]
10001db4c: 910083fd    	add	x29, sp, #0x20
10001db50: 2a0103f3    	mov	w19, w1
10001db54: f90003e0    	str	x0, [sp]
10001db58: 94001cc5    	bl	0x100024e6c <_scoop_rt_safepoint>
10001db5c: f94003e0    	ldr	x0, [sp]
10001db60: f90007e0    	str	x0, [sp, #0x8]
10001db64: 94000175    	bl	0x10001e138 <_scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be>
10001db68: f94003e8    	ldr	x8, [sp]
10001db6c: 4a133409    	eor	w9, w0, w19, lsl #13
10001db70: f90007e8    	str	x8, [sp, #0x8]
10001db74: a9427bfd    	ldp	x29, x30, [sp, #0x20]
10001db78: 4a130129    	eor	w9, w9, w19
10001db7c: a9414ff4    	ldp	x20, x19, [sp, #0x10]
10001db80: 4a494529    	eor	w9, w9, w9, lsr #17
10001db84: 4a091520    	eor	w0, w9, w9, lsl #5
10001db88: 9100c3ff    	add	sp, sp, #0x30
10001db8c: d65f03c0    	ret

; Cell.$get$salt, MIR fn1, LIR scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001e138 <_scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be>:
10001e138: d10083ff    	sub	sp, sp, #0x20
10001e13c: a9017bfd    	stp	x29, x30, [sp, #0x10]
10001e140: 910043fd    	add	x29, sp, #0x10
10001e144: f90003e0    	str	x0, [sp]
10001e148: 94001b49    	bl	0x100024e6c <_scoop_rt_safepoint>
10001e14c: f94003e8    	ldr	x8, [sp]
10001e150: f90007e8    	str	x8, [sp, #0x8]
10001e154: a9417bfd    	ldp	x29, x30, [sp, #0x10]
10001e158: b9401100    	ldr	w0, [x8, #0x10]
10001e15c: 910083ff    	add	sp, sp, #0x20
10001e160: d65f03c0    	ret

; known, MIR fn2, LIR scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001e06c <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb>:
10001e06c: d10183ff    	sub	sp, sp, #0x60
10001e070: a90357f6    	stp	x22, x21, [sp, #0x30]
10001e074: a9044ff4    	stp	x20, x19, [sp, #0x40]
10001e078: a9057bfd    	stp	x29, x30, [sp, #0x50]
10001e07c: 910143fd    	add	x29, sp, #0x50
10001e080: 2a0103f3    	mov	w19, w1
10001e084: f9000be0    	str	x0, [sp, #0x10]
10001e088: 94001b79    	bl	0x100024e6c <_scoop_rt_safepoint>
10001e08c: f9400be8    	ldr	x8, [sp, #0x10]
10001e090: f90017e8    	str	x8, [sp, #0x28]
10001e094: f00007f4    	adrp	x20, 0x10011d000 <_scoop$1$sr$662dbe2599cfdfe26f30acca15dce5b7ec1172c946d91e0d4b2515764e9b79e0+0x20>
10001e098: 528acf15    	mov	w21, #0x5678            ; =22136
10001e09c: 91064294    	add	x20, x20, #0x190
10001e0a0: 2a1f03f6    	mov	w22, wzr
10001e0a4: 72a24695    	movk	w21, #0x1234, lsl #16
10001e0a8: f90013e8    	str	x8, [sp, #0x20]
10001e0ac: f94013e8    	ldr	x8, [sp, #0x20]
10001e0b0: f9000be8    	str	x8, [sp, #0x10]
10001e0b4: 94001b6e    	bl	0x100024e6c <_scoop_rt_safepoint>
10001e0b8: f9400be8    	ldr	x8, [sp, #0x10]
10001e0bc: 6b1302df    	cmp	w22, w19
10001e0c0: f90013e8    	str	x8, [sp, #0x20]
10001e0c4: 5400022a    	b.ge	0x10001e108 <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x9c>
10001e0c8: f9000fe8    	str	x8, [sp, #0x18]
10001e0cc: aa1403e1    	mov	x1, x20
10001e0d0: f9400100    	ldr	x0, [x8]
10001e0d4: 940011c7    	bl	0x1000227f0 <_scoop_rt_itable_lookup>
10001e0d8: a941a7e8    	ldp	x8, x9, [sp, #0x18]
10001e0dc: 2a1503e1    	mov	w1, w21
10001e0e0: f940000a    	ldr	x10, [x0]
10001e0e4: aa0803e0    	mov	x0, x8
10001e0e8: a900a7e8    	stp	x8, x9, [sp, #0x8]
10001e0ec: d63f0140    	blr	x10
10001e0f0: a940a3e9    	ldp	x9, x8, [sp, #0x8]
10001e0f4: 2a0003f5    	mov	w21, w0
10001e0f8: f90013e8    	str	x8, [sp, #0x20]
10001e0fc: f9000fe9    	str	x9, [sp, #0x18]
10001e100: 110006d6    	add	w22, w22, #0x1
10001e104: 17ffffea    	b	0x10001e0ac <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x40>
10001e108: 2a1503e0    	mov	w0, w21
10001e10c: a9457bfd    	ldp	x29, x30, [sp, #0x50]
10001e110: a9444ff4    	ldp	x20, x19, [sp, #0x40]
10001e114: a94357f6    	ldp	x22, x21, [sp, #0x30]
10001e118: 910183ff    	add	sp, sp, #0x60
10001e11c: d65f03c0    	ret

; unknown, MIR fn3, LIR scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001dfbc <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0>:
10001dfbc: d10143ff    	sub	sp, sp, #0x50
10001dfc0: a90257f6    	stp	x22, x21, [sp, #0x20]
10001dfc4: a9034ff4    	stp	x20, x19, [sp, #0x30]
10001dfc8: a9047bfd    	stp	x29, x30, [sp, #0x40]
10001dfcc: 910103fd    	add	x29, sp, #0x40
10001dfd0: 2a0103f3    	mov	w19, w1
10001dfd4: f90007e0    	str	x0, [sp, #0x8]
10001dfd8: 94001ba5    	bl	0x100024e6c <_scoop_rt_safepoint>
10001dfdc: f94007e8    	ldr	x8, [sp, #0x8]
10001dfe0: 90000814    	adrp	x20, 0x10011d000 <_scoop$1$sr$662dbe2599cfdfe26f30acca15dce5b7ec1172c946d91e0d4b2515764e9b79e0+0x20>
10001dfe4: 528acf15    	mov	w21, #0x5678            ; =22136
10001dfe8: 2a1f03f6    	mov	w22, wzr
10001dfec: 91064294    	add	x20, x20, #0x190
10001dff0: 72a24695    	movk	w21, #0x1234, lsl #16
10001dff4: f9000fe8    	str	x8, [sp, #0x18]
10001dff8: f9400fe8    	ldr	x8, [sp, #0x18]
10001dffc: f90007e8    	str	x8, [sp, #0x8]
10001e000: 94001b9b    	bl	0x100024e6c <_scoop_rt_safepoint>
10001e004: f94007e8    	ldr	x8, [sp, #0x8]
10001e008: 6b1302df    	cmp	w22, w19
10001e00c: f9000fe8    	str	x8, [sp, #0x18]
10001e010: 5400022a    	b.ge	0x10001e054 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x98>
10001e014: f9000be8    	str	x8, [sp, #0x10]
10001e018: aa1403e1    	mov	x1, x20
10001e01c: f9400100    	ldr	x0, [x8]
10001e020: 940011f4    	bl	0x1000227f0 <_scoop_rt_itable_lookup>
10001e024: a94127e8    	ldp	x8, x9, [sp, #0x10]
10001e028: 2a1503e1    	mov	w1, w21
10001e02c: f940000a    	ldr	x10, [x0]
10001e030: aa0803e0    	mov	x0, x8
10001e034: a90027e8    	stp	x8, x9, [sp]
10001e038: d63f0140    	blr	x10
10001e03c: a94023e9    	ldp	x9, x8, [sp]
10001e040: 2a0003f5    	mov	w21, w0
10001e044: f9000fe8    	str	x8, [sp, #0x18]
10001e048: f9000be9    	str	x9, [sp, #0x10]
10001e04c: 110006d6    	add	w22, w22, #0x1
10001e050: 17ffffea    	b	0x10001dff8 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x3c>
10001e054: 2a1503e0    	mov	w0, w21
10001e058: a9447bfd    	ldp	x29, x30, [sp, #0x40]
10001e05c: a9434ff4    	ldp	x20, x19, [sp, #0x30]
10001e060: a94257f6    	ldp	x22, x21, [sp, #0x20]
10001e064: 910143ff    	add	sp, sp, #0x50
10001e068: d65f03c0    	ret

; convertedOnce, MIR fn4, LIR scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001d68c <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd>:
10001d68c: d10183ff    	sub	sp, sp, #0x60
10001d690: a90357f6    	stp	x22, x21, [sp, #0x30]
10001d694: a9044ff4    	stp	x20, x19, [sp, #0x40]
10001d698: a9057bfd    	stp	x29, x30, [sp, #0x50]
10001d69c: 910143fd    	add	x29, sp, #0x50
10001d6a0: 2a0103f3    	mov	w19, w1
10001d6a4: f90007e0    	str	x0, [sp, #0x8]
10001d6a8: 94001df1    	bl	0x100024e6c <_scoop_rt_safepoint>
10001d6ac: f94007f5    	ldr	x21, [sp, #0x8]
10001d6b0: f90017f5    	str	x21, [sp, #0x28]
10001d6b4: 90000801    	adrp	x1, 0x10011d000 <_scoop$1$sr$662dbe2599cfdfe26f30acca15dce5b7ec1172c946d91e0d4b2515764e9b79e0+0x20>
10001d6b8: aa1503e0    	mov	x0, x21
10001d6bc: 91064021    	add	x1, x1, #0x190
10001d6c0: 940013e6    	bl	0x100022658 <_scoop_rt_is_instance>
10001d6c4: 36000480    	tbz	w0, #0x0, 0x10001d754 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xc8>
10001d6c8: 90000814    	adrp	x20, 0x10011d000 <_scoop$1$sr$662dbe2599cfdfe26f30acca15dce5b7ec1172c946d91e0d4b2515764e9b79e0+0x20>
10001d6cc: 2a1f03f6    	mov	w22, wzr
10001d6d0: 91064294    	add	x20, x20, #0x190
10001d6d4: f9000ff5    	str	x21, [sp, #0x18]
10001d6d8: 528acf15    	mov	w21, #0x5678            ; =22136
10001d6dc: 72a24695    	movk	w21, #0x1234, lsl #16
10001d6e0: f9400fe8    	ldr	x8, [sp, #0x18]
10001d6e4: f90007e8    	str	x8, [sp, #0x8]
10001d6e8: 94001de1    	bl	0x100024e6c <_scoop_rt_safepoint>
10001d6ec: f94007e8    	ldr	x8, [sp, #0x8]
10001d6f0: 6b1302df    	cmp	w22, w19
10001d6f4: f9000fe8    	str	x8, [sp, #0x18]
10001d6f8: 5400022a    	b.ge	0x10001d73c <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xb0>
10001d6fc: f9000be8    	str	x8, [sp, #0x10]
10001d700: aa1403e1    	mov	x1, x20
10001d704: f9400100    	ldr	x0, [x8]
10001d708: 9400143a    	bl	0x1000227f0 <_scoop_rt_itable_lookup>
10001d70c: a94127e8    	ldp	x8, x9, [sp, #0x10]
10001d710: 2a1503e1    	mov	w1, w21
10001d714: f940000a    	ldr	x10, [x0]
10001d718: aa0803e0    	mov	x0, x8
10001d71c: a90027e8    	stp	x8, x9, [sp]
10001d720: d63f0140    	blr	x10
10001d724: a94023e9    	ldp	x9, x8, [sp]
10001d728: 2a0003f5    	mov	w21, w0
10001d72c: f9000fe8    	str	x8, [sp, #0x18]
10001d730: f9000be9    	str	x9, [sp, #0x10]
10001d734: 110006d6    	add	w22, w22, #0x1
10001d738: 17ffffea    	b	0x10001d6e0 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x54>
10001d73c: 2a1503e0    	mov	w0, w21
10001d740: a9457bfd    	ldp	x29, x30, [sp, #0x50]
10001d744: a9444ff4    	ldp	x20, x19, [sp, #0x40]
10001d748: a94357f6    	ldp	x22, x21, [sp, #0x30]
10001d74c: 910183ff    	add	sp, sp, #0x60
10001d750: d65f03c0    	ret
10001d754: f0000960    	adrp	x0, 0x10014c000 <dyld_stub_binder+0x10014c000>
10001d758: d0000269    	adrp	x9, 0x10006b000 <_scoop$1$td$bb6274a8ad942aaa9e497e7cf9cedec226cd43a70f4602c59619ea3af8ce965e+0x50>
10001d75c: 9117a000    	add	x0, x0, #0x5e8
10001d760: 91294129    	add	x9, x9, #0xa50
10001d764: f9400008    	ldr	x8, [x0]
10001d768: f9400d2a    	ldr	x10, [x9, #0x18]
10001d76c: d63f0100    	blr	x8
10001d770: f9400008    	ldr	x8, [x0]
10001d774: cb0a03ed    	neg	x13, x10
10001d778: 91005d4e    	add	x14, x10, #0x17
10001d77c: 8a0d01c2    	and	x2, x14, x13
10001d780: a940310b    	ldp	x11, x12, [x8]
10001d784: 8b0b014b    	add	x11, x10, x11
10001d788: d100056b    	sub	x11, x11, #0x1
10001d78c: 8a0d0173    	and	x19, x11, x13
10001d790: f940492b    	ldr	x11, [x9, #0x90]
10001d794: 528ff009    	mov	w9, #0x7f80             ; =32640
10001d798: f100027f    	cmp	x19, #0x0
10001d79c: 92f0000d    	mov	x13, #0x7fffffffffffffff ; =9223372036854775807
10001d7a0: fa491042    	ccmp	x2, x9, #0x2, ne
10001d7a4: 8b020269    	add	x9, x19, x2
10001d7a8: 8b0d014a    	add	x10, x10, x13
10001d7ac: fa409960    	ccmp	x11, #0x0, #0x0, ls
10001d7b0: fa4c0122    	ccmp	x9, x12, #0x2, eq
10001d7b4: ba589942    	ccmn	x10, #0x18, #0x2, ls
10001d7b8: 540000e3    	b.lo	0x10001d7d4 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x148>
10001d7bc: d0000260    	adrp	x0, 0x10006b000 <_scoop$1$td$bb6274a8ad942aaa9e497e7cf9cedec226cd43a70f4602c59619ea3af8ce965e+0x50>
10001d7c0: 52800301    	mov	w1, #0x18               ; =24
10001d7c4: 91294000    	add	x0, x0, #0xa50
10001d7c8: 94001dad    	bl	0x100024e7c <_scoop_runtime_alloc_slow>
10001d7cc: aa0003f3    	mov	x19, x0
10001d7d0: 14000006    	b	0x10001d7e8 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x15c>
10001d7d4: f9000109    	str	x9, [x8]
10001d7d8: d0000261    	adrp	x1, 0x10006b000 <_scoop$1$td$bb6274a8ad942aaa9e497e7cf9cedec226cd43a70f4602c59619ea3af8ce965e+0x50>
10001d7dc: aa1303e0    	mov	x0, x19
10001d7e0: 91294021    	add	x1, x1, #0xa50
10001d7e4: 940040b9    	bl	0x10002dac8 <_scoop_runtime_finish_tlab_alloc>
10001d7e8: aa1303e0    	mov	x0, x19
10001d7ec: f90007f3    	str	x19, [sp, #0x8]
10001d7f0: 97ff9e4a    	bl	0x100005118 <_scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
10001d7f4: f94007e0    	ldr	x0, [sp, #0x8]
10001d7f8: f90013e0    	str	x0, [sp, #0x20]
10001d7fc: 940022c9    	bl	0x100026320 <_scoop_rt_throw>

; convertedEach, MIR fn5, LIR scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001de38 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b>:
10001de38: d101c3ff    	sub	sp, sp, #0x70
10001de3c: a9035ff8    	stp	x24, x23, [sp, #0x30]
10001de40: a90457f6    	stp	x22, x21, [sp, #0x40]
10001de44: a9054ff4    	stp	x20, x19, [sp, #0x50]
10001de48: a9067bfd    	stp	x29, x30, [sp, #0x60]
10001de4c: 910183fd    	add	x29, sp, #0x60
10001de50: 2a0103f3    	mov	w19, w1
10001de54: f9000be0    	str	x0, [sp, #0x10]
10001de58: 94001c05    	bl	0x100024e6c <_scoop_rt_safepoint>
10001de5c: f9400be8    	ldr	x8, [sp, #0x10]
10001de60: 90000814    	adrp	x20, 0x10011d000 <_scoop$1$sr$662dbe2599cfdfe26f30acca15dce5b7ec1172c946d91e0d4b2515764e9b79e0+0x20>
10001de64: 528acf15    	mov	w21, #0x5678            ; =22136
10001de68: 2a1f03f7    	mov	w23, wzr
10001de6c: 91064294    	add	x20, x20, #0x190
10001de70: 72a24695    	movk	w21, #0x1234, lsl #16
10001de74: f90017e8    	str	x8, [sp, #0x28]
10001de78: f94017e8    	ldr	x8, [sp, #0x28]
10001de7c: f9000be8    	str	x8, [sp, #0x10]
10001de80: 94001bfb    	bl	0x100024e6c <_scoop_rt_safepoint>
10001de84: f9400bf6    	ldr	x22, [sp, #0x10]
10001de88: 6b1302ff    	cmp	w23, w19
10001de8c: f90017f6    	str	x22, [sp, #0x28]
10001de90: 540002ca    	b.ge	0x10001dee8 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xb0>
10001de94: aa1603e0    	mov	x0, x22
10001de98: aa1403e1    	mov	x1, x20
10001de9c: 940011ef    	bl	0x100022658 <_scoop_rt_is_instance>
10001dea0: 36000320    	tbz	w0, #0x0, 0x10001df04 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xcc>
10001dea4: f9000ff6    	str	x22, [sp, #0x18]
10001dea8: aa1403e1    	mov	x1, x20
10001deac: f94002c0    	ldr	x0, [x22]
10001deb0: 94001250    	bl	0x1000227f0 <_scoop_rt_itable_lookup>
10001deb4: f9400fe8    	ldr	x8, [sp, #0x18]
10001deb8: f94017e9    	ldr	x9, [sp, #0x28]
10001debc: 2a1503e1    	mov	w1, w21
10001dec0: f940000a    	ldr	x10, [x0]
10001dec4: aa0803e0    	mov	x0, x8
10001dec8: a900a7e8    	stp	x8, x9, [sp, #0x8]
10001decc: d63f0140    	blr	x10
10001ded0: a940a3e9    	ldp	x9, x8, [sp, #0x8]
10001ded4: 2a0003f5    	mov	w21, w0
10001ded8: f90017e8    	str	x8, [sp, #0x28]
10001dedc: f9000fe9    	str	x9, [sp, #0x18]
10001dee0: 110006f7    	add	w23, w23, #0x1
10001dee4: 17ffffe5    	b	0x10001de78 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x40>
10001dee8: 2a1503e0    	mov	w0, w21
10001deec: a9467bfd    	ldp	x29, x30, [sp, #0x60]
10001def0: a9454ff4    	ldp	x20, x19, [sp, #0x50]
10001def4: a94457f6    	ldp	x22, x21, [sp, #0x40]
10001def8: a9435ff8    	ldp	x24, x23, [sp, #0x30]
10001defc: 9101c3ff    	add	sp, sp, #0x70
10001df00: d65f03c0    	ret
10001df04: f0000960    	adrp	x0, 0x10014c000 <dyld_stub_binder+0x10014c000>
10001df08: d000026a    	adrp	x10, 0x10006b000 <_scoop$1$td$bb6274a8ad942aaa9e497e7cf9cedec226cd43a70f4602c59619ea3af8ce965e+0x50>
10001df0c: 9117a000    	add	x0, x0, #0x5e8
10001df10: 9129414a    	add	x10, x10, #0xa50
10001df14: f9400008    	ldr	x8, [x0]
10001df18: f9400d49    	ldr	x9, [x10, #0x18]
10001df1c: d63f0100    	blr	x8
10001df20: f9400008    	ldr	x8, [x0]
10001df24: cb0903eb    	neg	x11, x9
10001df28: f940010c    	ldr	x12, [x8]
10001df2c: 8b0c012c    	add	x12, x9, x12
10001df30: d100058c    	sub	x12, x12, #0x1
10001df34: ea0b0193    	ands	x19, x12, x11
10001df38: 540002c0    	b.eq	0x10001df90 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x158>
10001df3c: 91005d2c    	add	x12, x9, #0x17
10001df40: 8a0b0182    	and	x2, x12, x11
10001df44: 528ff00b    	mov	w11, #0x7f80            ; =32640
10001df48: eb0b005f    	cmp	x2, x11
10001df4c: 54000228    	b.hi	0x10001df90 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x158>
10001df50: f940494a    	ldr	x10, [x10, #0x90]
10001df54: b50001ea    	cbnz	x10, 0x10001df90 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x158>
10001df58: f940050b    	ldr	x11, [x8, #0x8]
10001df5c: 8b02026a    	add	x10, x19, x2
10001df60: eb0b015f    	cmp	x10, x11
10001df64: 54000168    	b.hi	0x10001df90 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x158>
10001df68: 92f0000b    	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10001df6c: 8b0b0129    	add	x9, x9, x11
10001df70: b100613f    	cmn	x9, #0x18
10001df74: 540000e2    	b.hs	0x10001df90 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x158>
10001df78: f900010a    	str	x10, [x8]
10001df7c: d0000261    	adrp	x1, 0x10006b000 <_scoop$1$td$bb6274a8ad942aaa9e497e7cf9cedec226cd43a70f4602c59619ea3af8ce965e+0x50>
10001df80: aa1303e0    	mov	x0, x19
10001df84: 91294021    	add	x1, x1, #0xa50
10001df88: 94003ed0    	bl	0x10002dac8 <_scoop_runtime_finish_tlab_alloc>
10001df8c: 14000006    	b	0x10001dfa4 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x16c>
10001df90: d0000260    	adrp	x0, 0x10006b000 <_scoop$1$td$bb6274a8ad942aaa9e497e7cf9cedec226cd43a70f4602c59619ea3af8ce965e+0x50>
10001df94: 52800301    	mov	w1, #0x18               ; =24
10001df98: 91294000    	add	x0, x0, #0xa50
10001df9c: 94001bb8    	bl	0x100024e7c <_scoop_runtime_alloc_slow>
10001dfa0: aa0003f3    	mov	x19, x0
10001dfa4: aa1303e0    	mov	x0, x19
10001dfa8: f9000bf3    	str	x19, [sp, #0x10]
10001dfac: 97ff9c5b    	bl	0x100005118 <_scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
10001dfb0: f9400be0    	ldr	x0, [sp, #0x10]
10001dfb4: f90013e0    	str	x0, [sp, #0x20]
10001dfb8: 940020da    	bl	0x100026320 <_scoop_rt_throw>
