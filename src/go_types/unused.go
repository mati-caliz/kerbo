// unused de staticcheck sobre los archivos del paquete, como lo corre golangci-lint: el grafo de
// unused_graph.go con las opciones por defecto, un paquete por vez.
package main

import (
	"bufio"
	"bytes"
	"fmt"
	"go/ast"
	"go/token"
	"go/types"
	"io"
	"os"
	"path/filepath"
	"strings"
)

const unusedRule = "unused"

// Lo que unused_graph.go toma de otros paquetes de staticcheck.
type lintDirective struct {
	Command   string
	Arguments []string
	Node      ast.Node
}

type SerializedGraph struct {
	nodes []Node
}

var (
	runtimeFuncs         map[string]bool
	runtimeCoverageFuncs map[string]bool
)

func (c *collector) collectUnused(files []*ast.File) {
	defer func() {
		if failure := recover(); failure != nil {
			fmt.Fprintf(os.Stderr, "unused en %s: %v\n", c.pkg.Path(), failure)
		}
	}()
	generated := map[string]bool{}
	for _, file := range files {
		name := c.fset.PositionFor(file.Pos(), false).Filename
		if isGeneratedFile(name) {
			generated[name] = true
		}
	}
	graph := newGraph(c.fset, files, c.pkg, c.info, lintDirectives(c.fset, files), generated, DefaultOptions)
	graph.entry()
	results := (&SerializedGraph{nodes: graph.nodes}).Results()
	reported := map[string]bool{}
	for _, object := range results.Used {
		reported[unusedKey(object)] = true
	}
	for _, object := range results.Unused {
		key := unusedKey(object)
		if object.Kind == "type param" || reported[key] {
			continue
		}
		reported[key] = true
		c.addReportAt(unusedRule, object.DisplayPosition, object.Kind+" "+object.Name)
	}
}

// golangci-lint no repite un objeto que aparece usado y sin usar en el mismo lugar.
func unusedKey(object Object) string {
	return fmt.Sprintf("%s %d %s", object.Position.Filename, object.Position.Line, object.Name)
}

func (c *collector) addReportAt(rule string, position token.Position, subject string) {
	if filepath.Dir(position.Filename) != c.directory {
		return
	}
	facts, ok := c.byFile[position.Filename]
	if !ok {
		facts = &fileFacts{File: position.Filename}
		c.byFile[position.Filename] = facts
	}
	facts.Reports = append(facts.Reports, report{Rule: rule, Line: position.Line, Column: position.Column, Subject: subject})
}

// `report.DisplayPosition`: la posición ajustada por `//line` sólo si apunta a otro archivo Go.
func displayPosition(fset *token.FileSet, pos token.Pos) token.Position {
	if pos == token.NoPos {
		return token.Position{}
	}
	adjusted := fset.PositionFor(pos, true)
	if filepath.Ext(adjusted.Filename) == ".go" {
		return adjusted
	}
	return fset.PositionFor(pos, false)
}

var (
	generatedPrefix = []byte("// Code generated ")
	generatedSuffix = []byte(" DO NOT EDIT.")
	oldCgoMarker    = []byte("// Created by cgo - DO NOT EDIT")
)

// El analizador `generated` de staticcheck: alguna línea del archivo dice que es generado.
func isGeneratedFile(path string) bool {
	file, err := os.Open(path)
	if err != nil {
		return false
	}
	defer file.Close()
	reader := bufio.NewReader(file)
	for {
		line, err := reader.ReadBytes('\n')
		if err != nil && err != io.EOF {
			return false
		}
		line = bytes.TrimSuffix(bytes.TrimSuffix(line, []byte("\r\n")), []byte("\n"))
		if bytes.HasPrefix(line, generatedPrefix) && bytes.HasSuffix(line, generatedSuffix) {
			return true
		}
		if bytes.Equal(line, oldCgoMarker) {
			return true
		}
		if err == io.EOF {
			return false
		}
	}
}

// `lint.ParseDirectives`: los comentarios `//lint:` con el nodo al que los asocia go/ast.
func lintDirectives(fset *token.FileSet, files []*ast.File) []lintDirective {
	var directives []lintDirective
	for _, file := range files {
		for node, groups := range ast.NewCommentMap(fset, file, file.Comments) {
			for _, group := range groups {
				for _, comment := range group.List {
					if !strings.HasPrefix(comment.Text, "//lint:") {
						continue
					}
					fields := strings.Split(strings.TrimPrefix(comment.Text, "//lint:"), " ")
					directives = append(directives, lintDirective{Command: fields[0], Arguments: fields[1:], Node: node})
				}
			}
		}
	}
	return directives
}

func exhaustiveTypeSwitch(value any) {
	panic(fmt.Sprintf("caso no contemplado %T", value))
}

func dereference(typ types.Type) types.Type {
	if pointer, ok := typ.Underlying().(*types.Pointer); ok {
		return pointer.Elem()
	}
	return typ
}

// `typeutil.CoreType`: el tipo subyacente común a todos los términos del conjunto de tipos, o nil.
func coreType(typ types.Type) types.Type {
	terms, ok := typeTerms(typ)
	if !ok || len(terms) == 0 {
		return nil
	}
	core := terms[0].Underlying()
	for _, term := range terms[1:] {
		underlying := term.Underlying()
		if types.Identical(core, underlying) {
			continue
		}
		first, firstIsChan := core.(*types.Chan)
		second, secondIsChan := underlying.(*types.Chan)
		if !firstIsChan || !secondIsChan || !types.Identical(first.Elem(), second.Elem()) {
			return nil
		}
		switch {
		case first.Dir() == types.SendRecv:
			core = underlying
		case second.Dir() == types.SendRecv:
		case first.Dir() != second.Dir():
			return nil
		}
	}
	return core
}

// Los términos del conjunto de tipos: el tipo solo si no es una interfaz ni un parámetro de tipo; para
// las restricciones, los de su única unión (con más de una, no se calcula la intersección).
func typeTerms(typ types.Type) ([]types.Type, bool) {
	if parameter, ok := typ.(*types.TypeParam); ok {
		typ = parameter.Constraint()
	}
	constraint, ok := typ.Underlying().(*types.Interface)
	if !ok {
		return []types.Type{typ}, true
	}
	var terms []types.Type
	restricted := false
	for i := 0; i < constraint.NumEmbeddeds(); i++ {
		embedded := constraint.EmbeddedType(i)
		var embeddedTerms []types.Type
		if union, ok := embedded.(*types.Union); ok {
			for j := 0; j < union.Len(); j++ {
				embeddedTerms = append(embeddedTerms, union.Term(j).Type())
			}
		} else if nested, ok := typeTerms(embedded); ok {
			embeddedTerms = nested
		} else {
			return nil, false
		}
		if len(embeddedTerms) == 0 {
			continue
		}
		if restricted {
			return nil, false
		}
		restricted = true
		terms = embeddedTerms
	}
	for _, term := range terms {
		if _, ok := term.Underlying().(*types.Interface); ok {
			return nil, false
		}
	}
	return terms, true
}
