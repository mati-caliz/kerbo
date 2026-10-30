// Las reglas de staticcheck v0.8.2 que son un solo patrón sobre go/ast y go/types: S1004, S1006,
// S1010, S1012, S1023, S1024, S1028, S1033, S1037, SA1012, SA1013, SA2000, SA3001, SA4001, SA4013,
// SA4021, SA4022, SA4024, SA4028 y SA6006.
package main

import (
	"go/ast"
	"go/constant"
	"go/token"
	"go/types"
	"regexp"
)

func (c *collector) collectStaticcheckShapes(file *ast.File) {
	ast.Inspect(file, func(node ast.Node) bool {
		switch node := node.(type) {
		case *ast.BinaryExpr:
			c.checkBytesCompare(node)
			c.checkAddressIsNil(node)
			c.checkLengthBelowZero(node)
			c.checkModuloOne(node)
		case *ast.ForStmt:
			c.checkForTrue(node)
			c.checkSpinningLoop(node)
		case *ast.SliceExpr:
			c.checkSliceToLength(node)
		case *ast.CallExpr:
			c.checkTimeSince(node)
			c.checkTimeUntil(node)
			c.checkErrorsNewSprintf(node)
			c.checkNilContext(node)
			c.checkSeekArguments(node)
			c.checkSingleArgumentAppend(node)
			c.checkWriteStringConversion(node)
		case *ast.CaseClause:
			c.checkRedundantBreak(node)
		case *ast.FuncDecl:
			c.checkRedundantReturn(node.Type, node.Body)
		case *ast.FuncLit:
			c.checkRedundantReturn(node.Type, node.Body)
		case *ast.IfStmt:
			c.checkGuardedDelete(node)
		case *ast.SelectStmt:
			c.checkElaborateSleep(node)
		case *ast.GoStmt:
			c.checkWaitGroupAddInGoroutine(node)
		case *ast.AssignStmt:
			c.checkBenchmarkIterationsAssignment(node)
		case *ast.UnaryExpr:
			c.checkIneffectiveAddressCopy(node)
			c.checkDoubleNegation(node)
		case *ast.StarExpr:
			c.checkIneffectiveDereferenceCopy(node)
		}
		return true
	})
}

// `IntegerLiteral` de los patrones: un entero literal (con signo) con ese valor.
func (c *collector) isIntegerLiteral(expression ast.Expr, value string) bool {
	literal, ok := c.integerLiteral(expression)
	return ok && literal.String() == value
}

// S1004: `bytes.Compare(a, b) == 0` en vez de `bytes.Equal`.
func (c *collector) checkBytesCompare(expression *ast.BinaryExpr) {
	if c.pkg.Path() == "bytes" || c.pkg.Path() == "bytes_test" || (expression.Op != token.EQL && expression.Op != token.NEQ) {
		return
	}
	call, ok := ast.Unparen(expression.X).(*ast.CallExpr)
	if !ok || c.symbolName(call.Fun) != "bytes.Compare" || !c.isIntegerLiteral(expression.Y, "0") {
		return
	}
	replacement := "bytes.Equal"
	if expression.Op == token.NEQ {
		replacement = "!" + replacement
	}
	c.addReport("s1004", expression.Pos(), replacement)
}

// S1006: `for true {}` en vez de `for {}`.
func (c *collector) checkForTrue(loop *ast.ForStmt) {
	if loop.Init != nil || loop.Post != nil || loop.Cond == nil || !c.isBoolConst(loop.Cond) {
		return
	}
	if constant.BoolVal(c.info.ObjectOf(loop.Cond.(*ast.Ident)).(*types.Const).Val()) {
		c.addReport("s1006", loop.Pos(), "")
	}
}

// S1010: `s[a:len(s)]` en vez de `s[a:]`.
func (c *collector) checkSliceToLength(slice *ast.SliceExpr) {
	if slice.Max != nil {
		return
	}
	sliced, ok := c.patternObject(slice.X)
	if !ok {
		return
	}
	length, ok := ast.Unparen(slice.High).(*ast.CallExpr)
	if ok && c.isBuiltin(length.Fun, "len") && len(length.Args) == 1 && c.refersToObject(length.Args[0], sliced) {
		c.addReport("s1010", slice.High.Pos(), "")
	}
}

func (c *collector) isTimeNow(expression ast.Expr) bool {
	call, ok := ast.Unparen(expression).(*ast.CallExpr)
	return ok && len(call.Args) == 0 && c.symbolName(call.Fun) == "time.Now"
}

