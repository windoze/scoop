; compute, MIR fn5, LIR scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c

/Volumes/Data/home/chenxu/repos/scoop/tmp/m34/final-poll-off-darwin/poll:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001df88 <_scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c>:
10001df88: a9bd57f6    	stp	x22, x21, [sp, #-0x30]!
10001df8c: a9014ff4    	stp	x20, x19, [sp, #0x10]
10001df90: a9027bfd    	stp	x29, x30, [sp, #0x20]
10001df94: 910083fd    	add	x29, sp, #0x20
10001df98: 2a0003f3    	mov	w19, w0
10001df9c: 94001e6a    	bl	0x100025944 <_scoop_rt_safepoint>
10001dfa0: aa1f03f4    	mov	x20, xzr
10001dfa4: 52800035    	mov	w21, #0x1               ; =1
10001dfa8: 510006b6    	sub	w22, w21, #0x1
10001dfac: 94001e66    	bl	0x100025944 <_scoop_rt_safepoint>
10001dfb0: 6b1302df    	cmp	w22, w19
10001dfb4: 5400008a    	b.ge	0x10001dfc4 <_scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x3c>
10001dfb8: 8b150294    	add	x20, x20, x21
10001dfbc: 910006b5    	add	x21, x21, #0x1
10001dfc0: 17fffffa    	b	0x10001dfa8 <_scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x20>
10001dfc4: aa1403e0    	mov	x0, x20
10001dfc8: a9427bfd    	ldp	x29, x30, [sp, #0x20]
10001dfcc: a9414ff4    	ldp	x20, x19, [sp, #0x10]
10001dfd0: a8c357f6    	ldp	x22, x21, [sp], #0x30
10001dfd4: d65f03c0    	ret
