package p

import "unsafe"

type T struct{ f int }

func deref(q unsafe.Pointer, s string) {
	*(*struct {
		p *byte
	})(unsafe.Pointer(&s)).p = 'H'
	*(*T)(q).f=2
	_ = *(*T)(q)
	_ = *[]int(nil)
	_ = (*T)(q).f
}

func invalid(x any) {
	_ = ~x
	_ = ~ x+^x
	_ = x.(type)
	f[x /* a */ /* b */ .(type)]()
	switch x.( type ) {
	case int:
	}
}

var _ = struct{ A }.m
var _ = (*struct{ J }).method
var _ = (*T).m
var _ = (x.y).f
