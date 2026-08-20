; compute, MIR fn5, LIR scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-poll-on-darwin/poll:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002e9dc <_scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c>:
10002e9dc: a9bc5ff8    	stp	x24, x23, [sp, #-0x40]!
10002e9e0: a90157f6    	stp	x22, x21, [sp, #0x10]
10002e9e4: a9024ff4    	stp	x20, x19, [sp, #0x20]
10002e9e8: a9037bfd    	stp	x29, x30, [sp, #0x30]
10002e9ec: 9100c3fd    	add	x29, sp, #0x30
10002e9f0: f0000948    	adrp	x8, 0x100159000 <_scoop_gc_marker+0x800>
10002e9f4: 2a0003f3    	mov	w19, w0
10002e9f8: 91174108    	add	x8, x8, #0x5d0
10002e9fc: f9400109    	ldr	x9, [x8]
10002ea00: aa0803e0    	mov	x0, x8
10002ea04: d63f0120    	blr	x9
10002ea08: d0001595    	adrp	x21, 0x1002e0000 <_pauses+0xc3300>
10002ea0c: d0001597    	adrp	x23, 0x1002e0000 <_pauses+0xc3300>
10002ea10: 910922b5    	add	x21, x21, #0x248
10002ea14: f9400016    	ldr	x22, [x0]
10002ea18: 9108d2f7    	add	x23, x23, #0x234
10002ea1c: 910022ca    	add	x10, x22, #0x8
10002ea20: c8dffea8    	ldar	x8, [x21]
10002ea24: 88dffeeb    	ldar	w11, [x23]
10002ea28: 88dffec9    	ldar	w9, [x22]
10002ea2c: c8dffd4a    	ldar	x10, [x10]
10002ea30: 3500008b    	cbnz	w11, 0x10002ea40 <_scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x64>
10002ea34: 7100053f    	cmp	w9, #0x1
10002ea38: fa480140    	ccmp	x10, x8, #0x0, eq
10002ea3c: 54000040    	b.eq	0x10002ea44 <_scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x68>
10002ea40: 94001ba7    	bl	0x1000358dc <_scoop_rt_safepoint>
10002ea44: aa1f03f4    	mov	x20, xzr
10002ea48: 52800038    	mov	w24, #0x1               ; =1
10002ea4c: c8dffea8    	ldar	x8, [x21]
10002ea50: 88dffee9    	ldar	w9, [x23]
10002ea54: 910022cb    	add	x11, x22, #0x8
10002ea58: 88dffeca    	ldar	w10, [x22]
10002ea5c: c8dffd6b    	ldar	x11, [x11]
10002ea60: 7100013f    	cmp	w9, #0x0
10002ea64: 7a410940    	ccmp	w10, #0x1, #0x0, eq
10002ea68: fa480160    	ccmp	x11, x8, #0x0, eq
10002ea6c: 54000040    	b.eq	0x10002ea74 <_scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x98>
10002ea70: 94001b9b    	bl	0x1000358dc <_scoop_rt_safepoint>
10002ea74: d1000708    	sub	x8, x24, #0x1
10002ea78: 6b13011f    	cmp	w8, w19
10002ea7c: 5400008a    	b.ge	0x10002ea8c <_scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0xb0>
10002ea80: 8b180294    	add	x20, x20, x24
10002ea84: 91000718    	add	x24, x24, #0x1
10002ea88: 17fffff1    	b	0x10002ea4c <_scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x70>
10002ea8c: aa1403e0    	mov	x0, x20
10002ea90: a9437bfd    	ldp	x29, x30, [sp, #0x30]
10002ea94: a9424ff4    	ldp	x20, x19, [sp, #0x20]
10002ea98: a94157f6    	ldp	x22, x21, [sp, #0x10]
10002ea9c: a8c45ff8    	ldp	x24, x23, [sp], #0x40
10002eaa0: d65f03c0    	ret