// S1012: `time.Now().Sub(t)` en vez de `time.Since(t)`.
func (c *collector) checkTimeSince(call *ast.CallExpr) {
	selector, ok := ast.Unparen(call.Fun).(*ast.SelectorExpr)
	if ok && len(call.Args) == 1 && c.isTimeNow(selector.X) && c.symbolName(selector.Sel) == "(time.Time).Sub" {
		c.addReport("s1012", call.Pos(), "")
	}
}

// S1024: `t.Sub(time.Now())` en vez de `time.Until(t)`.
func (c *collector) checkTimeUntil(call *ast.CallExpr) {
	if len(call.Args) == 1 && c.symbolName(call.Fun) == "(time.Time).Sub" && c.isTimeNow(call.Args[0]) {
		c.addReport("s1024", call.Pos(), "")
	}
}

// S1028: `errors.New(fmt.Sprintf(...))` en vez de `fmt.Errorf(...)`.
func (c *collector) checkErrorsNewSprintf(call *ast.CallExpr) {
	if len(call.Args) == 1 && c.symbolName(call.Fun) == "errors.New" && c.isSprintfCall(call.Args[0]) {
		c.addReport("s1028", call.Pos(), "")
	}
}

// S1023: el `break` al final de un case y el `return` al final de una función que no devuelve nada.
func (c *collector) checkRedundantBreak(clause *ast.CaseClause) {
	if len(clause.Body) < 2 {
		return
	}
	if branch, ok := clause.Body[len(clause.Body)-1].(*ast.BranchStmt); ok && branch.Tok == token.BREAK && branch.Label == nil {
		c.addReport("s1023", branch.Pos(), "break")
	}
}

func (c *collector) checkRedundantReturn(function *ast.FuncType, body *ast.BlockStmt) {
	if function.Results != nil || body == nil || len(body.List) == 0 {
		return
	}
	if statement, ok := body.List[len(body.List)-1].(*ast.ReturnStmt); ok {
		c.addReport("s1023", statement.Pos(), "return")
	}
}

// S1033: `if _, ok := m[k]; ok { delete(m, k) }`: delete ya no hace nada si la clave no está.
func (c *collector) checkGuardedDelete(statement *ast.IfStmt) {
	initialization, ok := statement.Init.(*ast.AssignStmt)
	if !ok || initialization.Tok != token.DEFINE || len(initialization.Lhs) != 2 || len(initialization.Rhs) != 1 || statement.Else != nil {
		return
	}
	found, ok := initialization.Lhs[1].(*ast.Ident)
	if !ok || !isBlank(initialization.Lhs[0]) || !sameSyntax(found, statement.Cond) {
		return
	}
	lookup, ok := ast.Unparen(initialization.Rhs[0]).(*ast.IndexExpr)
	if !ok || len(statement.Body.List) != 1 {
		return
	}
	expression, ok := statement.Body.List[0].(*ast.ExprStmt)
	if !ok {
		return
	}
	call, ok := ast.Unparen(expression.X).(*ast.CallExpr)
	if ok && c.isBuiltin(call.Fun, "delete") && len(call.Args) == 2 && sameSyntax(call.Args[0], lookup.X) && sameSyntax(call.Args[1], lookup.Index) {
		c.addReport("s1033", statement.Pos(), "")
	}
}

// S1037: `select { case <-time.After(d): }` en vez de `time.Sleep(d)`.
func (c *collector) checkElaborateSleep(statement *ast.SelectStmt) {
	if len(statement.Body.List) != 1 {
		return
	}
	clause, ok := statement.Body.List[0].(*ast.CommClause)
	if !ok {
		return
	}
	communication, ok := clause.Comm.(*ast.ExprStmt)
	if !ok {
		return
	}
	receive, ok := ast.Unparen(communication.X).(*ast.UnaryExpr)
	if !ok || receive.Op != token.ARROW {
		return
	}
	call, ok := ast.Unparen(receive.X).(*ast.CallExpr)
	if ok && len(call.Args) == 1 && c.symbolName(call.Fun) == "time.After" {
		c.addReport("s1037", statement.Pos(), "")
	}
}

