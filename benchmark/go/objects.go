package main

type Particle struct{ x, y, z int64 }
type Box struct{ a, b int64 }

var escaped any

func valueRecords(n int, seed int64) int64 {
	a := make([]Particle, n)
	for i := 0; i < n; i++ {
		j := int64(i)
		a[i] = Particle{j + seed, j * 2, j * 3}
	}
	for i := 0; i < n; i++ {
		p := a[i]
		a[i] = Particle{p.y + 1, p.z + 2, p.x + 3}
	}
	var sum int64
	for i := 0; i < n; i++ {
		p := a[i]
		sum += p.x + p.y*2 + p.z*3
	}
	escaped = a
	return sum
}

func referenceRecords(n int, seed int64) int64 {
	a := make([]*Particle, n)
	for i := 0; i < n; i++ {
		j := int64(i)
		a[i] = &Particle{j + seed, j * 2, j * 3}
	}
	for i := 0; i < n; i++ {
		p := a[i]
		a[i] = &Particle{p.y + 1, p.z + 2, p.x + 3}
	}
	var sum int64
	for i := 0; i < n; i++ {
		p := a[i]
		sum += p.x + p.y*2 + p.z*3
	}
	escaped = a
	return sum
}

func allocationRing(n int, seed int64) int64 {
	zero := &Box{0, 0}
	ring := make([]*Box, 1024)
	for i := range ring {
		ring[i] = zero
	}
	escaped = ring
	var sum int64
	for i := 0; i < n; i++ {
		slot := i & 1023
		old := ring[slot]
		sum += old.a ^ old.b
		ring[slot] = &Box{int64(i), int64(i) + seed}
	}
	return sum + ring[(n-1)&1023].b
}

type Tree struct {
	left, right *Tree
	value       int64
}

func makeTree(depth int, seed int64) *Tree {
	if depth == 0 {
		return &Tree{nil, nil, seed & 1023}
	}
	return &Tree{makeTree(depth-1, seed+1), makeTree(depth-1, seed+2), 0}
}
func checkTree(tree *Tree) int64 {
	if tree.left == nil {
		return tree.value
	}
	return checkTree(tree.left) + checkTree(tree.right)
}
func trees(n int, seed int64) int64 {
	var sum int64
	for i := 0; i < n; i++ {
		root := makeTree(12, seed+int64(i))
		escaped = root
		sum += checkTree(root)
	}
	return sum
}

type Operation interface{ apply(int64) int64 }
type Add struct{ k int64 }
type Xor struct{ k int64 }
type Multiply struct{ k int64 }
type Shift struct{ k int64 }

func (o *Add) apply(x int64) int64      { return x + o.k }
func (o *Xor) apply(x int64) int64      { return x ^ o.k }
func (o *Multiply) apply(x int64) int64 { return x * o.k }
func (o *Shift) apply(x int64) int64    { return (x >> 1) + o.k }
func dispatch(n int, seed int64) int64 {
	ops := make([]Operation, 64)
	for i := range ops {
		k := int64(i) + seed
		switch i & 3 {
		case 0:
			ops[i] = &Add{k}
		case 1:
			ops[i] = &Xor{k}
		case 2:
			ops[i] = &Multiply{k}
		case 3:
			ops[i] = &Shift{k}
		}
	}
	x := seed
	for i := 0; i < n; i++ {
		x = ops[i&63].apply(x) & 2147483647
	}
	return x
}
