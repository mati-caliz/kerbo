// Reglas chicas de staticcheck v0.8.2 calcadas sobre go/ast y go/types: ST1012, QF1011 y ST1023,
// QF1002, S1005, SA9003 y QF1006.
package main

import (
	"go/ast"
	"go/token"
	"go/types"
	"reflect"
	"strings"
)

func (c *collector) addReport(rule string, pos token.Pos, subject string) {
	if facts, position, ok := c.file(pos); ok {
		facts.Reports = append(facts.Reports, report{Rule: rule, Line: position.Line, Column: position.Column, Subject: subject})
	}
}

func (c *collector) collectStaticcheckTail(file *ast.File, filename string) {
	c.checkErrorVarNames(file)
	inFunction := 0
	var stack []ast.Node
	ast.Inspect(file, func(node ast.Node) bool {
		if node == nil {
			if _, ok := stack[len(stack)-1].(*ast.FuncDecl); ok {
				inFunction--
			} else if _, ok := stack[len(stack)-1].(*ast.FuncLit); ok {
				inFunction--
			}
			stack = stack[:len(stack)-1]
			return false
		}
		stack = append(stack, node)
		switch node := node.(type) {
		case *ast.FuncDecl:
			inFunction++
			if node.Body != nil && !(strings.HasPrefix(node.Name.Name, "Example") && strings.HasSuffix(filename, "_test.go")) {
				c.checkEmptyBranches(node.Body)
			}
		case *ast.FuncLit:
			if inFunction == 0 {
				c.checkEmptyBranches(node.Body)
			}
			inFunction++
		case *ast.GenDecl:
			if inFunction > 0 {
				c.checkRedundantTypes(node)
			}
		case *ast.SwitchStmt:
			c.checkTaggedSwitch(node)
		case *ast.AssignStmt:
			c.checkBlankReceive(node)
		case *ast.RangeStmt:
			c.checkBlankRange(node)
		case *ast.ForStmt:
			c.checkLiftableBreak(node)
		}
		return true
	})
}

// ST1012: una variable de error del paquete se llama errFoo (o ErrFoo si se exporta).
func (c *collector) checkErrorVarNames(file *ast.File) {
	for _, declaration := range file.Decls {
		general, ok := declaration.(*ast.GenDecl)
		if !ok || general.Tok != token.VAR {
			continue
		}
		for _, spec := range general.Specs {
			value := spec.(*ast.ValueSpec)
			if len(value.Names) != len(value.Values) {
				continue
			}
			for i, name := range value.Names {
				call, ok := value.Values[i].(*ast.CallExpr)
				if !ok {
					continue
				}
				if called := c.callName(call); called != "errors.New" && called != "fmt.Errorf" {
					continue
				}
				if c.pkg.Path() == "net/http" && strings.HasPrefix(name.Name, "http2err") {
					continue
				}
				prefix := "err"
				if name.IsExported() {
					prefix = "Err"
				}
				if !strings.HasPrefix(name.Name, prefix) {
					c.addReport("st1012", name.Pos(), name.Name)
				}
			}
		}
	}
}

func (c *collector) callName(call *ast.CallExpr) string {
	function := ast.Unparen(call.Fun)
	switch index := function.(type) {
	case *ast.IndexExpr:
		function = index.X
	case *ast.IndexListExpr:
		function = index.X
	}
	switch function.(type) {
	case *ast.SelectorExpr, *ast.Ident:
		return c.symbolName(function)
	}
	return ""
}

// QF1011 ("could") y ST1023 ("should", que además saltea `_`, constantes con nombre y lo que no
// sea un literal): el tipo de un `var` dentro de una función que el valor ya da.
func (c *collector) checkRedundantTypes(declaration *ast.GenDecl) {
	if declaration.Tok != token.VAR {
		return
	}
	strict := true
	for _, imported := range c.pkg.Imports() {
		if imported.Path() == "syscall" || imported.Path() == "unsafe" {
			strict = false
		}
	}
	for _, rule := range []struct {
		name    string
		helpful bool
	}{{"qf1011", true}, {"st1023", false}} {
		if !rule.helpful && !strict {
			continue
		}
		for _, spec := range declaration.Specs {
			value := spec.(*ast.ValueSpec)
			if value.Type != nil && len(value.Names) == len(value.Values) && c.redundantType(value, rule.helpful) {
				c.addReport(rule.name, value.Type.Pos(), types.ExprString(value.Type))
			}
		}
	}
}

func (c *collector) redundantType(spec *ast.ValueSpec, helpful bool) bool {
	declared := c.info.TypeOf(spec.Type)
	for i, value := range spec.Values {
		if !helpful && spec.Names[i].Name == "_" {
			return false
		}
		if !types.Identical(declared, c.info.TypeOf(value)) {
			return false
		}
		evaluated := &types.Info{Types: map[ast.Expr]types.TypeAndValue{}}
		if err := types.CheckExpr(c.fset, c.pkg, value.Pos(), value, evaluated); err != nil {
			return false
		}
		basic, ok := types.Unalias(evaluated.Types[value].Type).(*types.Basic)
		if !ok || basic.Info()&types.IsUntyped == 0 {
			continue
		}
		if declared != types.Default(basic) {
			return false
		}
		switch value := value.(type) {
		case *ast.Ident:
			if object := c.info.ObjectOf(value); object != nil && object.Pkg() != nil && !helpful {
				return false
			}
		case *ast.BasicLit:
		default:
			if !helpful {
				return false
			}
		}
	}
	return true
}

