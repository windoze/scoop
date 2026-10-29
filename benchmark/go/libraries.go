package main

import (
	"strconv"
	"strings"
	"sync"
)

func growList(n int, seed int64) int64 {
	list := make([]int64, 0)
	for i := 0; i < n; i++ {
		list = append(list, int64(i)+seed)
	}
	var sum int64
	for i := 0; i < n; i++ {
		sum += list[i]
	}
	escaped = list
	return sum
}

func hashMap(n int, seed int64) int64 {
	m := make(map[int64]int64)
	var sum int64
	for i := 0; i < n; i++ {
		key := (int64(i)*17 + seed) & 65535
		m[key] = m[key] + 1
		sum += m[key]
	}
	escaped = m
	return sum
}

func decimal(n int, seed int64) int64 {
	ring := make([]string, 256)
	escaped = ring
	x := uint32(seed)
	var sum int64
	for i := 0; i < n; i++ {
		x = x*1664525 + 1013904223
		text := strconv.FormatInt(int64(x), 10)
		ring[i&255] = text
		sum += int64(len(text))
	}
	return sum + int64(ring[(n-1)&255][0])
}

func buildString(n int, seed int64) int64 {
	var builder strings.Builder
	for i := 0; i < n; i++ {
		builder.WriteString(strconv.FormatInt(int64(i)^seed, 10))
		builder.WriteString(",")
	}
	text := builder.String()
	escaped = text
	return int64(len(text)) + int64(text[0])
}

func unicode(n int, seed int64) int64 {
	var builder strings.Builder
	for i := 0; i < 512; i++ {
		builder.WriteString("abé雪🙂z")
	}
	builder.WriteString(strconv.FormatInt(seed, 10))
	text := builder.String()
	var sum int64
	for i := 0; i < n; i++ {
		for _, c := range text {
			sum += int64(c)
		}
	}
	return sum
}

func parseIntegers(n int, seed int64) int64 {
	texts := make([]string, 256)
	for i := range texts {
		texts[i] = strconv.FormatInt(int64(i)*1234567+seed, 10)
	}
	var sum int64
	for i := 0; i < n; i++ {
		value, err := strconv.ParseInt(texts[i&255], 10, 64)
		if err != nil {
			panic(err)
		}
		sum += value
	}
	return sum
}

func parallel(n int, seed int64, workers int) int64 {
	results := make([]int64, 8)
	var wg sync.WaitGroup
	for worker := 0; worker < workers; worker++ {
		wg.Add(1)
		go func(worker int) {
			defer wg.Done()
			for shard := worker; shard < 8; shard += workers {
				results[shard] = mix(n/8, seed+int64(shard))
			}
		}(worker)
	}
	wg.Wait()
	var sum int64
	for _, value := range results {
		sum += value
	}
	return sum
}
