package fixture

import (
	"errors"
	"fmt"
)

var global = 1

func value() int { return 2 }

func reassigned() {
	x := value()
	x = value()
	fmt.Println(x)
	y := 3
	y = 4
	_ = global
	var n int = 5
	n = 6
	fmt.Println(n, y)
}

func zeroInitializers() {
	a := 0
	b := ""
	c := false
	d := int64(0)
	e := (*int)(nil)
	a, b, c, d, e = 1, "x", true, 2, nil
	fmt.Println(a, b, c, d, e)
}

func branches(flag bool) int {
	result := 1
	if flag {
		result = 2
	} else {
		result = 3
	}
	return result
}

func loops(items []int) (total int) {
	count := 0
	for _, item := range items {
		count++
		total += item
	}
	last := 0
	for index := 0; index < len(items); index++ {
		last = items[index]
		if last > 10 {
			break
		}
	}
	return
}

func labeled(rows [][]int) int {
	found := -1
outer:
	for _, row := range rows {
		for _, cell := range row {
			if cell == 0 {
				found = cell
				continue outer
			}
			found = 1
			break outer
		}
	}
	return found
}

func switches(kind int) string {
	label := "none"
	switch kind {
	case 1:
		label = "one"
		fallthrough
	case 2:
		label = "two"
	default:
		label = "other"
	}
	return label
}

func typeSwitch(value any) string {
	text := ""
	switch typed := value.(type) {
	case string:
		text = typed
	case int:
		text = fmt.Sprint(typed)
	}
	return text
}

func selects(left, right chan int) int {
	received := 0
	select {
	case received = <-left:
	case number := <-right:
		received = number
	}
	return received
}

func closures() func() int {
	captured := 1
	captured = 2
	return func() int { return captured }
}

func escapes() *int {
	pointer := 1
	pointer = 2
	return &pointer
}

func methodReceiverEscapes() {
	var builder fmt.Stringer
	builder = nil
	_ = builder.String
}

func deferred() (err error) {
	defer func() {
		if recovered := recover(); recovered != nil {
			err = errors.New("panic")
		}
	}()
	err = errors.New("first")
	fmt.Println("work")
	return nil
}

func gotos(limit int) int {
	counter := 0
loop:
	counter++
	if counter < limit {
		goto loop
	}
	counter = 0
	return limit
}

func shadowed() {
	value := 1
	{
		value := 2
		fmt.Println(value)
	}
	value = 3
}

func compositeKeys() map[string]int {
	key := 1
	key = 2
	return map[string]int{"key": 1}
}

type point struct{ x, y int }

func structKeys() point {
	x := 1
	x = 2
	return point{x: 3}
}

func laterGlobal() {
	declaredLater = 1
	declaredLater = 2
}

var declaredLater int

func generic[T any](items []T) T {
	var first T
	first = items[0]
	first = items[1]
	return first
}
