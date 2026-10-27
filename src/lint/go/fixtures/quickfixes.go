package quickfixes

import (
	"bytes"
	str "strings"
)

type token struct{ kind int }

func next() int { return 0 }

func chains(x, y int, t token, values []int, channel chan int, flag bool) {
	if x == 1 {
		println("one")
	} else if x == 2 || x == 3 {
		println("two or three")
	} else {
		println("other")
	}

	if x == 1 {
		println("alone")
	}

	if (t.kind) == 1 {
		println("one")
	} else if (t.kind == 2) {
		println("two")
	}

	if t.kind == 1 {
		println("one")
	} else if t.kind == 2 {
		println("two")
	} else if y == 3 {
		println("three")
	}

	if x == next() {
		println("call")
	} else if x == 2 {
		println("two")
	}

	if x := next(); x == 1 {
		println("init")
	} else if x == 2 {
		println("two")
	}

	for _, value := range values {
		if value == 1 {
			continue
		} else if value == 2 {
			println("two")
		}
		if value == 1 {
			println("one")
		} else if value == 2 {
			goto done
		}
	done:
		if value == 1 {
			println("one")
		} else if value == 2 {
			println("two")
		} else {
			break
		}
	}

	if values[0] == 1 {
		println("one")
	} else if values[0] == 2 || values[1] == 3 {
		println("mixed")
	}

	if values[0] == 1 {
		println("one")
	} else if values[0] == 2 && flag {
		println("and")
	}

	if 1 == x {
		println("reversed")
	} else if 2 == x {
		println("reversed")
	}

	if <-channel == 1 {
		println("receive")
	} else if <-channel == 2 {
		println("receive")
	}

	if x == 1 {
		println("one")
	} else {
		if x == 2 {
			println("nested")
		} else if x == 3 {
			println("nested three")
		}
	}

	if x == 1 {
		func() {
			for {
				break
			}
		}()
	} else if x == 2 {
		println("two")
	}
}

func replacements(text string, data []byte) {
	_ = str.Replace(text, "a", "b", -1)
	_ = str.Replace(text, "a", "b", - 1)
	_ = str.Replace(text, "a", "b", 1)
	_ = str.SplitN(text, ",", -1)
	_ = str.SplitAfterN(text, ",", -1)
	_ = (str.Replace)(text, "a", "b", -1)
	_ = bytes.Replace(data, []byte("a"), []byte("b"), -1)
	_ = bytes.SplitN(data, []byte(","), -1)
	_ = bytes.SplitAfterN(data, []byte(","), -1 /* todo */)
	_ = str.Replace(text, "a", "b", -0x1)
	_ = str.Replace(text, "a", "b", (-1))
	_ = str.Count(text, "-1")
}
