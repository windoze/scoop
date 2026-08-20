; Cell.next, MIR fn0, LIR scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002eca8 <_scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0>:
10002eca8: d100c3ff    	sub	sp, sp, #0x30
10002ecac: a9014ff4    	stp	x20, x19, [sp, #0x10]
10002ecb0: a9027bfd    	stp	x29, x30, [sp, #0x20]
10002ecb4: 910083fd    	add	x29, sp, #0x20
10002ecb8: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002ecbc: aa0003e9    	mov	x9, x0
10002ecc0: 2a0103f3    	mov	w19, w1
10002ecc4: 91174108    	add	x8, x8, #0x5d0
10002ecc8: f940010a    	ldr	x10, [x8]
10002eccc: aa0803e0    	mov	x0, x8
10002ecd0: d63f0140    	blr	x10
10002ecd4: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002ecd8: f000094b    	adrp	x11, 0x100159000 <_scoop_gc_marker+0x800>
10002ecdc: 91202108    	add	x8, x8, #0x808
10002ece0: f940000a    	ldr	x10, [x0]
10002ece4: 911fd16b    	add	x11, x11, #0x7f4
10002ece8: f90007e9    	str	x9, [sp, #0x8]
10002ecec: 9100214c    	add	x12, x10, #0x8
10002ecf0: c8dffd08    	ldar	x8, [x8]
10002ecf4: 88dffd6b    	ldar	w11, [x11]
10002ecf8: 88dffd49    	ldar	w9, [x10]
10002ecfc: c8dffd8a    	ldar	x10, [x12]
10002ed00: 3500008b    	cbnz	w11, 0x10002ed10 <_scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0+0x68>
10002ed04: 7100053f    	cmp	w9, #0x1
10002ed08: fa480140    	ccmp	x10, x8, #0x0, eq
10002ed0c: 540000c0    	b.eq	0x10002ed24 <_scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0+0x7c>
10002ed10: f94007e8    	ldr	x8, [sp, #0x8]
10002ed14: f90003e8    	str	x8, [sp]
10002ed18: 94001e35    	bl	0x1000365ec <_scoop_rt_safepoint>
10002ed1c: f94003e8    	ldr	x8, [sp]
10002ed20: f90007e8    	str	x8, [sp, #0x8]
10002ed24: f94007e8    	ldr	x8, [sp, #0x8]
10002ed28: a9427bfd    	ldp	x29, x30, [sp, #0x20]
10002ed2c: b9401108    	ldr	w8, [x8, #0x10]
10002ed30: 4a133508    	eor	w8, w8, w19, lsl #13
10002ed34: 4a130108    	eor	w8, w8, w19
10002ed38: a9414ff4    	ldp	x20, x19, [sp, #0x10]
10002ed3c: 4a484508    	eor	w8, w8, w8, lsr #17
10002ed40: 4a081500    	eor	w0, w8, w8, lsl #5
10002ed44: 9100c3ff    	add	sp, sp, #0x30
10002ed48: d65f03c0    	ret

; Cell.$get$salt, MIR fn1, LIR scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002edac <_scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be>:
10002edac: d10083ff    	sub	sp, sp, #0x20
10002edb0: a9017bfd    	stp	x29, x30, [sp, #0x10]
10002edb4: 910043fd    	add	x29, sp, #0x10
10002edb8: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002edbc: aa0003e9    	mov	x9, x0
10002edc0: 91174108    	add	x8, x8, #0x5d0
10002edc4: f940010a    	ldr	x10, [x8]
10002edc8: aa0803e0    	mov	x0, x8
10002edcc: d63f0140    	blr	x10
10002edd0: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002edd4: f000094b    	adrp	x11, 0x100159000 <_scoop_gc_marker+0x800>
10002edd8: 91202108    	add	x8, x8, #0x808
10002eddc: f940000a    	ldr	x10, [x0]
10002ede0: 911fd16b    	add	x11, x11, #0x7f4
10002ede4: f90007e9    	str	x9, [sp, #0x8]
10002ede8: 9100214c    	add	x12, x10, #0x8
10002edec: c8dffd08    	ldar	x8, [x8]
10002edf0: 88dffd6b    	ldar	w11, [x11]
10002edf4: 88dffd49    	ldar	w9, [x10]
10002edf8: c8dffd8a    	ldar	x10, [x12]
10002edfc: 3500008b    	cbnz	w11, 0x10002ee0c <_scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be+0x60>
10002ee00: 7100053f    	cmp	w9, #0x1
10002ee04: fa480140    	ccmp	x10, x8, #0x0, eq
10002ee08: 540000c0    	b.eq	0x10002ee20 <_scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be+0x74>
10002ee0c: f94007e8    	ldr	x8, [sp, #0x8]
10002ee10: f90003e8    	str	x8, [sp]
10002ee14: 94001df6    	bl	0x1000365ec <_scoop_rt_safepoint>
10002ee18: f94003e8    	ldr	x8, [sp]
10002ee1c: f90007e8    	str	x8, [sp, #0x8]
10002ee20: f94007e8    	ldr	x8, [sp, #0x8]
10002ee24: a9417bfd    	ldp	x29, x30, [sp, #0x10]
10002ee28: b9401100    	ldr	w0, [x8, #0x10]
10002ee2c: 910083ff    	add	sp, sp, #0x20
10002ee30: d65f03c0    	ret

; known, MIR fn2, LIR scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002eb7c <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb>:
10002eb7c: d101c3ff    	sub	sp, sp, #0x70
10002eb80: a9035ff8    	stp	x24, x23, [sp, #0x30]
10002eb84: a90457f6    	stp	x22, x21, [sp, #0x40]
10002eb88: a9054ff4    	stp	x20, x19, [sp, #0x50]
10002eb8c: a9067bfd    	stp	x29, x30, [sp, #0x60]
10002eb90: 910183fd    	add	x29, sp, #0x60
10002eb94: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002eb98: aa0003e9    	mov	x9, x0
10002eb9c: 2a0103f3    	mov	w19, w1
10002eba0: 91174108    	add	x8, x8, #0x5d0
10002eba4: f940010a    	ldr	x10, [x8]
10002eba8: aa0803e0    	mov	x0, x8
10002ebac: d63f0140    	blr	x10
10002ebb0: f0000955    	adrp	x21, 0x100159000 <_scoop_gc_marker+0x800>
10002ebb4: f0000957    	adrp	x23, 0x100159000 <_scoop_gc_marker+0x800>
10002ebb8: 912022b5    	add	x21, x21, #0x808
10002ebbc: f9400016    	ldr	x22, [x0]
10002ebc0: f90013e9    	str	x9, [sp, #0x20]
10002ebc4: 911fd2f7    	add	x23, x23, #0x7f4
10002ebc8: f90017ff    	str	xzr, [sp, #0x28]
10002ebcc: 910022ca    	add	x10, x22, #0x8
10002ebd0: c8dffea8    	ldar	x8, [x21]
10002ebd4: 88dffeeb    	ldar	w11, [x23]
10002ebd8: 88dffec9    	ldar	w9, [x22]
10002ebdc: c8dffd4a    	ldar	x10, [x10]
10002ebe0: 3500008b    	cbnz	w11, 0x10002ebf0 <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x74>
10002ebe4: 7100053f    	cmp	w9, #0x1
10002ebe8: fa480140    	ccmp	x10, x8, #0x0, eq
10002ebec: 540000c0    	b.eq	0x10002ec04 <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x88>
10002ebf0: f94013e8    	ldr	x8, [sp, #0x20]
10002ebf4: f90007e8    	str	x8, [sp, #0x8]
10002ebf8: 94001e7d    	bl	0x1000365ec <_scoop_rt_safepoint>
10002ebfc: f94007e8    	ldr	x8, [sp, #0x8]
10002ec00: f90013e8    	str	x8, [sp, #0x20]
10002ec04: f94013e8    	ldr	x8, [sp, #0x20]
10002ec08: 528acf14    	mov	w20, #0x5678            ; =22136
10002ec0c: 2a1f03f8    	mov	w24, wzr
10002ec10: 72a24694    	movk	w20, #0x1234, lsl #16
10002ec14: f9000fe8    	str	x8, [sp, #0x18]
10002ec18: c8dffea8    	ldar	x8, [x21]
10002ec1c: 88dffee9    	ldar	w9, [x23]
10002ec20: 910022cb    	add	x11, x22, #0x8
10002ec24: 88dffeca    	ldar	w10, [x22]
10002ec28: c8dffd6b    	ldar	x11, [x11]
10002ec2c: 7100013f    	cmp	w9, #0x0
10002ec30: 7a410940    	ccmp	w10, #0x1, #0x0, eq
10002ec34: fa480160    	ccmp	x11, x8, #0x0, eq
10002ec38: 540000c0    	b.eq	0x10002ec50 <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0xd4>
10002ec3c: f9400fe8    	ldr	x8, [sp, #0x18]
10002ec40: f90007e8    	str	x8, [sp, #0x8]
10002ec44: 94001e6a    	bl	0x1000365ec <_scoop_rt_safepoint>
10002ec48: f94007e8    	ldr	x8, [sp, #0x8]
10002ec4c: f9000fe8    	str	x8, [sp, #0x18]
10002ec50: 6b13031f    	cmp	w24, w19
10002ec54: 540001ca    	b.ge	0x10002ec8c <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x110>
10002ec58: f9400fe8    	ldr	x8, [sp, #0x18]
10002ec5c: 2a1403e1    	mov	w1, w20
10002ec60: f9000be8    	str	x8, [sp, #0x10]
10002ec64: f9400be0    	ldr	x0, [sp, #0x10]
10002ec68: f90017e0    	str	x0, [sp, #0x28]
10002ec6c: a90023e0    	stp	x0, x8, [sp]
10002ec70: 9400000e    	bl	0x10002eca8 <_scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0>
10002ec74: a94023e9    	ldp	x9, x8, [sp]
10002ec78: 2a0003f4    	mov	w20, w0
10002ec7c: f9000fe8    	str	x8, [sp, #0x18]
10002ec80: f90017e9    	str	x9, [sp, #0x28]
10002ec84: 11000718    	add	w24, w24, #0x1
10002ec88: 17ffffe4    	b	0x10002ec18 <_scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x9c>
10002ec8c: 2a1403e0    	mov	w0, w20
10002ec90: a9467bfd    	ldp	x29, x30, [sp, #0x60]
10002ec94: a9454ff4    	ldp	x20, x19, [sp, #0x50]
10002ec98: a94457f6    	ldp	x22, x21, [sp, #0x40]
10002ec9c: a9435ff8    	ldp	x24, x23, [sp, #0x30]
10002eca0: 9101c3ff    	add	sp, sp, #0x70
10002eca4: d65f03c0    	ret

; unknown, MIR fn3, LIR scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002ea44 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0>:
10002ea44: d10203ff    	sub	sp, sp, #0x80
10002ea48: a90367fa    	stp	x26, x25, [sp, #0x30]
10002ea4c: a9045ff8    	stp	x24, x23, [sp, #0x40]
10002ea50: a90557f6    	stp	x22, x21, [sp, #0x50]
10002ea54: a9064ff4    	stp	x20, x19, [sp, #0x60]
10002ea58: a9077bfd    	stp	x29, x30, [sp, #0x70]
10002ea5c: 9101c3fd    	add	x29, sp, #0x70
10002ea60: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002ea64: aa0003e9    	mov	x9, x0
10002ea68: 2a0203f3    	mov	w19, w2
10002ea6c: 91174108    	add	x8, x8, #0x5d0
10002ea70: aa0103f4    	mov	x20, x1
10002ea74: f940010a    	ldr	x10, [x8]
10002ea78: aa0803e0    	mov	x0, x8
10002ea7c: d63f0140    	blr	x10
10002ea80: f0000956    	adrp	x22, 0x100159000 <_scoop_gc_marker+0x800>
10002ea84: f0000958    	adrp	x24, 0x100159000 <_scoop_gc_marker+0x800>
10002ea88: 912022d6    	add	x22, x22, #0x808
10002ea8c: f9400017    	ldr	x23, [x0]
10002ea90: f90013e9    	str	x9, [sp, #0x20]
10002ea94: 911fd318    	add	x24, x24, #0x7f4
10002ea98: f90017ff    	str	xzr, [sp, #0x28]
10002ea9c: 910022ea    	add	x10, x23, #0x8
10002eaa0: c8dffec8    	ldar	x8, [x22]
10002eaa4: 88dfff0b    	ldar	w11, [x24]
10002eaa8: 88dffee9    	ldar	w9, [x23]
10002eaac: c8dffd4a    	ldar	x10, [x10]
10002eab0: 3500008b    	cbnz	w11, 0x10002eac0 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x7c>
10002eab4: 7100053f    	cmp	w9, #0x1
10002eab8: fa480140    	ccmp	x10, x8, #0x0, eq
10002eabc: 540000c0    	b.eq	0x10002ead4 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x90>
10002eac0: f94013e8    	ldr	x8, [sp, #0x20]
10002eac4: f9000be8    	str	x8, [sp, #0x10]
10002eac8: 94001ec9    	bl	0x1000365ec <_scoop_rt_safepoint>
10002eacc: f9400be8    	ldr	x8, [sp, #0x10]
10002ead0: f90013e8    	str	x8, [sp, #0x20]
10002ead4: 528acf15    	mov	w21, #0x5678            ; =22136
10002ead8: 2a1f03f9    	mov	w25, wzr
10002eadc: 72a24695    	movk	w21, #0x1234, lsl #16
10002eae0: c8dffec8    	ldar	x8, [x22]
10002eae4: 88dfff09    	ldar	w9, [x24]
10002eae8: 910022eb    	add	x11, x23, #0x8
10002eaec: 88dffeea    	ldar	w10, [x23]
10002eaf0: c8dffd6b    	ldar	x11, [x11]
10002eaf4: 7100013f    	cmp	w9, #0x0
10002eaf8: 7a410940    	ccmp	w10, #0x1, #0x0, eq
10002eafc: fa480160    	ccmp	x11, x8, #0x0, eq
10002eb00: 540000c0    	b.eq	0x10002eb18 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0xd4>
10002eb04: f94013e8    	ldr	x8, [sp, #0x20]
10002eb08: f9000be8    	str	x8, [sp, #0x10]
10002eb0c: 94001eb8    	bl	0x1000365ec <_scoop_rt_safepoint>
10002eb10: f9400be8    	ldr	x8, [sp, #0x10]
10002eb14: f90013e8    	str	x8, [sp, #0x20]
10002eb18: 6b13033f    	cmp	w25, w19
10002eb1c: 5400020a    	b.ge	0x10002eb5c <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x118>
10002eb20: f94013e8    	ldr	x8, [sp, #0x20]
10002eb24: 2a1503e1    	mov	w1, w21
10002eb28: f9000fe8    	str	x8, [sp, #0x18]
10002eb2c: f9400fe9    	ldr	x9, [sp, #0x18]
10002eb30: f9400fe0    	ldr	x0, [sp, #0x18]
10002eb34: f9400289    	ldr	x9, [x20]
10002eb38: f90017e0    	str	x0, [sp, #0x28]
10002eb3c: a900a3e0    	stp	x0, x8, [sp, #0x8]
10002eb40: d63f0120    	blr	x9
10002eb44: a940a3e9    	ldp	x9, x8, [sp, #0x8]
10002eb48: 2a0003f5    	mov	w21, w0
10002eb4c: f90013e8    	str	x8, [sp, #0x20]
10002eb50: f90017e9    	str	x9, [sp, #0x28]
10002eb54: 11000739    	add	w25, w25, #0x1
10002eb58: 17ffffe2    	b	0x10002eae0 <_scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x9c>
10002eb5c: 2a1503e0    	mov	w0, w21
10002eb60: a9477bfd    	ldp	x29, x30, [sp, #0x70]
10002eb64: a9464ff4    	ldp	x20, x19, [sp, #0x60]
10002eb68: a94557f6    	ldp	x22, x21, [sp, #0x50]
10002eb6c: a9445ff8    	ldp	x24, x23, [sp, #0x40]
10002eb70: a94367fa    	ldp	x26, x25, [sp, #0x30]
10002eb74: 910203ff    	add	sp, sp, #0x80
10002eb78: d65f03c0    	ret

; convertedOnce, MIR fn4, LIR scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002dd40 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd>:
10002dd40: d10243ff    	sub	sp, sp, #0x90
10002dd44: a90467fa    	stp	x26, x25, [sp, #0x40]
10002dd48: a9055ff8    	stp	x24, x23, [sp, #0x50]
10002dd4c: a90657f6    	stp	x22, x21, [sp, #0x60]
10002dd50: a9074ff4    	stp	x20, x19, [sp, #0x70]
10002dd54: a9087bfd    	stp	x29, x30, [sp, #0x80]
10002dd58: 910203fd    	add	x29, sp, #0x80
10002dd5c: 90000968    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002dd60: aa0003e9    	mov	x9, x0
10002dd64: 2a0103f3    	mov	w19, w1
10002dd68: 91174108    	add	x8, x8, #0x5d0
10002dd6c: f940010a    	ldr	x10, [x8]
10002dd70: aa0803e0    	mov	x0, x8
10002dd74: d63f0140    	blr	x10
10002dd78: 90000976    	adrp	x22, 0x100159000 <_scoop_gc_marker+0x800>
10002dd7c: 90000978    	adrp	x24, 0x100159000 <_scoop_gc_marker+0x800>
10002dd80: 912022d6    	add	x22, x22, #0x808
10002dd84: f9400017    	ldr	x23, [x0]
10002dd88: f9001be9    	str	x9, [sp, #0x30]
10002dd8c: 911fd318    	add	x24, x24, #0x7f4
10002dd90: f9001fff    	str	xzr, [sp, #0x38]
10002dd94: 910022ea    	add	x10, x23, #0x8
10002dd98: c8dffec8    	ldar	x8, [x22]
10002dd9c: 88dfff0b    	ldar	w11, [x24]
10002dda0: 88dffee9    	ldar	w9, [x23]
10002dda4: c8dffd4a    	ldar	x10, [x10]
10002dda8: 3500008b    	cbnz	w11, 0x10002ddb8 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x78>
10002ddac: 7100053f    	cmp	w9, #0x1
10002ddb0: fa480140    	ccmp	x10, x8, #0x0, eq
10002ddb4: 540000c0    	b.eq	0x10002ddcc <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x8c>
10002ddb8: f9401be8    	ldr	x8, [sp, #0x30]
10002ddbc: f9000be8    	str	x8, [sp, #0x10]
10002ddc0: 9400220b    	bl	0x1000365ec <_scoop_rt_safepoint>
10002ddc4: f9400be8    	ldr	x8, [sp, #0x10]
10002ddc8: f9001be8    	str	x8, [sp, #0x30]
10002ddcc: f9401bf5    	ldr	x21, [sp, #0x30]
10002ddd0: d00007e1    	adrp	x1, 0x10012b000 <_scoop$1$sr$832b1482192b2f25978d81971d4dbb0993e8a3a1292ef4635526486a7af9020a+0x50>
10002ddd4: 9127c021    	add	x1, x1, #0x9f0
10002ddd8: aa1503e0    	mov	x0, x21
10002dddc: 94001a38    	bl	0x1000346bc <_scoop_rt_is_instance>
10002dde0: 36000620    	tbz	w0, #0x0, 0x10002dea4 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x164>
10002dde4: d00007e1    	adrp	x1, 0x10012b000 <_scoop$1$sr$832b1482192b2f25978d81971d4dbb0993e8a3a1292ef4635526486a7af9020a+0x50>
10002dde8: f94002a0    	ldr	x0, [x21]
10002ddec: 9127c021    	add	x1, x1, #0x9f0
10002ddf0: 94001a99    	bl	0x100034854 <_scoop_rt_itable_lookup>
10002ddf4: f90013f5    	str	x21, [sp, #0x20]
10002ddf8: 528acf15    	mov	w21, #0x5678            ; =22136
10002ddfc: aa0003f4    	mov	x20, x0
10002de00: 2a1f03f9    	mov	w25, wzr
10002de04: 72a24695    	movk	w21, #0x1234, lsl #16
10002de08: c8dffec8    	ldar	x8, [x22]
10002de0c: 88dfff09    	ldar	w9, [x24]
10002de10: 910022eb    	add	x11, x23, #0x8
10002de14: 88dffeea    	ldar	w10, [x23]
10002de18: c8dffd6b    	ldar	x11, [x11]
10002de1c: 7100013f    	cmp	w9, #0x0
10002de20: 7a410940    	ccmp	w10, #0x1, #0x0, eq
10002de24: fa480160    	ccmp	x11, x8, #0x0, eq
10002de28: 540000c0    	b.eq	0x10002de40 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x100>
10002de2c: f94013e8    	ldr	x8, [sp, #0x20]
10002de30: f9000be8    	str	x8, [sp, #0x10]
10002de34: 940021ee    	bl	0x1000365ec <_scoop_rt_safepoint>
10002de38: f9400be8    	ldr	x8, [sp, #0x10]
10002de3c: f90013e8    	str	x8, [sp, #0x20]
10002de40: 6b13033f    	cmp	w25, w19
10002de44: 5400020a    	b.ge	0x10002de84 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x144>
10002de48: f94013e8    	ldr	x8, [sp, #0x20]
10002de4c: 2a1503e1    	mov	w1, w21
10002de50: f9000fe8    	str	x8, [sp, #0x18]
10002de54: f9400fe9    	ldr	x9, [sp, #0x18]
10002de58: f9400fe0    	ldr	x0, [sp, #0x18]
10002de5c: f9001fe0    	str	x0, [sp, #0x38]
10002de60: f9400289    	ldr	x9, [x20]
10002de64: a900a3e0    	stp	x0, x8, [sp, #0x8]
10002de68: d63f0120    	blr	x9
10002de6c: a940a3e9    	ldp	x9, x8, [sp, #0x8]
10002de70: 2a0003f5    	mov	w21, w0
10002de74: f90013e8    	str	x8, [sp, #0x20]
10002de78: f9001fe9    	str	x9, [sp, #0x38]
10002de7c: 11000739    	add	w25, w25, #0x1
10002de80: 17ffffe2    	b	0x10002de08 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xc8>
10002de84: 2a1503e0    	mov	w0, w21
10002de88: a9487bfd    	ldp	x29, x30, [sp, #0x80]
10002de8c: a9474ff4    	ldp	x20, x19, [sp, #0x70]
10002de90: a94657f6    	ldp	x22, x21, [sp, #0x60]
10002de94: a9455ff8    	ldp	x24, x23, [sp, #0x50]
10002de98: a94467fa    	ldp	x26, x25, [sp, #0x40]
10002de9c: 910243ff    	add	sp, sp, #0x90
10002dea0: d65f03c0    	ret
10002dea4: 90000960    	adrp	x0, 0x100159000 <_scoop_gc_marker+0x800>
10002dea8: d000028a    	adrp	x10, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002deac: 9116e000    	add	x0, x0, #0x5b8
10002deb0: 913cc14a    	add	x10, x10, #0xf30
10002deb4: f9400008    	ldr	x8, [x0]
10002deb8: f9400d49    	ldr	x9, [x10, #0x18]
10002debc: d63f0100    	blr	x8
10002dec0: f9400008    	ldr	x8, [x0]
10002dec4: cb0903eb    	neg	x11, x9
10002dec8: f940010c    	ldr	x12, [x8]
10002decc: 8b0c012c    	add	x12, x9, x12
10002ded0: d100058c    	sub	x12, x12, #0x1
10002ded4: ea0b0193    	ands	x19, x12, x11
10002ded8: 540002c0    	b.eq	0x10002df30 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1f0>
10002dedc: 91005d2c    	add	x12, x9, #0x17
10002dee0: 8a0b0182    	and	x2, x12, x11
10002dee4: 528ff00b    	mov	w11, #0x7f80            ; =32640
10002dee8: eb0b005f    	cmp	x2, x11
10002deec: 54000228    	b.hi	0x10002df30 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1f0>
10002def0: f940494a    	ldr	x10, [x10, #0x90]
10002def4: b50001ea    	cbnz	x10, 0x10002df30 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1f0>
10002def8: f940050b    	ldr	x11, [x8, #0x8]
10002defc: 8b02026a    	add	x10, x19, x2
10002df00: eb0b015f    	cmp	x10, x11
10002df04: 54000168    	b.hi	0x10002df30 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1f0>
10002df08: 92f0000b    	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10002df0c: 8b0b0129    	add	x9, x9, x11
10002df10: b100613f    	cmn	x9, #0x18
10002df14: 540000e2    	b.hs	0x10002df30 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1f0>
10002df18: f900010a    	str	x10, [x8]
10002df1c: d0000281    	adrp	x1, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002df20: aa1303e0    	mov	x0, x19
10002df24: 913cc021    	add	x1, x1, #0xf30
10002df28: 9400103b    	bl	0x100032014 <_scoop_runtime_finish_tlab_alloc>
10002df2c: 14000006    	b	0x10002df44 <_scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x204>
10002df30: d0000280    	adrp	x0, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002df34: 52800301    	mov	w1, #0x18               ; =24
10002df38: 913cc000    	add	x0, x0, #0xf30
10002df3c: 940021b0    	bl	0x1000365fc <_scoop_runtime_alloc_slow>
10002df40: aa0003f3    	mov	x19, x0
10002df44: aa1303e0    	mov	x0, x19
10002df48: f9000bf3    	str	x19, [sp, #0x10]
10002df4c: 97ff8323    	bl	0x10000ebd8 <_scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
10002df50: f9400be0    	ldr	x0, [sp, #0x10]
10002df54: f90017e0    	str	x0, [sp, #0x28]
10002df58: 94002722    	bl	0x100037be0 <_scoop_rt_throw>

; convertedEach, MIR fn5, LIR scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-darwin/interfaces:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002e824 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b>:
10002e824: d10243ff    	sub	sp, sp, #0x90
10002e828: a90467fa    	stp	x26, x25, [sp, #0x40]
10002e82c: a9055ff8    	stp	x24, x23, [sp, #0x50]
10002e830: a90657f6    	stp	x22, x21, [sp, #0x60]
10002e834: a9074ff4    	stp	x20, x19, [sp, #0x70]
10002e838: a9087bfd    	stp	x29, x30, [sp, #0x80]
10002e83c: 910203fd    	add	x29, sp, #0x80
10002e840: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002e844: aa0003e9    	mov	x9, x0
10002e848: 2a0103f3    	mov	w19, w1
10002e84c: 91174108    	add	x8, x8, #0x5d0
10002e850: f940010a    	ldr	x10, [x8]
10002e854: aa0803e0    	mov	x0, x8
10002e858: d63f0140    	blr	x10
10002e85c: f0000957    	adrp	x23, 0x100159000 <_scoop_gc_marker+0x800>
10002e860: f0000959    	adrp	x25, 0x100159000 <_scoop_gc_marker+0x800>
10002e864: 912022f7    	add	x23, x23, #0x808
10002e868: f9400018    	ldr	x24, [x0]
10002e86c: f9001be9    	str	x9, [sp, #0x30]
10002e870: 911fd339    	add	x25, x25, #0x7f4
10002e874: f9001fff    	str	xzr, [sp, #0x38]
10002e878: 9100230a    	add	x10, x24, #0x8
10002e87c: c8dffee8    	ldar	x8, [x23]
10002e880: 88dfff2b    	ldar	w11, [x25]
10002e884: 88dfff09    	ldar	w9, [x24]
10002e888: c8dffd4a    	ldar	x10, [x10]
10002e88c: 3500008b    	cbnz	w11, 0x10002e89c <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x78>
10002e890: 7100053f    	cmp	w9, #0x1
10002e894: fa480140    	ccmp	x10, x8, #0x0, eq
10002e898: 540000c0    	b.eq	0x10002e8b0 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x8c>
10002e89c: f9401be8    	ldr	x8, [sp, #0x30]
10002e8a0: f9000be8    	str	x8, [sp, #0x10]
10002e8a4: 94001f52    	bl	0x1000365ec <_scoop_rt_safepoint>
10002e8a8: f9400be8    	ldr	x8, [sp, #0x10]
10002e8ac: f9001be8    	str	x8, [sp, #0x30]
10002e8b0: b00007f4    	adrp	x20, 0x10012b000 <_scoop$1$sr$832b1482192b2f25978d81971d4dbb0993e8a3a1292ef4635526486a7af9020a+0x50>
10002e8b4: 528acf15    	mov	w21, #0x5678            ; =22136
10002e8b8: 2a1f03fa    	mov	w26, wzr
10002e8bc: 9127c294    	add	x20, x20, #0x9f0
10002e8c0: 72a24695    	movk	w21, #0x1234, lsl #16
10002e8c4: c8dffee8    	ldar	x8, [x23]
10002e8c8: 88dfff29    	ldar	w9, [x25]
10002e8cc: 9100230b    	add	x11, x24, #0x8
10002e8d0: 88dfff0a    	ldar	w10, [x24]
10002e8d4: c8dffd6b    	ldar	x11, [x11]
10002e8d8: 7100013f    	cmp	w9, #0x0
10002e8dc: 7a410940    	ccmp	w10, #0x1, #0x0, eq
10002e8e0: fa480160    	ccmp	x11, x8, #0x0, eq
10002e8e4: 540000c0    	b.eq	0x10002e8fc <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xd8>
10002e8e8: f9401be8    	ldr	x8, [sp, #0x30]
10002e8ec: f9000be8    	str	x8, [sp, #0x10]
10002e8f0: 94001f3f    	bl	0x1000365ec <_scoop_rt_safepoint>
10002e8f4: f9400be8    	ldr	x8, [sp, #0x10]
10002e8f8: f9001be8    	str	x8, [sp, #0x30]
10002e8fc: 6b13035f    	cmp	w26, w19
10002e900: 5400036a    	b.ge	0x10002e96c <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x148>
10002e904: f9401bf6    	ldr	x22, [sp, #0x30]
10002e908: aa1403e1    	mov	x1, x20
10002e90c: aa1603e0    	mov	x0, x22
10002e910: 9400176b    	bl	0x1000346bc <_scoop_rt_is_instance>
10002e914: 360003c0    	tbz	w0, #0x0, 0x10002e98c <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x168>
10002e918: f94002c0    	ldr	x0, [x22]
10002e91c: aa1403e1    	mov	x1, x20
10002e920: 940017cd    	bl	0x100034854 <_scoop_rt_itable_lookup>
10002e924: f90013f6    	str	x22, [sp, #0x20]
10002e928: 2a1503e1    	mov	w1, w21
10002e92c: f94013e8    	ldr	x8, [sp, #0x20]
10002e930: f9000fe8    	str	x8, [sp, #0x18]
10002e934: f9400fe8    	ldr	x8, [sp, #0x18]
10002e938: f9400fe8    	ldr	x8, [sp, #0x18]
10002e93c: f9401be9    	ldr	x9, [sp, #0x30]
10002e940: f9001fe8    	str	x8, [sp, #0x38]
10002e944: f940000a    	ldr	x10, [x0]
10002e948: aa0803e0    	mov	x0, x8
10002e94c: a900a7e8    	stp	x8, x9, [sp, #0x8]
10002e950: d63f0140    	blr	x10
10002e954: a940a3e9    	ldp	x9, x8, [sp, #0x8]
10002e958: 2a0003f5    	mov	w21, w0
10002e95c: f9001be8    	str	x8, [sp, #0x30]
10002e960: f9001fe9    	str	x9, [sp, #0x38]
10002e964: 1100075a    	add	w26, w26, #0x1
10002e968: 17ffffd7    	b	0x10002e8c4 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xa0>
10002e96c: 2a1503e0    	mov	w0, w21
10002e970: a9487bfd    	ldp	x29, x30, [sp, #0x80]
10002e974: a9474ff4    	ldp	x20, x19, [sp, #0x70]
10002e978: a94657f6    	ldp	x22, x21, [sp, #0x60]
10002e97c: a9455ff8    	ldp	x24, x23, [sp, #0x50]
10002e980: a94467fa    	ldp	x26, x25, [sp, #0x40]
10002e984: 910243ff    	add	sp, sp, #0x90
10002e988: d65f03c0    	ret
10002e98c: f0000940    	adrp	x0, 0x100159000 <_scoop_gc_marker+0x800>
10002e990: b000028a    	adrp	x10, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002e994: 9116e000    	add	x0, x0, #0x5b8
10002e998: 913cc14a    	add	x10, x10, #0xf30
10002e99c: f9400008    	ldr	x8, [x0]
10002e9a0: f9400d49    	ldr	x9, [x10, #0x18]
10002e9a4: d63f0100    	blr	x8
10002e9a8: f9400008    	ldr	x8, [x0]
10002e9ac: cb0903eb    	neg	x11, x9
10002e9b0: f940010c    	ldr	x12, [x8]
10002e9b4: 8b0c012c    	add	x12, x9, x12
10002e9b8: d100058c    	sub	x12, x12, #0x1
10002e9bc: ea0b0193    	ands	x19, x12, x11
10002e9c0: 540002c0    	b.eq	0x10002ea18 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
10002e9c4: 91005d2c    	add	x12, x9, #0x17
10002e9c8: 8a0b0182    	and	x2, x12, x11
10002e9cc: 528ff00b    	mov	w11, #0x7f80            ; =32640
10002e9d0: eb0b005f    	cmp	x2, x11
10002e9d4: 54000228    	b.hi	0x10002ea18 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
10002e9d8: f940494a    	ldr	x10, [x10, #0x90]
10002e9dc: b50001ea    	cbnz	x10, 0x10002ea18 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
10002e9e0: f940050b    	ldr	x11, [x8, #0x8]
10002e9e4: 8b02026a    	add	x10, x19, x2
10002e9e8: eb0b015f    	cmp	x10, x11
10002e9ec: 54000168    	b.hi	0x10002ea18 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
10002e9f0: 92f0000b    	mov	x11, #0x7fffffffffffffff ; =9223372036854775807
10002e9f4: 8b0b0129    	add	x9, x9, x11
10002e9f8: b100613f    	cmn	x9, #0x18
10002e9fc: 540000e2    	b.hs	0x10002ea18 <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
10002ea00: f900010a    	str	x10, [x8]
10002ea04: b0000281    	adrp	x1, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002ea08: aa1303e0    	mov	x0, x19
10002ea0c: 913cc021    	add	x1, x1, #0xf30
10002ea10: 94000d81    	bl	0x100032014 <_scoop_runtime_finish_tlab_alloc>
10002ea14: 14000006    	b	0x10002ea2c <_scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x208>
10002ea18: b0000280    	adrp	x0, 0x10007f000 <_scoop$1$td$3f8d512cd2cb3bd302cd11941672da1311589cdb6305c3926845df239da30f7f+0x70>
10002ea1c: 52800301    	mov	w1, #0x18               ; =24
10002ea20: 913cc000    	add	x0, x0, #0xf30
10002ea24: 94001ef6    	bl	0x1000365fc <_scoop_runtime_alloc_slow>
10002ea28: aa0003f3    	mov	x19, x0
10002ea2c: aa1303e0    	mov	x0, x19
10002ea30: f9000bf3    	str	x19, [sp, #0x10]
10002ea34: 97ff8069    	bl	0x10000ebd8 <_scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
10002ea38: f9400be0    	ldr	x0, [sp, #0x10]
10002ea3c: f90017e0    	str	x0, [sp, #0x28]
10002ea40: 94002468    	bl	0x100037be0 <_scoop_rt_throw>
