package main

import (
	"fmt"
	"os"
	"strconv"
	"time"
)

func config(name string, fallback int64) int64 {
	if value := os.Getenv(name); value != "" {
		result, err := strconv.ParseInt(value, 10, 64)
		if err != nil {
			panic(err)
		}
		return result
	}
	return fallback
}

func kernel(kind int64, n int, seed int64) int64 {
	switch kind {
	case 0:
		return mix(n, seed)
	case 1:
		return mandelbrot(n, seed)
	case 2:
		return arrayTransform(n, seed)
	case 3:
		return branchScan(n, seed)
	case 4:
		return pointerChase(n, seed)
	case 5:
		return sortValues(n, seed)
	case 6:
		return valueRecords(n, seed)
	case 7:
		return referenceRecords(n, seed)
	case 8:
		return allocationRing(n, seed)
	case 9:
		return trees(n, seed)
	case 10:
		return dispatch(n, seed)
	case 11:
		return growList(n, seed)
	case 12:
		return hashMap(n, seed)
	case 13:
		return decimal(n, seed)
	case 14:
		return buildString(n, seed)
	case 15:
		return unicode(n, seed)
	case 16:
		return parseIntegers(n, seed)
	case 17:
		return parallel(n, seed, 1)
	case 18:
		return parallel(n, seed, 4)
	case 19:
		return parallel(n, seed, 8)
	default:
		panic("unknown case")
	}
}

func main() {
	kind := config("BENCH_CASE", 0)
	if kind < 0 {
		fmt.Println("ready")
		return
	}
	n := int(config("BENCH_N", 1000))
	seed := config("BENCH_SEED", 42)
	for i := int64(0); i < 8; i++ {
		fmt.Printf("{\"type\":\"check\",\"seed\":%d,\"value\":%d}\n", seed+i, kernel(kind, n, seed+i))
	}
	for sample := int64(-1); sample < config("BENCH_SAMPLES", 5); sample++ {
		duration := config("BENCH_SAMPLE_MS", 200)
		if sample < 0 {
			duration = config("BENCH_WARMUP_MS", 500)
		}
		start := time.Now()
		var calls, checksum int64
		for time.Since(start).Nanoseconds() < duration*1000000 {
			checksum ^= kernel(kind, n, seed+(calls&7))
			calls++
		}
		elapsed := time.Since(start).Nanoseconds()
		fmt.Printf("{\"type\":\"sample\",\"index\":%d,\"elapsed_ns\":%d,\"calls\":%d,\"checksum\":%d}\n", sample, elapsed, calls, checksum)
	}
}
