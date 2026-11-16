//Package docs has a comment without a space.
//It goes on.
//
//
// Old Heading
//
// A paragraph after the old heading, with ``quotes'' and a link to [the site]
// and http://example.com/a''b(c)d. and a ```fence``` and x``y.
//
// # New heading
//
//   indented code
//      deeper
//
//   more code
// Text right after code.
//
// A list:
//   * one
//   * two
//     continues
//   * three
//
// Numbered:
//  1) first
//  2. second
//
//  3. loose third
//
// Unindented list
// - a
// - b
//
// func f() {
//	body
// }
//
// [the site]: https://example.com
// [unused]: https://example.org
// [not a link]: nothing
//
package docs

import "C"

//
//go:noinline
//
func removedBlank() {}

//
func removedEntirely() {}

/*
F does things.
  code here

- item
*/
func F() {}

/*
 * Stars stay.
 */
func G() {}

/* single */
func H() {}

// T is a type.
// It has an old heading
//
// Not A Heading.
//
// Title Here
//
// Text following.
//
// Don't Title
//
// Bob's Title
//
// End.
type T struct {
	// Field doc is indented:
	//   stays as is
	Field int
}

// List with paragraphs:
//
//   - first
//
//     second paragraph
//   - [unknown] reference
//   - [docs.T] and [io.Reader]
//
// Trailing after list
// + plus item
// + another
// then text {
// forced code
//
//	[a b]: http://x.example/y
//
// [a
// b] spans lines.
//
// [a b]: http://x.example/y
// [a b]: http://dup.example/
//
//export K
func K() {}

//go:generate x
//line foo.go:1
func L() {}

// # 
// #notheading
// #	tabbed heading
//
// 12. twelve
// 13. thirteen
//
// •	bullet char
//
// text ending in backslash \
// continued
//
//
func M() {
	// inside:
	//   not touched
	var _ = 1
}