// QF1002: `switch { case x == 1: ... case x == 2 || x == 3: ... }` con el mismo `x`.
func (c *collector) checkTaggedSwitch(statement *ast.SwitchStmt) {
	if statement.Tag != nil || len(statement.Body.List) == 0 {
		return
	}
	pairs := make([][]*ast.BinaryExpr, len(statement.Body.List))
	for i, clause := range statement.Body.List {
		for _, condition := range clause.(*ast.CaseClause).List {
			if !switchPairs(condition, &pairs[i]) {
				return
			}
		}
	}
	var tag ast.Expr
	for _, pair := range pairs {
		if len(pair) == 0 {
			continue
		}
		if tag == nil {
			tag = pair[0].X
		} else if !astEqual(tag, pair[0].X) {
			return
		}
	}
	if tag != nil {
		c.addReport("qf1002", statement.Pos(), types.ExprString(tag))
	}
}

func switchPairs(expression ast.Expr, pairs *[]*ast.BinaryExpr) bool {
	binary, ok := ast.Unparen(expression).(*ast.BinaryExpr)
	if !ok {
		return false
	}
	switch binary.Op {
	case token.EQL:
		if mayHaveSideEffects(binary.X) || mayHaveSideEffects(binary.Y) {
			return false
		}
		if len(*pairs) > 0 && !astEqual(binary.X, (*pairs)[0].X) {
			return false
		}
		*pairs = append(*pairs, binary)
		return true
	case token.LOR:
		return switchPairs(binary.X, pairs) && switchPairs(binary.Y, pairs)
	}
	return false
}

// `code.MayHaveSideEffects` sin funciones puras: toda llamada cuenta, y `&` y `<-` también.
func mayHaveSideEffects(expression ast.Expr) bool {
	switch expression := expression.(type) {
	case nil, *ast.FuncLit, *ast.BasicLit, *ast.Ident,
		*ast.ArrayType, *ast.StructType, *ast.FuncType, *ast.InterfaceType, *ast.MapType, *ast.ChanType:
		return false
	case *ast.CallExpr, *ast.BadExpr:
		return true
	case *ast.Ellipsis:
		return mayHaveSideEffects(expression.Elt)
	case *ast.BinaryExpr:
		return mayHaveSideEffects(expression.X) || mayHaveSideEffects(expression.Y)
	case *ast.CompositeLit:
		if mayHaveSideEffects(expression.Type) {
			return true
		}
		for _, element := range expression.Elts {
			if mayHaveSideEffects(element) {
				return true
			}
		}
		return false
	case *ast.IndexExpr:
		return mayHaveSideEffects(expression.X) || mayHaveSideEffects(expression.Index)
	case *ast.IndexListExpr:
		if mayHaveSideEffects(expression.X) {
			return true
		}
		for _, index := range expression.Indices {
			if mayHaveSideEffects(index) {
				return true
			}
		}
		return false
	case *ast.KeyValueExpr:
		return mayHaveSideEffects(expression.Key) || mayHaveSideEffects(expression.Value)
	case *ast.SelectorExpr:
		return mayHaveSideEffects(expression.X)
	case *ast.SliceExpr:
		return mayHaveSideEffects(expression.X) || mayHaveSideEffects(expression.Low) ||
			mayHaveSideEffects(expression.High) || mayHaveSideEffects(expression.Max)
	case *ast.StarExpr:
		return mayHaveSideEffects(expression.X)
	case *ast.TypeAssertExpr:
		return mayHaveSideEffects(expression.X)
	case *ast.UnaryExpr:
		return mayHaveSideEffects(expression.X) || expression.Op == token.ARROW || expression.Op == token.AND
	case *ast.ParenExpr:
		return mayHaveSideEffects(expression.X)
	}
	return true
}

