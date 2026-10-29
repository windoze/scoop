package main

func mix(n int, seed int64) int64 {
	x := uint32(seed)
	for i := 0; i < n; i++ {
		x ^= x << 13
		x ^= x >> 17
		x ^= x << 5
	}
	return int64(x)
}

func mandelbrot(n int, seed int64) int64 {
	var total int64
	for y := 0; y < n; y++ {
		for x := 0; x < n; x++ {
			cr := float64(x)*3.0/float64(n) - 2.0 + float64(seed)*0.000001
			ci := float64(y)*2.0/float64(n) - 1.0
			zr, zi := 0.0, 0.0
			step := 0
			for step < 64 && float64(zr*zr)+float64(zi*zi) <= 4.0 {
				next := float64(float64(zr*zr)-float64(zi*zi)) + cr
				zi = float64(float64(2.0*zr)*zi) + ci
				zr = next
				step++
			}
			total += int64(step)
		}
	}
	return total
}

func arrayTransform(n int, seed int64) int64 {
	a, b, c := make([]float64, n), make([]float64, n), make([]float64, n)
	for i := 0; i < n; i++ {
		a[i] = float64(int64(i&1023)+seed) * 0.25
	}
	for i := 0; i < n; i++ {
		b[i] = float64(i&255) * 0.5
	}
	for i := 0; i < n; i++ {
		c[i] = float64(a[i]*1.25) + b[i]
	}
	sum := 0.0
	for i := 0; i < n; i++ {
		sum += c[i]
	}
	return int64(sum * 16.0)
}

func branchScan(n int, seed int64) int64 {
	a := make([]uint32, n)
	state := uint32(seed)
	for i := 0; i < n; i++ {
		state ^= state << 13
		state ^= state >> 17
		state ^= state << 5
		a[i] = state
	}
	var sum int64
	for i := 0; i < n; i++ {
		x := a[i]
		if x&1 == 0 {
			sum += int64(x & 1023)
		} else {
			sum -= int64(x & 1023)
		}
	}
	return sum
}

func pointerChase(n int, seed int64) int64 {
	next := make([]int32, n)
	for i := 0; i < n; i++ {
		next[i] = int32((int64(i)*1664525 + 1013904223) & int64(n-1))
	}
	at := int32(seed & int64(n-1))
	var sum int64
	for i := 0; i < n*8; i++ {
		at = next[at]
		sum += int64(at)
	}
	return sum & 2147483647
}

func quicksort(a []int32, low, high int) {
	i, j := low, high
	pivot := a[(low+high)/2]
	for i <= j {
		for a[i] < pivot {
			i++
		}
		for a[j] > pivot {
			j--
		}
		if i <= j {
			a[i], a[j] = a[j], a[i]
			i++
			j--
		}
	}
	if low < j {
		quicksort(a, low, j)
	}
	if i < high {
		quicksort(a, i, high)
	}
}

func sortValues(n int, seed int64) int64 {
	a := make([]int32, n)
	state := uint32(seed)
	for i := 0; i < n; i++ {
		state ^= state << 13
		state ^= state >> 17
		state ^= state << 5
		a[i] = int32(state & 2147483647)
	}
	quicksort(a, 0, n-1)
	var sum int64
	for i := 0; i < n; i++ {
		sum += int64(a[i]) * int64(i%257)
	}
	return sum
}
