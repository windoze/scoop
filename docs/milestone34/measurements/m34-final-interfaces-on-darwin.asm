; Cell.next, MIR fn0, LIR scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002ed28 <_scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0>:
10002ed28: d100c3ff    	sub	sp, sp, #0x30
10002ed2c: a9014ff4    	stp	x20, x19, [sp, #0x10]
10002ed30: a9027bfd    	stp	x29, x30, [sp, #0x20]
10002ed34: 910083fd    	add	x29, sp, #0x20
10002ed38: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002ed3c: aa0003e9    	mov	x9, x0
10002ed40: 2a0103f3    	mov	w19, w1
10002ed44: 91174108    	add	x8, x8, #0x5d0
10002ed48: f940010a    	ldr	x10, [x8]
10002ed4c: aa0803e0    	mov	x0, x8
10002ed50: d63f0140    	blr	x10
10002ed54: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002ed58: f000094b    	adrp	x11, 0x100159000 <_scoop_gc_marker+0x800>
10002ed5c: 91202108    	add	x8, x8, #0x808
10002ed60: f940000a    	ldr	x10, [x0]
10002ed64: 911fd16b    	add	x11, x11, #0x7f4
10002ed68: f90007e9    	str	x9, [sp, #0x8]
10002ed6c: 9100214c    	add	x12, x10, #0x8
10002ed70: c8dffd08    	ldar	x8, [x8]
10002ed74: 88dffd6b    	ldar	w11, [x11]
10002ed78: 88dffd49    	ldar	w9, [x10]
10002ed7c: c8dffd8a    	ldar	x10, [x12]
10002ed80: 3500008b    	cbnz	w11, 0x10002ed90 <_scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0+0x68>
10002ed84: 7100053f    	cmp	w9, #0x1
10002ed88: fa480140    	ccmp	x10, x8, #0x0, eq
10002ed8c: 540000c0    	b.eq	0x10002eda4 <_scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0+0x7c>
10002ed90: f94007e8    	ldr	x8, [sp, #0x8]
10002ed94: f90003e8    	str	x8, [sp]
10002ed98: 94001e35    	bl	0x10003666c <_scoop_rt_safepoint>
10002ed9c: f94003e8    	ldr	x8, [sp]
10002eda0: f90007e8    	str	x8, [sp, #0x8]
10002eda4: f94007e8    	ldr	x8, [sp, #0x8]
10002eda8: a9427bfd    	ldp	x29, x30, [sp, #0x20]
10002edac: b9401108    	ldr	w8, [x8, #0x10]
10002edb0: 4a133508    	eor	w8, w8, w19, lsl #13
10002edb4: 4a130108    	eor	w8, w8, w19
10002edb8: a9414ff4    	ldp	x20, x19, [sp, #0x10]
10002edbc: 4a484508    	eor	w8, w8, w8, lsr #17
10002edc0: 4a081500    	eor	w0, w8, w8, lsl #5
10002edc4: 9100c3ff    	add	sp, sp, #0x30
10002edc8: d65f03c0    	ret

; Cell.$get$salt, MIR fn1, LIR scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002ee2c <_scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be>:
10002ee2c: d10083ff    	sub	sp, sp, #0x20
10002ee30: a9017bfd    	stp	x29, x30, [sp, #0x10]
10002ee34: 910043fd    	add	x29, sp, #0x10
10002ee38: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002ee3c: aa0003e9    	mov	x9, x0
10002ee40: 91174108    	add	x8, x8, #0x5d0
10002ee44: f940010a    	ldr	x10, [x8]
10002ee48: aa0803e0    	mov	x0, x8
10002ee4c: d63f0140    	blr	x10
10002ee50: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002ee54: f000094b    	adrp	x11, 0x100159000 <_scoop_gc_marker+0x800>
10002ee58: 91202108    	add	x8, x8, #0x808
10002ee5c: f940000a    	ldr	x10, [x0]
10002ee60: 911fd16b    	add	x11, x11, #0x7f4
10002ee64: f90007e9    	str	x9, [sp, #0x8]
10002ee68: 9100214c    	add	x12, x10, #0x8
10002ee6c: c8dffd08    	ldar	x8, [x8]
10002ee70: 88dffd6b    	ldar	w11, [x11]
10002ee74: 88dffd49    	ldar	w9, [x10]
10002ee78: c8dffd8a    	ldar	x10, [x12]
10002ee7c: 3500008b    	cbnz	w11, 0x10002ee8c <_scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be+0x60>
10002ee80: 7100053f    	cmp	w9, #0x1
10002ee84: fa480140    	ccmp	x10, x8, #0x0, eq
10002ee88: 540000c0    	b.eq	0x10002eea0 <_scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be+0x74>
10002ee8c: f94007e8    	ldr	x8, [sp, #0x8]
10002ee90: f90003e8    	str	x8, [sp]
10002ee94: 94001df6    	bl	0x10003666c <_scoop_rt_safepoint>
10002ee98: f94003e8    	ldr	x8, [sp]
10002ee9c: f90007e8    	str	x8, [sp, #0x8]
10002eea0: f94007e8    	ldr	x8, [sp, #0x8]
10002eea4: a9417bfd    	ldp	x29, x30, [sp, #0x10]
10002eea8: b9401100    	ldr	w0, [x8, #0x10]
10002eeac: 910083ff    	add	sp, sp, #0x20
10002eeb0: d65f03c0    	ret

; known, MIR fn2, LIR scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002ebfc <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb>:
10002ebfc: d101c3ff    	sub	sp, sp, #0x70
10002ec00: a9035ff8    	stp	x24, x23, [sp, #0x30]
10002ec04: a90457f6    	stp	x22, x21, [sp, #0x40]
10002ec08: a9054ff4    	stp	x20, x19, [sp, #0x50]
10002ec0c: a9067bfd    	stp	x29, x30, [sp, #0x60]
10002ec10: 910183fd    	add	x29, sp, #0x60
10002ec14: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002ec18: aa0003e9    	mov	x9, x0
10002ec1c: 2a0103f3    	mov	w19, w1
10002ec20: 91174108    	add	x8, x8, #0x5d0
10002ec24: f940010a    	ldr	x10, [x8]
10002ec28: aa0803e0    	mov	x0, x8
10002ec2c: d63f0140    	blr	x10
10002ec30: f0000955    	adrp	x21, 0x100159000 <_scoop_gc_marker+0x800>
10002ec34: f0000957    	adrp	x23, 0x100159000 <_scoop_gc_marker+0x800>
10002ec38: 912022b5    	add	x21, x21, #0x808
10002ec3c: f9400016    	ldr	x22, [x0]
10002ec40: f90013e9    	str	x9, [sp, #0x20]
10002ec44: 911fd2f7    	add	x23, x23, #0x7f4
10002ec48: f90017ff    	str	xzr, [sp, #0x28]
10002ec4c: 910022ca    	add	x10, x22, #0x8
10002ec50: c8dffea8    	ldar	x8, [x21]
10002ec54: 88dffeeb    	ldar	w11, [x23]
10002ec58: 88dffec9    	ldar	w9, [x22]
10002ec5c: c8dffd4a    	ldar	x10, [x10]
10002ec60: 3500008b    	cbnz	w11, 0x10002ec70 <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x74>
10002ec64: 7100053f    	cmp	w9, #0x1
10002ec68: fa480140    	ccmp	x10, x8, #0x0, eq
10002ec6c: 540000c0    	b.eq	0x10002ec84 <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x88>
10002ec70: f94013e8    	ldr	x8, [sp, #0x20]
10002ec74: f90007e8    	str	x8, [sp, #0x8]
10002ec78: 94001e7d    	bl	0x10003666c <_scoop_rt_safepoint>
10002ec7c: f94007e8    	ldr	x8, [sp, #0x8]
10002ec80: f90013e8    	str	x8, [sp, #0x20]
10002ec84: f94013e8    	ldr	x8, [sp, #0x20]
10002ec88: 528acf14    	mov	w20, #0x5678            ; =22136
10002ec8c: 2a1f03f8    	mov	w24, wzr
10002ec90: 72a24694    	movk	w20, #0x1234, lsl #16
10002ec94: f9000fe8    	str	x8, [sp, #0x18]
10002ec98: c8dffea8    	ldar	x8, [x21]
10002ec9c: 88dffee9    	ldar	w9, [x23]
10002eca0: 910022cb    	add	x11, x22, #0x8
10002eca4: 88dffeca    	ldar	w10, [x22]
10002eca8: c8dffd6b    	ldar	x11, [x11]
10002ecac: 7100013f    	cmp	w9, #0x0
10002ecb0: 7a410940    	ccmp	w10, #0x1, #0x0, eq
10002ecb4: fa480160    	ccmp	x11, x8, #0x0, eq
10002ecb8: 540000c0    	b.eq	0x10002ecd0 <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0xd4>
10002ecbc: f9400fe8    	ldr	x8, [sp, #0x18]
10002ecc0: f90007e8    	str	x8, [sp, #0x8]
10002ecc4: 94001e6a    	bl	0x10003666c <_scoop_rt_safepoint>
10002ecc8: f94007e8    	ldr	x8, [sp, #0x8]
10002eccc: f9000fe8    	str	x8, [sp, #0x18]
10002ecd0: 6b13031f    	cmp	w24, w19
10002ecd4: 540001ca    	b.ge	0x10002ed0c <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x110>
10002ecd8: f9400fe8    	ldr	x8, [sp, #0x18]
10002ecdc: 2a1403e1    	mov	w1, w20
10002ece0: f9000be8    	str	x8, [sp, #0x10]
10002ece4: f9400be0    	ldr	x0, [sp, #0x10]
10002ece8: f90017e0    	str	x0, [sp, #0x28]
10002ecec: a90023e0    	stp	x0, x8, [sp]
10002ecf0: 9400000e    	bl	0x10002ed28 <_scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0>
10002ecf4: a94023e9    	ldp	x9, x8, [sp]
10002ecf8: 2a0003f4    	mov	w20, w0
10002ecfc: f9000fe8    	str	x8, [sp, #0x18]
10002ed00: f90017e9    	str	x9, [sp, #0x28]
10002ed04: 11000718    	add	w24, w24, #0x1
10002ed08: 17ffffe4    	b	0x10002ec98 <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x9c>
10002ed0c: 2a1403e0    	mov	w0, w20
10002ed10: a9467bfd    	ldp	x29, x30, [sp, #0x60]
10002ed14: a9454ff4    	ldp	x20, x19, [sp, #0x50]
10002ed18: a94457f6    	ldp	x22, x21, [sp, #0x40]
10002ed1c: a9435ff8    	ldp	x24, x23, [sp, #0x30]
10002ed20: 9101c3ff    	add	sp, sp, #0x70
10002ed24: d65f03c0    	ret

; unknown, MIR fn3, LIR scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002eac4 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0>:
10002eac4: d10203ff    	sub	sp, sp, #0x80
10002eac8: a90367fa    	stp	x26, x25, [sp, #0x30]
10002eacc: a9045ff8    	stp	x24, x23, [sp, #0x40]
10002ead0: a90557f6    	stp	x22, x21, [sp, #0x50]
10002ead4: a9064ff4    	stp	x20, x19, [sp, #0x60]
10002ead8: a9077bfd    	stp	x29, x30, [sp, #0x70]
10002eadc: 9101c3fd    	add	x29, sp, #0x70
10002eae0: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002eae4: aa0003e9    	mov	x9, x0
10002eae8: 2a0203f3    	mov	w19, w2
10002eaec: 91174108    	add	x8, x8, #0x5d0
10002eaf0: aa0103f4    	mov	x20, x1
10002eaf4: f940010a    	ldr	x10, [x8]
10002eaf8: aa0803e0    	mov	x0, x8
10002eafc: d63f0140    	blr	x10
10002eb00: f0000956    	adrp	x22, 0x100159000 <_scoop_gc_marker+0x800>
10002eb04: f0000958    	adrp	x24, 0x100159000 <_scoop_gc_marker+0x800>
10002eb08: 912022d6    	add	x22, x22, #0x808
10002eb0c: f9400017    	ldr	x23, [x0]
10002eb10: f90013e9    	str	x9, [sp, #0x20]
10002eb14: 911fd318    	add	x24, x24, #0x7f4
10002eb18: f90017ff    	str	xzr, [sp, #0x28]
10002eb1c: 910022ea    	add	x10, x23, #0x8
10002eb20: c8dffec8    	ldar	x8, [x22]
10002eb24: 88dfff0b    	ldar	w11, [x24]
10002eb28: 88dffee9    	ldar	w9, [x23]
10002eb2c: c8dffd4a    	ldar	x10, [x10]
10002eb30: 3500008b    	cbnz	w11, 0x10002eb40 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x7c>
10002eb34: 7100053f    	cmp	w9, #0x1
10002eb38: fa480140    	ccmp	x10, x8, #0x0, eq
10002eb3c: 540000c0    	b.eq	0x10002eb54 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x90>
10002eb40: f94013e8    	ldr	x8, [sp, #0x20]
10002eb44: f9000be8    	str	x8, [sp, #0x10]
10002eb48: 94001ec9    	bl	0x10003666c <_scoop_rt_safepoint>
10002eb4c: f9400be8    	ldr	x8, [sp, #0x10]
10002eb50: f90013e8    	str	x8, [sp, #0x20]
10002eb54: 528acf15    	mov	w21, #0x5678            ; =22136
10002eb58: 2a1f03f9    	mov	w25, wzr
10002eb5c: 72a24695    	movk	w21, #0x1234, lsl #16
10002eb60: c8dffec8    	ldar	x8, [x22]
10002eb64: 88dfff09    	ldar	w9, [x24]
10002eb68: 910022eb    	add	x11, x23, #0x8
10002eb6c: 88dffeea    	ldar	w10, [x23]
10002eb70: c8dffd6b    	ldar	x11, [x11]
10002eb74: 7100013f    	cmp	w9, #0x0
10002eb78: 7a410940    	ccmp	w10, #0x1, #0x0, eq
10002eb7c: fa480160    	ccmp	x11, x8, #0x0, eq
10002eb80: 540000c0    	b.eq	0x10002eb98 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0xd4>
10002eb84: f94013e8    	ldr	x8, [sp, #0x20]
10002eb88: f9000be8    	str	x8, [sp, #0x10]
10002eb8c: 94001eb8    	bl	0x10003666c <_scoop_rt_safepoint>
10002eb90: f9400be8    	ldr	x8, [sp, #0x10]
10002eb94: f90013e8    	str	x8, [sp, #0x20]
10002eb98: 6b13033f    	cmp	w25, w19
10002eb9c: 5400020a    	b.ge	0x10002ebdc <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x118>
10002eba0: f94013e8    	ldr	x8, [sp, #0x20]
10002eba4: 2a1503e1    	mov	w1, w21
10002eba8: f9000fe8    	str	x8, [sp, #0x18]
10002ebac: f9400fe9    	ldr	x9, [sp, #0x18]
10002ebb0: f9400fe0    	ldr	x0, [sp, #0x18]
10002ebb4: f9400289    	ldr	x9, [x20]
10002ebb8: f90017e0    	str	x0, [sp, #0x28]
10002ebbc: a900a3e0    	stp	x0, x8, [sp, #0x8]
10002ebc0: d63f0120    	blr	x9
10002ebc4: a940a3e9    	ldp	x9, x8, [sp, #0x8]
10002ebc8: 2a0003f5    	mov	w21, w0
10002ebcc: f90013e8    	str	x8, [sp, #0x20]
10002ebd0: f90017e9    	str	x9, [sp, #0x28]
10002ebd4: 11000739    	add	w25, w25, #0x1
10002ebd8: 17ffffe2    	b	0x10002eb60 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x9c>
10002ebdc: 2a1503e0    	mov	w0, w21
10002ebe0: a9477bfd    	ldp	x29, x30, [sp, #0x70]
10002ebe4: a9464ff4    	ldp	x20, x19, [sp, #0x60]
10002ebe8: a94557f6    	ldp	x22, x21, [sp, #0x50]
10002ebec: a9445ff8    	ldp	x24, x23, [sp, #0x40]
10002ebf0: a94367fa    	ldp	x26, x25, [sp, #0x30]
10002ebf4: 910203ff    	add	sp, sp, #0x80
10002ebf8: d65f03c0    	ret

; convertedOnce, MIR fn4, LIR scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002ddc0 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd>:
10002ddc0: d10243ff    	sub	sp, sp, #0x90
10002ddc4: a90467fa    	stp	x26, x25, [sp, #0x40]
10002ddc8: a9055ff8    	stp	x24, x23, [sp, #0x50]
10002ddcc: a90657f6    	stp	x22, x21, [sp, #0x60]
10002ddd0: a9074ff4    	stp	x20, x19, [sp, #0x70]
10002ddd4: a9087bfd    	stp	x29, x30, [sp, #0x80]
10002ddd8: 910203fd    	add	x29, sp, #0x80
10002dddc: 90000968    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002dde0: aa0003e9    	mov	x9, x0
10002dde4: 2a0103f3    	mov	w19, w1
10002dde8: 91174108    	add	x8, x8, #0x5d0
10002ddec: f940010a    	ldr	x10, [x8]
10002ddf0: aa0803e0    	mov	x0, x8
10002ddf4: d63f0140    	blr	x10
10002ddf8: 90000976    	adrp	x22, 0x100159000 <_scoop_gc_marker+0x800>
10002ddfc: 90000978    	adrp	x24, 0x100159000 <_scoop_gc_marker+0x800>
10002de00: 912022d6    	add	x22, x22, #0x808
10002de04: f9400017    	ldr	x23, [x0]
10002de08: f9001be9    	str	x9, [sp, #0x30]
10002de0c: 911fd318    	add	x24, x24, #0x7f4
10002de10: f9001fff    	str	xzr, [sp, #0x38]
10002de14: 910022ea    	add	x10, x23, #0x8
10002de18: c8dffec8    	ldar	x8, [x22]
10002de1c: 88dfff0b    	ldar	w11, [x24]
10002de20: 88dffee9    	ldar	w9, [x23]
10002de24: c8dffd4a    	ldar	x10, [x10]
10002de28: 3500008b    	cbnz	w11, 0x10002de38 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x78>
10002de2c: 7100053f    	cmp	w9, #0x1
10002de30: fa480140    	ccmp	x10, x8, #0x0, eq
10002de34: 540000c0    	b.eq	0x10002de4c <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x8c>
10002de38: f9401be8    	ldr	x8, [sp, #0x30]
10002de3c: f9000be8    	str	x8, [sp, #0x10]
10002de40: 9400220b    	bl	0x10003666c <_scoop_rt_safepoint>
10002de44: f9400be8    	ldr	x8, [sp, #0x10]
10002de48: f9001be8    	str	x8, [sp, #0x30]
10002de4c: f9401bf5    	ldr	x21, [sp, #0x30]
10002de50: d00007e1    	adrp	x1, 0x10012b000 <_scoop$1$sr$832b1482192b2f25978d81971d4dbb0993e8a3a1292ef4635526486a7af9020a+0x50>
10002de54: 9127c021    	add	x1, x1, #0x9f0
10002de58: aa1503e0    	mov	x0, x21
10002de5c: 94001a38    	bl	0x10003473c <_scoop_rt_is_instance>
10002de60: 36000620    	tbz	w0, #0x0, 0x10002df24 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x164>
10002de64: d00007e1    	adrp	x1, 0x10012b000 <_scoop$1$sr$832b1482192b2f25978d81971d4dbb0993e8a3a1292ef4635526486a7af9020a+0x50>
10002de68: f94002a0    	ldr	x0, [x21]
10002de6c: 9127c021    	add	x1, x1, #0x9f0
10002de70: 94001a99    	bl	0x1000348d4 <_scoop_rt_itable_lookup>
10002de74: f90013f5    	str	x21, [sp, #0x20]
10002de78: 528acf15    	mov	w21, #0x5678            ; =22136
10002de7c: aa0003f4    	mov	x20, x0
10002de80: 2a1f03f9    	mov	w25, wzr
10002de84: 72a24695    	movk	w21, #0x1234, lsl #16
10002de88: c8dffec8    	ldar	x8, [x22]
10002de8c: 88dfff09    	ldar	w9, [x24]
10002de90: 910022eb    	add	x11, x23, #0x8
10002de94: 88dffeea    	ldar	w10, [x23]
10002de98: c8dffd6b    	ldar	x11, [x11]
10002de9c: 7100013f    	cmp	w9, #0x0
10002dea0: 7a410940    	ccmp	w10, #0x1, #0x0, eq
10002dea4: fa480160    	ccmp	x11, x8, #0x0, eq
10002dea8: 540000c0    	b.eq	0x10002dec0 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x100>
10002deac: f94013e8    	ldr	x8, [sp, #0x20]
10002deb0: f9000be8    	str	x8, [sp, #0x10]
10002deb4: 940021ee    	bl	0x10003666c <_scoop_rt_safepoint>
10002deb8: f9400be8    	ldr	x8, [sp, #0x10]
10002debc: f90013e8    	str	x8, [sp, #0x20]
10002dec0: 6b13033f    	cmp	w25, w19
10002dec4: 5400020a    	b.ge	0x10002df04 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x144>
10002dec8: f94013e8    	ldr	x8, [sp, #0x20]
10002decc: 2a1503e1    	mov	w1, w21
10002ded0: f9000fe8    	str	x8, [sp, #0x18]
10002ded4: f9400fe9    	ldr	x9, [sp, #0x18]
10002ded8: f9400fe0    	ldr	x0, [sp, #0x18]
10002dedc: f9001fe0    	str	x0, [sp, #0x38]
10002dee0: f9400289    	ldr	x9, [x20]
10002dee4: a900a3e0    	stp	x0, x8, [sp, #0x8]
10002dee8: d63f0120    	blr	x9
10002deec: a940a3e9    	ldp	x9, x8, [sp, #0x8]
10002def0: 2a0003f5    	mov	w21, w0
10002def4: f90013e8    	str	x8, [sp, #0x20]
10002def8: f9001fe9    	str	x9, [sp, #0x38]
10002defc: 11000739    	add	w25, w25, #0x1
10002df00: 17ffffe2    	b	0x10002de88 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xc8>
10002df04: 2a1503e0    	mov	w0, w21
10002df08: a9487bfd    	ldp	x29, x30, [sp, #0x80]
10002df0c: a9474ff4    	ldp	x20, x19, [sp, #0x70]
10002df10: a94657f6    	ldp	x22, x21, [sp, #0x60]
10002df14: a9455ff8    	ldp	x24, x23, [sp, #0x50]
10002df18: a94467fa    	ldp	x26, x25, [sp, #0x40]
10002df1c: 910243ff    	add	sp, sp, #0x90
10002df20: d65f03c0    	ret
10002df24: 90000960    	adrp	x0, 0x100159000 <_scoop_gc_marker+0x800>
10002df28: d000028a    	adrp	x10, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002df2c: 9116e000    	add	x0, x0, #0x5b8
10002df30: 913cc14a    	add	x10, x10, #0xf30
10002df34: f9400008    	ldr	x8, [x0]
10002df38: f9400d49    	ldr	x9, [x10, #0x18]
10002df3c: d63f0100    	blr	x8
10002df40: f9400008    	ldr	x8, [x0]
10002df44: cb0903eb    	neg	x11, x9
10002df48: f940010c    	ldr	x12, [x8]
10002df4c: 8b0c012c    	add	x12, x9, x12
10002df50: d100058c    	sub	x12, x12, #0x1
10002df54: ea0b0193    	ands	x19, x12, x11
10002df58: 540002c0    	b.eq	0x10002dfb0 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1f0>
10002df5c: 91005d2c    	add	x12, x9, #0x17
10002df60: 8a0b0182    	and	x2, x12, x11
10002df64: 528ff00b    	mov	w11, #0x7f80            ; =32640
10002df68: eb0b005f    	cmp	x2, x11
10002df6c: 54000228    	b.hi	0x10002dfb0 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1f0>
10002df70: f940494a    	ldr	x10, [x10, #0x90]
10002df74: b50001ea    	cbnz	x10, 0x10002dfb0 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1f0>
10002df78: f940050b    	ldr	x11, [x8, #0x8]
10002df7c: 8b02026a    	add	x10, x19, x2
10002df80: eb0b015f    	cmp	x10, x11
10002df84: 54000168    	b.hi	0x10002dfb0 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1f0>
10002df88: 92f0000b    	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10002df8c: 8b0b0129    	add	x9, x9, x11
10002df90: b100613f    	cmn	x9, #0x18
10002df94: 540000e2    	b.hs	0x10002dfb0 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1f0>
10002df98: f900010a    	str	x10, [x8]
10002df9c: d0000281    	adrp	x1, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002dfa0: aa1303e0    	mov	x0, x19
10002dfa4: 913cc021    	add	x1, x1, #0xf30
10002dfa8: 9400103b    	bl	0x100032094 <_scoop_runtime_finish_tlab_alloc>
10002dfac: 14000006    	b	0x10002dfc4 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x204>
10002dfb0: d0000280    	adrp	x0, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002dfb4: 52800301    	mov	w1, #0x18               ; =24
10002dfb8: 913cc000    	add	x0, x0, #0xf30
10002dfbc: 940021b0    	bl	0x10003667c <_scoop_runtime_alloc_slow>
10002dfc0: aa0003f3    	mov	x19, x0
10002dfc4: aa1303e0    	mov	x0, x19
10002dfc8: f9000bf3    	str	x19, [sp, #0x10]
10002dfcc: 97ff8313    	bl	0x10000ec18 <_scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
10002dfd0: f9400be0    	ldr	x0, [sp, #0x10]
10002dfd4: f90017e0    	str	x0, [sp, #0x28]
10002dfd8: 94002722    	bl	0x100037c60 <_scoop_rt_throw>

; convertedEach, MIR fn5, LIR scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002e8a4 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b>:
10002e8a4: d10243ff    	sub	sp, sp, #0x90
10002e8a8: a90467fa    	stp	x26, x25, [sp, #0x40]
10002e8ac: a9055ff8    	stp	x24, x23, [sp, #0x50]
10002e8b0: a90657f6    	stp	x22, x21, [sp, #0x60]
10002e8b4: a9074ff4    	stp	x20, x19, [sp, #0x70]
10002e8b8: a9087bfd    	stp	x29, x30, [sp, #0x80]
10002e8bc: 910203fd    	add	x29, sp, #0x80
10002e8c0: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002e8c4: aa0003e9    	mov	x9, x0
10002e8c8: 2a0103f3    	mov	w19, w1
10002e8cc: 91174108    	add	x8, x8, #0x5d0
10002e8d0: f940010a    	ldr	x10, [x8]
10002e8d4: aa0803e0    	mov	x0, x8
10002e8d8: d63f0140    	blr	x10
10002e8dc: f0000957    	adrp	x23, 0x100159000 <_scoop_gc_marker+0x800>
10002e8e0: f0000959    	adrp	x25, 0x100159000 <_scoop_gc_marker+0x800>
10002e8e4: 912022f7    	add	x23, x23, #0x808
10002e8e8: f9400018    	ldr	x24, [x0]
10002e8ec: f9001be9    	str	x9, [sp, #0x30]
10002e8f0: 911fd339    	add	x25, x25, #0x7f4
10002e8f4: f9001fff    	str	xzr, [sp, #0x38]
10002e8f8: 9100230a    	add	x10, x24, #0x8
10002e8fc: c8dffee8    	ldar	x8, [x23]
10002e900: 88dfff2b    	ldar	w11, [x25]
10002e904: 88dfff09    	ldar	w9, [x24]
10002e908: c8dffd4a    	ldar	x10, [x10]
10002e90c: 3500008b    	cbnz	w11, 0x10002e91c <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x78>
10002e910: 7100053f    	cmp	w9, #0x1
10002e914: fa480140    	ccmp	x10, x8, #0x0, eq
10002e918: 540000c0    	b.eq	0x10002e930 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x8c>
10002e91c: f9401be8    	ldr	x8, [sp, #0x30]
10002e920: f9000be8    	str	x8, [sp, #0x10]
10002e924: 94001f52    	bl	0x10003666c <_scoop_rt_safepoint>
10002e928: f9400be8    	ldr	x8, [sp, #0x10]
10002e92c: f9001be8    	str	x8, [sp, #0x30]
10002e930: b00007f4    	adrp	x20, 0x10012b000 <_scoop$1$sr$832b1482192b2f25978d81971d4dbb0993e8a3a1292ef4635526486a7af9020a+0x50>
10002e934: 528acf15    	mov	w21, #0x5678            ; =22136
10002e938: 2a1f03fa    	mov	w26, wzr
10002e93c: 9127c294    	add	x20, x20, #0x9f0
10002e940: 72a24695    	movk	w21, #0x1234, lsl #16
10002e944: c8dffee8    	ldar	x8, [x23]
10002e948: 88dfff29    	ldar	w9, [x25]
10002e94c: 9100230b    	add	x11, x24, #0x8
10002e950: 88dfff0a    	ldar	w10, [x24]
10002e954: c8dffd6b    	ldar	x11, [x11]
10002e958: 7100013f    	cmp	w9, #0x0
10002e95c: 7a410940    	ccmp	w10, #0x1, #0x0, eq
10002e960: fa480160    	ccmp	x11, x8, #0x0, eq
10002e964: 540000c0    	b.eq	0x10002e97c <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xd8>
10002e968: f9401be8    	ldr	x8, [sp, #0x30]
10002e96c: f9000be8    	str	x8, [sp, #0x10]
10002e970: 94001f3f    	bl	0x10003666c <_scoop_rt_safepoint>
10002e974: f9400be8    	ldr	x8, [sp, #0x10]
10002e978: f9001be8    	str	x8, [sp, #0x30]
10002e97c: 6b13035f    	cmp	w26, w19
10002e980: 5400036a    	b.ge	0x10002e9ec <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x148>
10002e984: f9401bf6    	ldr	x22, [sp, #0x30]
10002e988: aa1403e1    	mov	x1, x20
10002e98c: aa1603e0    	mov	x0, x22
10002e990: 9400176b    	bl	0x10003473c <_scoop_rt_is_instance>
10002e994: 360003c0    	tbz	w0, #0x0, 0x10002ea0c <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x168>
10002e998: f94002c0    	ldr	x0, [x22]
10002e99c: aa1403e1    	mov	x1, x20
10002e9a0: 940017cd    	bl	0x1000348d4 <_scoop_rt_itable_lookup>
10002e9a4: f90013f6    	str	x22, [sp, #0x20]
10002e9a8: 2a1503e1    	mov	w1, w21
10002e9ac: f94013e8    	ldr	x8, [sp, #0x20]
10002e9b0: f9000fe8    	str	x8, [sp, #0x18]
10002e9b4: f9400fe8    	ldr	x8, [sp, #0x18]
10002e9b8: f9400fe8    	ldr	x8, [sp, #0x18]
10002e9bc: f9401be9    	ldr	x9, [sp, #0x30]
10002e9c0: f9001fe8    	str	x8, [sp, #0x38]
10002e9c4: f940000a    	ldr	x10, [x0]
10002e9c8: aa0803e0    	mov	x0, x8
10002e9cc: a900a7e8    	stp	x8, x9, [sp, #0x8]
10002e9d0: d63f0140    	blr	x10
10002e9d4: a940a3e9    	ldp	x9, x8, [sp, #0x8]
10002e9d8: 2a0003f5    	mov	w21, w0
10002e9dc: f9001be8    	str	x8, [sp, #0x30]
10002e9e0: f9001fe9    	str	x9, [sp, #0x38]
10002e9e4: 1100075a    	add	w26, w26, #0x1
10002e9e8: 17ffffd7    	b	0x10002e944 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xa0>
10002e9ec: 2a1503e0    	mov	w0, w21
10002e9f0: a9487bfd    	ldp	x29, x30, [sp, #0x80]
10002e9f4: a9474ff4    	ldp	x20, x19, [sp, #0x70]
10002e9f8: a94657f6    	ldp	x22, x21, [sp, #0x60]
10002e9fc: a9455ff8    	ldp	x24, x23, [sp, #0x50]
10002ea00: a94467fa    	ldp	x26, x25, [sp, #0x40]
10002ea04: 910243ff    	add	sp, sp, #0x90
10002ea08: d65f03c0    	ret
10002ea0c: f0000940    	adrp	x0, 0x100159000 <_scoop_gc_marker+0x800>
10002ea10: b000028a    	adrp	x10, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002ea14: 9116e000    	add	x0, x0, #0x5b8
10002ea18: 913cc14a    	add	x10, x10, #0xf30
10002ea1c: f9400008    	ldr	x8, [x0]
10002ea20: f9400d49    	ldr	x9, [x10, #0x18]
10002ea24: d63f0100    	blr	x8
10002ea28: f9400008    	ldr	x8, [x0]
10002ea2c: cb0903eb    	neg	x11, x9
10002ea30: f940010c    	ldr	x12, [x8]
10002ea34: 8b0c012c    	add	x12, x9, x12
10002ea38: d100058c    	sub	x12, x12, #0x1
10002ea3c: ea0b0193    	ands	x19, x12, x11
10002ea40: 540002c0    	b.eq	0x10002ea98 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
10002ea44: 91005d2c    	add	x12, x9, #0x17
10002ea48: 8a0b0182    	and	x2, x12, x11
10002ea4c: 528ff00b    	mov	w11, #0x7f80            ; =32640
10002ea50: eb0b005f    	cmp	x2, x11
10002ea54: 54000228    	b.hi	0x10002ea98 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
10002ea58: f940494a    	ldr	x10, [x10, #0x90]
10002ea5c: b50001ea    	cbnz	x10, 0x10002ea98 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
10002ea60: f940050b    	ldr	x11, [x8, #0x8]
10002ea64: 8b02026a    	add	x10, x19, x2
10002ea68: eb0b015f    	cmp	x10, x11
10002ea6c: 54000168    	b.hi	0x10002ea98 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
10002ea70: 92f0000b    	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10002ea74: 8b0b0129    	add	x9, x9, x11
10002ea78: b100613f    	cmn	x9, #0x18
10002ea7c: 540000e2    	b.hs	0x10002ea98 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
10002ea80: f900010a    	str	x10, [x8]
10002ea84: b0000281    	adrp	x1, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002ea88: aa1303e0    	mov	x0, x19
10002ea8c: 913cc021    	add	x1, x1, #0xf30
10002ea90: 94000d81    	bl	0x100032094 <_scoop_runtime_finish_tlab_alloc>
10002ea94: 14000006    	b	0x10002eaac <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x208>
10002ea98: b0000280    	adrp	x0, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002ea9c: 52800301    	mov	w1, #0x18               ; =24
10002eaa0: 913cc000    	add	x0, x0, #0xf30
10002eaa4: 94001ef6    	bl	0x10003667c <_scoop_runtime_alloc_slow>
10002eaa8: aa0003f3    	mov	x19, x0
10002eaac: aa1303e0    	mov	x0, x19
10002eab0: f9000bf3    	str	x19, [sp, #0x10]
10002eab4: 97ff8059    	bl	0x10000ec18 <_scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
10002eab8: f9400be0    	ldr	x0, [sp, #0x10]
10002eabc: f90017e0    	str	x0, [sp, #0x28]
10002eac0: 94002468    	bl	0x100037c60 <_scoop_rt_throw>
