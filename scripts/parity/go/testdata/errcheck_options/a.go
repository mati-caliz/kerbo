package options

import (
	"errors"
	"fmt"
	"os"
)

func pair() (int, error) { return 0, errors.New("x") }

func single() error { return nil }

func channel() chan error { return nil }

var _ = single()

var _, _ = pair()

var value, _ = pair()

var _ error = (*customError)(nil)

type customError struct{}

func (*customError) Error() string { return "" }

func blanks() {
	_ = single()
	_, _ = pair()
	number, _ := pair()
	_ = number
	_, err := pair()
	_ = err
	_, _ = single(), pair2()
	_ = <-channel()
	_ = (<-channel())
	_, _ = <-channel(), single()
	_ = recover()
	_ = fmt.Sprint("x")
	_, _ = fmt.Println("excluida")
	_ = os.Remove("x")
	defer func() {
		_ = single()
	}()
}

func pair2() error { return nil }

func assertions(any1 any, any2 any) {
	any1.(fmt.Stringer).String()
	first := any1.(fmt.Stringer)
	_ = first
	second, ok := any1.(fmt.Stringer)
	_, _ = second, ok
	third, _ := any1.(fmt.Stringer)
	_ = third
	_, _ = any1.(fmt.Stringer)
	fourth, fifth := any1.(fmt.Stringer), any2.(fmt.Stringer)
	_, _ = fourth, fifth
	_ = fmt.Sprint(any1.(fmt.Stringer))
	var sixth = any1.(fmt.Stringer)
	_ = sixth
	var seventh, _ = any1.(fmt.Stringer)
	_ = seventh
	switch typed := any1.(type) {
	case fmt.Stringer:
		_ = typed
	}
	nested := any1.(interface{ Inner() any }).Inner().(fmt.Stringer)
	_ = nested
}
