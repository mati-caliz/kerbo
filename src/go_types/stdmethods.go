// stdmethods de govet (golang.org/x/tools v0.52.0): un método con el nombre de uno de las
// interfaces conocidas de la biblioteca estándar tiene que tener su firma. Los tipos con `=`
// deciden si el método quiso implementarla; si coinciden, el resto también tiene que coincidir.
package main

import (
	"go/ast"
	"go/types"
	"strings"
)

const stdmethodsRule = "stdmethods"

var canonicalMethods = map[string]struct{ args, results []string }{
	"As":            {[]string{"any"}, []string{"bool"}},
	"Format":        {[]string{"=fmt.State", "rune"}, []string{}},
	"GobDecode":     {[]string{"[]byte"}, []string{"error"}},
	"GobEncode":     {[]string{}, []string{"[]byte", "error"}},
	"Is":            {[]string{"error"}, []string{"bool"}},
	"MarshalJSON":   {[]string{}, []string{"[]byte", "error"}},
	"MarshalXML":    {[]string{"*xml.Encoder", "xml.StartElement"}, []string{"error"}},
	"ReadByte":      {[]string{}, []string{"byte", "error"}},
	"ReadFrom":      {[]string{"=io.Reader"}, []string{"int64", "error"}},
	"ReadRune":      {[]string{}, []string{"rune", "int", "error"}},
	"Scan":          {[]string{"=fmt.ScanState", "rune"}, []string{"error"}},
	"Seek":          {[]string{"=int64", "int"}, []string{"int64", "error"}},
	"UnmarshalJSON": {[]string{"[]byte"}, []string{"error"}},
	"UnmarshalXML":  {[]string{"*xml.Decoder", "xml.StartElement"}, []string{"error"}},
	"UnreadByte":    {[]string{}, []string{"error"}},
	"UnreadRune":    {[]string{}, []string{"error"}},
	"Unwrap":        {[]string{}, []string{"error"}},
	"WriteByte":     {[]string{"byte"}, []string{"error"}},
	"WriteTo":       {[]string{"=io.Writer"}, []string{"int64", "error"}},
}

func (c *collector) collectCanonicalMethods(file *ast.File) {
	ast.Inspect(file, func(node ast.Node) bool {
		switch node := node.(type) {
		case *ast.FuncDecl:
			if node.Recv != nil {
				c.checkCanonicalMethod(node.Name)
			}
		case *ast.InterfaceType:
			for _, field := range node.Methods.List {
				for _, name := range field.Names {
					c.checkCanonicalMethod(name)
				}
			}
		}
		return true
	})
}

func (c *collector) checkCanonicalMethod(name *ast.Ident) {
	expect, ok := canonicalMethods[name.Name]
	if !ok {
		return
	}
	defined := c.info.Defs[name]
	if defined == nil {
		return
	}
	signature, ok := defined.Type().(*types.Signature)
	if !ok {
		return
	}
	args, results := signature.Params(), signature.Results()
	if name.Name == "WriteTo" && args.Len() > 1 {
		return
	}
	if name.Name == "Is" || name.Name == "As" || name.Name == "Unwrap" {
		if receiver := signature.Recv(); receiver == nil || !types.Implements(receiver.Type(), errorInterface) {
			return
		}
	}
	if name.Name == "Unwrap" {
		if args.Len() == 0 && results.Len() == 1 {
			if result := methodTypeString(results.At(0).Type()); result == "error" || result == "[]error" {
				return
			}
		}
		c.reportCanonical(name, "Unwrap() should have signature Unwrap() error or Unwrap() []error")
		return
	}
	if !matchParams(expect.args, args, "=") || !matchParams(expect.results, results, "=") {
		return
	}
	if matchParams(expect.args, args, "") && matchParams(expect.results, results, "") {
		return
	}
	expected := name.Name + "(" + argJoin(expect.args) + ")"
	if len(expect.results) == 1 {
		expected += " " + argJoin(expect.results)
	} else if len(expect.results) > 1 {
		expected += " (" + argJoin(expect.results) + ")"
	}
	actual := name.Name + strings.TrimPrefix(methodTypeString(signature), "func")
	c.reportCanonical(name, actual+" should have signature "+expected)
}

func (c *collector) reportCanonical(name *ast.Ident, subject string) {
	if facts, position, ok := c.file(name.Pos()); ok {
		facts.Reports = append(facts.Reports, report{Rule: stdmethodsRule, Line: position.Line, Column: position.Column, Subject: subject})
	}
}

func methodTypeString(typ types.Type) string {
	return types.TypeString(typ, (*types.Package).Name)
}

func argJoin(types []string) string {
	joined := make([]string, len(types))
	for i, typ := range types {
		joined[i] = strings.TrimPrefix(typ, "=")
	}
	return strings.Join(joined, ", ")
}

func matchParams(expect []string, actual *types.Tuple, prefix string) bool {
	for i, typ := range expect {
		if !strings.HasPrefix(typ, prefix) {
			continue
		}
		if i >= actual.Len() || !matchParamType(typ, actual.At(i).Type()) {
			return false
		}
	}
	return prefix != "" || actual.Len() <= len(expect)
}

func matchParamType(expect string, actual types.Type) bool {
	expect = strings.TrimPrefix(expect, "=")
	written := methodTypeString(actual)
	return written == expect ||
		(written == "any" || written == "interface{}") && (expect == "any" || expect == "interface{}")
}