// SA1012: `nil` como primer argumento de una función que recibe un context.Context.
func (c *collector) checkNilContext(call *ast.CallExpr) {
	if len(call.Args) == 0 || !c.isBuiltin(call.Args[0], "nil") {
		return
	}
	function, ok := c.calledFunction(call.Fun)
	if !ok {
		return
	}
	parameters := function.Type().(*types.Signature).Params()
	if parameters.Len() != 0 && isTypeWithName(parameters.At(0).Type(), "context.Context") {
		c.addReport("sa1012", call.Args[0].Pos(), "")
	}
}

// La función que nombra lo llamado, como `Symbol` en los patrones de staticcheck.
func (c *collector) calledFunction(function ast.Expr) (*types.Func, bool) {
	switch index := function.(type) {
	case *ast.IndexExpr:
		function = index.X
	case *ast.IndexListExpr:
		function = index.X
	}
	var object types.Object
	switch inner := ast.Unparen(function).(type) {
	case *ast.Ident:
		object = c.info.ObjectOf(inner)
	case *ast.SelectorExpr:
		object = c.info.ObjectOf(inner.Sel)
	}
	called, ok := object.(*types.Func)
	return called, ok
}

var seekerSignature = types.NewSignatureType(nil, nil, nil,
	types.NewTuple(types.NewParam(token.NoPos, nil, "", types.Typ[types.Int64]), types.NewParam(token.NoPos, nil, "", types.Typ[types.Int])),
	types.NewTuple(types.NewParam(token.NoPos, nil, "", types.Typ[types.Int64]), types.NewParam(token.NoPos, nil, "", types.Universe.Lookup("error").Type())),
	false)

var seekWhences = map[string]bool{"io.SeekStart": true, "io.SeekCurrent": true, "io.SeekEnd": true}

// SA1013: `Seek(io.SeekStart, 0)`, con los argumentos al revés.
func (c *collector) checkSeekArguments(call *ast.CallExpr) {
	method, ok := ast.Unparen(call.Fun).(*ast.SelectorExpr)
	if !ok || method.Sel.Name != "Seek" || len(call.Args) != 2 {
		return
	}
	whence, ok := ast.Unparen(call.Args[0]).(*ast.SelectorExpr)
	if !ok || !seekWhences[c.objectName(whence.Sel)] {
		return
	}
	selection, ok := c.info.Selections[method]
	if ok && selection.Kind() == types.MethodVal && types.Identical(selection.Type(), seekerSignature) {
		c.addReport("sa1013", call.Pos(), "")
	}
}

// El nombre de una constante o variable del paquete como lo da `Symbol`: ruta del paquete y nombre.
func (c *collector) objectName(ident *ast.Ident) string {
	object := c.info.ObjectOf(ident)
	switch object.(type) {
	case *types.Const, *types.Var:
		if object.Pkg() != nil && object.Parent() == object.Pkg().Scope() {
			return object.Pkg().Path() + "." + object.Name()
		}
	}
	return ""
}

// SA2000: `wg.Add` como primera sentencia de la goroutine, que puede correr después del Wait.
func (c *collector) checkWaitGroupAddInGoroutine(statement *ast.GoStmt) {
	literal, ok := ast.Unparen(statement.Call.Fun).(*ast.FuncLit)
	if !ok || len(literal.Body.List) == 0 {
		return
	}
	first, ok := literal.Body.List[0].(*ast.ExprStmt)
	if !ok {
		return
	}
	if call, ok := ast.Unparen(first.X).(*ast.CallExpr); ok && c.symbolName(call.Fun) == "(*sync.WaitGroup).Add" {
		c.addReport("sa2000", call.Pos(), c.render(call))
	}
}

// SA3001: asignarle a `b.N` en un benchmark.
func (c *collector) checkBenchmarkIterationsAssignment(assignment *ast.AssignStmt) {
	if assignment.Tok != token.ASSIGN || len(assignment.Lhs) != 1 || len(assignment.Rhs) != 1 {
		return
	}
	selector, ok := ast.Unparen(assignment.Lhs[0]).(*ast.SelectorExpr)
	if !ok || selector.Sel.Name != "N" {
		return
	}
	pointer, ok := types.Unalias(c.info.TypeOf(selector.X)).(*types.Pointer)
	if ok && isTypeWithName(pointer.Elem(), "testing.B") {
		c.addReport("sa3001", assignment.Pos(), c.render(selector))
	}
}

var cgoIdent = regexp.MustCompile(`^_C(func|var)_.+$`)

