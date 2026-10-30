package probe

import (
	"bytes"
	"fmt"
	"log"
	"net/http"
	"sort"
	"strings"
)

type names []string

const zero = 0

func nilChecks(items []int, table map[string]int, channel chan int, pointer *[3]int, list names) {
	if items == nil || len(items) == 0 {
	}
	if table != nil && len(table) > 0 {
	}
	if channel != nil && len(channel) != zero {
	}
	if items == nil || len(items) < 0 {
	}
	if items != nil && len(items) == 0 {
	}
	if pointer == nil || len(pointer) == 0 {
	}
	if list == nil || len(list) <= 2 {
	}
	if (items == nil) || (len(items) == 0) {
	}
	if items == nil || len(items) == 3 {
	}
}

func nilCheckGeneric[S ~[]int](items S) bool {
	return items == nil || len(items) == 0
}

func appends(source []int, other [][]int) []int {
	var target []int
	for _, value := range source {
		target = append(target, value)
	}
	for index := range source {
		target = append(target, source[index])
	}
	for index := range source {
		element := source[index]
		target = append(target, element)
	}
	for _, value := range source {
		other[value] = append(other[value], value)
	}
	var floats []float64
	for _, value := range source {
		floats = append(floats, float64(value))
	}
	for index := range other[0] {
		target = append(target, other[0][index])
	}
	return target
}

func trims(text string, data []byte) (string, []byte) {
	if strings.HasPrefix(text, "abc") {
		text = text[len("abc"):]
	}
	if strings.HasPrefix(text, "abc") {
		text = text[3:]
	}
	if strings.HasSuffix(text, "x") {
		text = text[:len(text)-len("x")]
	}
	if strings.HasPrefix(text, "abc") {
		text = strings.TrimPrefix(text, "abc")
	}
	if strings.Contains(text, "a") {
		text = strings.Replace(text, "a", "b", -1)
	}
	if bytes.HasPrefix(data, []byte("a")) {
		data = data[1:]
	}
	if bytes.HasSuffix(data, data) {
		data = data[:len(data)-len(data)]
	}
	if strings.HasPrefix(text, "abc") {
		text = text[4:]
	} else if strings.HasPrefix(text, "x") {
		text = text[1:]
	}
	return text, data
}

func runes(text string, named names) {
	for _, r := range []rune(text) {
		_ = r
	}
	for i, r := range []rune(text) {
		_, _ = i, r
	}
	for _, r := range []rune("literal") {
		_ = r
	}
	for _, s := range named {
		_ = s
	}
}

func buffers(buffer *bytes.Buffer) {
	_ = string(buffer.Bytes())
	_ = []byte(buffer.String())
	_ = string(buffer.Bytes())[0]
	_ = string((buffer.Bytes()))
}

func sorts(numbers []int, words []string, floats []float64) {
	sort.Sort(sort.IntSlice(numbers))
	sort.Sort(sort.StringSlice(words))
	func() {
		sort.Sort(sort.Float64Slice(floats))
	}()
}

func sortsPermissive(numbers []int, words names) {
	sort.Sort(sort.IntSlice(numbers))
	sort.Sort(sort.Reverse(sort.StringSlice(words)))
}

func (n names) Len() int           { return len(n) }
func (n names) Less(i, j int) bool { return n[i] < n[j] }
func (n names) Swap(i, j int)      { n[i], n[j] = n[j], n[i] }

func headers(header http.Header, name string) {
	header.Set(http.CanonicalHeaderKey(name), "x")
	_ = header.Get(http.CanonicalHeaderKey(name))
	header.Add(name, "y")
	type wrapper struct{ http.Header }
	w := wrapper{header}
	w.Del(http.CanonicalHeaderKey(name))
}

func prints(writer *bytes.Buffer, logger *log.Logger, value int) {
	fmt.Print(fmt.Sprintf("%d", value))
	fmt.Println(fmt.Sprintf("%d", value))
	format := "%d"
	fmt.Println(fmt.Sprintf(format, value))
	fmt.Fprint(writer, fmt.Sprintf("%d", value))
	_ = fmt.Sprint(fmt.Sprintf("%d", value))
	log.Print(fmt.Sprintf("%d", value))
	log.Println(fmt.Sprintf("%d", value))
	logger.Print(fmt.Sprintf("%d", value))
	logger.Fatalln(fmt.Sprintf("%d", value))
	_ = fmt.Sprint("literal")
	_ = fmt.Sprintf("literal")
	_ = fmt.Sprintf("100%%")
	_ = fmt.Sprint("a", "b")
}

type shape interface{ Area() float64 }

func assertions(value shape, anything any) {
	_ = value.(shape)
	_ = anything.(shape)
	_, _ = value.(shape)
}
