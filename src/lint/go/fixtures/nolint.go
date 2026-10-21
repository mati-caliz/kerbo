// Casos de nolint contra golangci-lint.
package fixture

import "fmt"

func trailing() {
	x := 1 //nolint:ineffassign // se pisa abajo
	x = 2
	fmt.Println(x)
}

func otherLinter() {
	x := 1 //nolint:errcheck
	x = 2
	fmt.Println(x)
}

func everything() {
	x := 1 //nolint
	x = 2
	fmt.Println(x)
}

func allKeyword() {
	x := 1 //nolint:all
	x = 2
	fmt.Println(x)
}

func spaced() {
	x := 1 // nolint: ineffassign
	x = 2
	fmt.Println(x)
}

//nolint:ineffassign
func wholeFunction() {
	x := 1
	x = 2
	fmt.Println(x)
}

func statementBlock(flag bool) {
	//nolint:ineffassign // todo el if
	if flag {
		y := 1
		y = 2
		fmt.Println(y)
	}
	z := 1
	z = 2
	fmt.Println(z)
}

func misaligned(flag bool) {
		//nolint:ineffassign
	if flag {
		y := 1
		y = 2
		fmt.Println(y)
	}
}

func grouped() {
	// explicación previa
	//nolint:ineffassign
	// y otra línea
	w := 1
	w = 2
	fmt.Println(w)
}

func separated() {
	//nolint:ineffassign

	v := 1
	v = 2
	fmt.Println(v)
}

func blockComment() {
	u := 1 /* nolint */
	u = 2
	fmt.Println(u)
}

func notADirective() {
	t := 1 // nolintx
	t = 2
	fmt.Println(t)
}
