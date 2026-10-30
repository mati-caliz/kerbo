package probe

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"sync"
	"testing"
	"time"
)

func compares(first, second []byte) bool {
	if bytes.Compare(first, second) == 0 {
		return true
	}
	return bytes.Compare(first, second) != 0 || bytes.Compare(first, second) > 0
}

func forever() {
	for true {
	}
	for false {
	}
	for {
	}
}

func slices(items []int, nested [][]int) {
	_ = items[1:len(items)]
	_ = items[:len(items)]
	_ = nested[0][1:len(nested[0])]
	_ = items[1:len(items):len(items)]
}

func times(start time.Time) {
	_ = time.Now().Sub(start)
	_ = start.Sub(time.Now())
	_ = start.Sub(start)
	_ = errors.New(fmt.Sprintf("%d", 1))
	_ = errors.New("x")
}

func redundant(value int) {
	switch value {
	case 1:
		fmt.Println()
		break
	case 2:
		break
	}
	switch any(value).(type) {
	case int:
		fmt.Println()
		break
	}
	func() {
		fmt.Println()
		return
	}()
	return
}

func withResult() int {
	return 0
}

func deletes(table map[string]int, key string) {
	if _, ok := table[key]; ok {
		delete(table, key)
	}
	if _, found := table[key]; found {
		delete(table, "other")
	}
	if _, ok := table[key]; ok {
		delete(table, key)
	} else {
		fmt.Println()
	}
}

func sleeps(duration time.Duration, done chan bool) {
	select {
	case <-time.After(duration):
	}
	select {
	case <-time.After(duration):
		fmt.Println()
	}
	select {
	case <-time.After(duration):
	case <-done:
	}
}

func takesContext(ctx context.Context, value int) {}

func contexts() {
	takesContext(nil, 1)
	takesContext(context.TODO(), 1)
}

func seeks(file *os.File) {
	_, _ = file.Seek(io.SeekStart, 0)
	_, _ = file.Seek(0, io.SeekStart)
}

func goroutines(group *sync.WaitGroup) {
	go func() {
		group.Add(1)
		defer group.Done()
	}()
	group.Add(1)
	go func() {
		defer group.Done()
	}()
}

func BenchmarkProbe(b *testing.B) {
	b.N = 10
}

func copies(value *int, number int) {
	_ = &*value
	_ = *&number
	_ = !!(number > 0)
	_ = !(!(number > 0))
	items := []int{}
	_ = append(items)
	_ = append(items, 1)
	_ = &number == nil
	_ = &number != nil
	_ = len(items) < 0
	_ = 0 > cap(items)
	_ = len(items) <= 0
	_ = number % 1
	_ = number % 2
}

type raw []byte

func writes(writer io.Writer, data []byte, other raw, text string) {
	_, _ = io.WriteString(writer, string(data))
	_, _ = io.WriteString(writer, string(other))
	_, _ = io.WriteString(writer, text)
}
