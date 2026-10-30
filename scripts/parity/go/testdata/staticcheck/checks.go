package probe

import (
	"errors"
	"fmt"
	"log"
	"os"
	"strings"
	"sync"
)

type flags int

const (
	flagNone flags = iota
	flagOne
)

const alone = iota

func dynamicFormats(message string, logger *log.Logger) error {
	fmt.Printf(message)
	fmt.Printf("%s", message)
	fmt.Printf("constant")
	_ = fmt.Sprintf(strings.ToUpper(message))
	fmt.Fprintf(os.Stdout, message)
	logger.Printf(message)
	log.Printf((message))
	_ = errors.New(message)
	return fmt.Errorf(message)
}

type guarded struct {
	mu   sync.Mutex
	rw   sync.RWMutex
	data int
}

func (g *guarded) sections() {
	g.mu.Lock()
	g.mu.Unlock()
	g.rw.RLock()
	g.rw.RUnlock()
	g.mu.Lock()
	g.data++
	g.mu.Unlock()
	g.rw.Lock()
	g.rw.RUnlock()
}

func loops(items []int, table map[int]int) int {
	for _, item := range items {
		if item > 0 {
			fmt.Println(item)
		}
		return item
	}
	for range table {
		if len(table) > 0 {
			fmt.Println()
		}
		break
	}
	for i := 0; i < 3; i++ {
		if i > 1 {
			continue
		}
		break
	}
outer:
	for _, item := range items {
		for range items {
			if item > 0 {
				break outer
			}
		}
		break outer
	}
	for _, item := range items {
		fmt.Println(item)
		return item
	}
	return 0
}

func breaks(items []int, channel chan int) {
	for _, item := range items {
		switch item {
		case 1:
			fmt.Println()
			break
		case 2:
			if item > 0 {
				break
			} else {
				break
			}
		}
		select {
		case <-channel:
			break
		}
	}
	switch {
	case true:
		break
	}
}

func bits(value int, flag flags) {
	_ = value & 0
	_ = value | 0
	_ = value ^ 0
	_ = flag & flagNone
	_ = flag | flagOne
	_ = value & alone
	_ = value + 0
	const local = iota
	_ = value | local
}

func pools(pool *sync.Pool, buffer []byte, pointer *int, anything any) {
	pool.Put(buffer)
	pool.Put(pointer)
	pool.Put(anything)
	pool.Put(3)
	pool.Put(nil)
	pool.Put(struct{}{})
}

func comparisons(first, second string) bool {
	if strings.ToLower(first) == strings.ToLower(second) {
		return true
	}
	if strings.ToUpper(first) != strings.ToUpper(second) {
		return false
	}
	return strings.ToLower(first) == strings.ToUpper(second)
}

// go:noinline
func spaced() {}

//  go:generate echo hi
//go:noinline
func directive() {}

func errorFirst() (error, int)        { return nil, 0 }
func errorMiddle() (int, error, bool) { return 0, nil, false }
func errorNamed() (err error, n int)  { return nil, 0 }
func errorLast() (int, error)         { return 0, nil }
func errorTwice() (error, error, int) {
	inner := func() (error, string) { return nil, "" }
	_, _ = inner()
	return nil, nil, 0
}

func merges(condition bool) (bool, bool, bool) {
	first := false
	if condition {
		first = true
	}
	second := true
	if condition {
		second = false
	}
	third := true
	if condition {
		third = true
	}
	return first, second, third
}