// `astutil.Equal` de staticcheck, con su comparación de literales compuestos que sólo mira la
// cantidad de elementos y el tipo.
func astEqual(a, b ast.Node) bool {
	if a == b {
		return true
	}
	if a == nil || b == nil || reflect.TypeOf(a) != reflect.TypeOf(b) {
		return false
	}
	switch a := a.(type) {
	case *ast.BasicLit:
		b := b.(*ast.BasicLit)
		return a.Kind == b.Kind && a.Value == b.Value
	case *ast.BinaryExpr:
		b := b.(*ast.BinaryExpr)
		return astEqual(a.X, b.X) && a.Op == b.Op && astEqual(a.Y, b.Y)
	case *ast.CallExpr:
		b := b.(*ast.CallExpr)
		if len(a.Args) != len(b.Args) {
			return false
		}
		for i := range a.Args {
			if !astEqual(a.Args[i], b.Args[i]) {
				return false
			}
		}
		return astEqual(a.Fun, b.Fun) && (a.Ellipsis == token.NoPos) == (b.Ellipsis == token.NoPos)
	case *ast.CompositeLit:
		b := b.(*ast.CompositeLit)
		return len(a.Elts) == len(b.Elts) && astEqual(a.Type, b.Type) && a.Incomplete == b.Incomplete
	case *ast.Ident:
		return a.Name == b.(*ast.Ident).Name
	case *ast.IndexExpr:
		b := b.(*ast.IndexExpr)
		return astEqual(a.X, b.X) && astEqual(a.Index, b.Index)
	case *ast.KeyValueExpr:
		b := b.(*ast.KeyValueExpr)
		return astEqual(a.Key, b.Key) && astEqual(a.Value, b.Value)
	case *ast.ParenExpr:
		return astEqual(a.X, b.(*ast.ParenExpr).X)
	case *ast.SelectorExpr:
		b := b.(*ast.SelectorExpr)
		return astEqual(a.X, b.X) && astEqual(a.Sel, b.Sel)
	case *ast.SliceExpr:
		b := b.(*ast.SliceExpr)
		return astEqual(a.X, b.X) && astEqual(a.Low, b.Low) && astEqual(a.High, b.High) &&
			astEqual(a.Max, b.Max) && a.Slice3 == b.Slice3
	case *ast.StarExpr:
		return astEqual(a.X, b.(*ast.StarExpr).X)
	case *ast.TypeAssertExpr:
		b := b.(*ast.TypeAssertExpr)
		return astEqual(a.X, b.X) && astEqual(a.Type, b.Type)
	case *ast.UnaryExpr:
		b := b.(*ast.UnaryExpr)
		return a.Op == b.Op && astEqual(a.X, b.X)
	case *ast.ArrayType:
		b := b.(*ast.ArrayType)
		return astEqual(a.Len, b.Len) && astEqual(a.Elt, b.Elt)
	case *ast.MapType:
		b := b.(*ast.MapType)
		return astEqual(a.Key, b.Key) && astEqual(a.Value, b.Value)
	case *ast.Ellipsis:
		return astEqual(a.Elt, b.(*ast.Ellipsis).Elt)
	}
	return false
}

// S1005: `x, _ = <-ch`, `_ = <-ch` y los `_` que sobran en un `for range`.
func (c *collector) checkBlankReceive(assignment *ast.AssignStmt) {
	if len(assignment.Rhs) != 1 {
		return
	}
	receive, ok := ast.Unparen(assignment.Rhs[0]).(*ast.UnaryExpr)
	if !ok || receive.Op != token.ARROW {
		return
	}
	switch len(assignment.Lhs) {
	case 1, 2:
		if isBlank(ast.Unparen(assignment.Lhs[len(assignment.Lhs)-1])) {
			c.addReport("s1005", assignment.Pos(), "")
		}
	}
}

func isBlank(expression ast.Expr) bool {
	ident, ok := expression.(*ast.Ident)
	return ok && ident.Name == "_"
}

func (c *collector) checkBlankRange(statement *ast.RangeStmt) {
	if typ := c.info.TypeOf(statement.X); typ == nil {
		return
	} else if _, ok := typ.Underlying().(*types.Signature); ok {
		return
	}
	if statement.Value == nil && isBlank(statement.Key) {
		c.addReport("s1005", statement.Key.Pos(), "")
	}
	if isBlank(statement.Key) && isBlank(statement.Value) {
		c.addReport("s1005", statement.Key.Pos(), "")
	}
	if !isBlank(statement.Key) && isBlank(statement.Value) {
		c.addReport("s1005", statement.Value.Pos(), "")
	}
}

// SA9003: un if (o su else) vacío dentro de una función que no es un ejemplo.
func (c *collector) checkEmptyBranches(body *ast.BlockStmt) {
	ast.Inspect(body, func(node ast.Node) bool {
		statement, ok := node.(*ast.IfStmt)
		if !ok {
			return true
		}
		if statement.Else != nil {
			block, ok := statement.Else.(*ast.BlockStmt)
			if !ok || len(block.List) != 0 {
				return true
			}
			c.addReport("sa9003", statement.Else.Pos(), "")
		}
		if len(statement.Body.List) == 0 {
			c.addReport("sa9003", statement.Pos(), "")
		}
		return true
	})
}

// QF1006: `for { if cond { break } ... }` puede llevar la condición al `for`.
func (c *collector) checkLiftableBreak(loop *ast.ForStmt) {
	if loop.Init != nil || loop.Cond != nil || loop.Post != nil || len(loop.Body.List) == 0 {
		return
	}
	statement, ok := loop.Body.List[0].(*ast.IfStmt)
	if !ok || statement.Init != nil || statement.Else != nil || len(statement.Body.List) != 1 {
		return
	}
	branch, ok := statement.Body.List[0].(*ast.BranchStmt)
	if ok && branch.Tok == token.BREAK && branch.Label == nil {
		c.addReport("qf1006", statement.Pos(), "")
	}
}