// SA4001: `&*x` y `*&x`, que el compilador simplifica a `x` sin copiar nada.
func (c *collector) checkIneffectiveAddressCopy(expression *ast.UnaryExpr) {
	if expression.Op != token.AND {
		return
	}
	star, ok := ast.Unparen(expression.X).(*ast.StarExpr)
	if !ok {
		return
	}
	if ident, ok := ast.Unparen(star.X).(*ast.Ident); !ok || !cgoIdent.MatchString(ident.Name) {
		c.addReport("sa4001", expression.Pos(), "&*x")
	}
}

func (c *collector) checkIneffectiveDereferenceCopy(expression *ast.StarExpr) {
	if address, ok := ast.Unparen(expression.X).(*ast.UnaryExpr); ok && address.Op == token.AND {
		c.addReport("sa4001", expression.Pos(), "*&x")
	}
}

// SA4013: `!!x`.
func (c *collector) checkDoubleNegation(expression *ast.UnaryExpr) {
	if inner, ok := ast.Unparen(expression.X).(*ast.UnaryExpr); ok && expression.Op == token.NOT && inner.Op == token.NOT {
		c.addReport("sa4013", expression.Pos(), "")
	}
}

// SA4021: `append(x)` sin nada que agregar.
func (c *collector) checkSingleArgumentAppend(call *ast.CallExpr) {
	if len(call.Args) == 1 && c.isBuiltin(call.Fun, "append") {
		c.addReport("sa4021", call.Pos(), "")
	}
}

// SA4022: `&x == nil`, que nunca se cumple.
func (c *collector) checkAddressIsNil(expression *ast.BinaryExpr) {
	if expression.Op != token.EQL && expression.Op != token.NEQ {
		return
	}
	if address, ok := ast.Unparen(expression.X).(*ast.UnaryExpr); ok && address.Op == token.AND && c.isBuiltin(expression.Y, "nil") {
		c.addReport("sa4022", expression.Pos(), "")
	}
}

// SA4024: `len(x) < 0` y `0 > cap(x)`, que nunca se cumplen.
func (c *collector) checkLengthBelowZero(expression *ast.BinaryExpr) {
	var measured ast.Expr
	switch {
	case expression.Op == token.GTR && c.isIntegerLiteral(expression.X, "0"):
		measured = expression.Y
	case expression.Op == token.LSS && c.isIntegerLiteral(expression.Y, "0"):
		measured = expression.X
	default:
		return
	}
	call, ok := ast.Unparen(measured).(*ast.CallExpr)
	if !ok {
		return
	}
	for _, name := range []string{"len", "cap"} {
		if c.isBuiltin(call.Fun, name) {
			c.addReport("sa4024", expression.Pos(), name)
		}
	}
}

// SA4028: `x % 1`, que siempre da cero.
func (c *collector) checkModuloOne(expression *ast.BinaryExpr) {
	if expression.Op == token.REM && c.isIntegerLiteral(expression.Y, "1") {
		c.addReport("sa4028", expression.Pos(), "")
	}
}

// SA5002: un `for` vacío que gira sin parar; con una condición que no cambia, además un aviso por
// ella, como staticcheck.
func (c *collector) checkSpinningLoop(loop *ast.ForStmt) {
	if len(loop.Body.List) != 0 || loop.Post != nil || loop.Init != nil {
		return
	}
	if loop.Cond != nil {
		if mayHaveSideEffects(loop.Cond) {
			return
		}
		if ident, ok := loop.Cond.(*ast.Ident); ok {
			if named, ok := c.info.ObjectOf(ident).(*types.Const); ok && !constant.BoolVal(named.Val()) {
				return
			}
		}
		c.addReport("sa5002", loop.Pos(), "condition")
	}
	c.addReport("sa5002", loop.Pos(), "spin")
}

// SA6006: `io.WriteString(w, string(b))` con un []byte, que se puede escribir directo.
func (c *collector) checkWriteStringConversion(call *ast.CallExpr) {
	if len(call.Args) != 2 || c.symbolName(call.Fun) != "io.WriteString" {
		return
	}
	conversion, ok := ast.Unparen(call.Args[1]).(*ast.CallExpr)
	if !ok || !c.isBuiltin(conversion.Fun, "string") || len(conversion.Args) != 1 {
		return
	}
	converted := c.info.TypeOf(ast.Unparen(conversion.Args[0]))
	if converted == nil {
		return
	}
	slice, ok := converted.Underlying().(*types.Slice)
	if ok && types.Identical(types.Unalias(slice.Elem()).Underlying(), types.Typ[types.Byte]) {
		c.addReport("sa6006", call.Pos(), "")
	}
}
