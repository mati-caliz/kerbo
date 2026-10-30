// Más reglas de staticcheck v0.8.2 calcadas sobre go/ast y go/types: S1038, SA1006, SA2001, SA4004,
// SA4011, SA4016, SA6002, SA6005, SA9009, ST1008 y QF1007.
package main

import (
	"bytes"
	"go/ast"
	"go/constant"
	"go/printer"
	"go/token"
	"go/types"
	"reflect"
	"strings"
)

func (c *collector) collectStaticcheckChecks(file *ast.File) {
	c.checkSpacedDirectives(file)
	exits := map[token.Pos]bool{}
	ast.Inspect(file, func(node ast.Node) bool {
		switch node := node.(type) {
		case *ast.CallExpr:
			c.checkDynamicFormat(node)
			c.checkPoolPut(node)
		case *ast.BlockStmt:
			c.checkEmptyCriticalSection(node)
		case *ast.BinaryExpr:
			c.checkIneffectiveBitOperation(node)
			c.checkCaseInsensitiveComparison(node)
		case *ast.ForStmt:
			c.checkIneffectiveBreaks(node.Body)
		case *ast.RangeStmt:
			c.checkIneffectiveBreaks(node.Body)
		case *ast.FuncDecl:
			c.checkTerminatedLoops(node.Body, exits)
			c.checkConditionalAssignments(node.Body)
			if function, ok := c.info.Defs[node.Name].(*types.Func); ok {
				c.checkErrorPosition(function.Type())
			}
		case *ast.FuncLit:
			c.checkTerminatedLoops(node.Body, exits)
			c.checkConditionalAssignments(node.Body)
			c.checkErrorPosition(c.info.TypeOf(node))
		}
		return true
	})
	for pos := range exits {
		c.addReport("sa4004", pos, "")
	}
}

var (
	positionType     = reflect.TypeOf(token.NoPos)
	objectType       = reflect.TypeOf((*ast.Object)(nil))
	commentGroupType = reflect.TypeOf((*ast.CommentGroup)(nil))
)

// Lo que hace el matcher de patrones de staticcheck cuando una variable ya ligada vuelve a aparecer:
// los dos árboles campo por campo, sin posiciones, comentarios ni paréntesis.
func sameSyntax(first, second ast.Node) bool {
	first, second = unparenNode(first), unparenNode(second)
	firstValue, secondValue := reflect.ValueOf(first), reflect.ValueOf(second)
	if !firstValue.IsValid() || !secondValue.IsValid() {
		return firstValue.IsValid() == secondValue.IsValid()
	}
	if firstValue.Type() != secondValue.Type() {
		return false
	}
	if firstValue.IsNil() || secondValue.IsNil() {
		return firstValue.IsNil() == secondValue.IsNil()
	}
	return sameFields(firstValue.Elem(), secondValue.Elem())
}

func sameFields(first, second reflect.Value) bool {
	for i := 0; i < first.NumField(); i++ {
		firstField, secondField := first.Field(i), second.Field(i)
		switch firstField.Type() {
		case positionType, objectType, commentGroupType:
			continue
		}
		switch firstField.Kind() {
		case reflect.Slice:
			if firstField.Len() != secondField.Len() {
				return false
			}
			for j := 0; j < firstField.Len(); j++ {
				if !sameSyntax(asNode(firstField.Index(j)), asNode(secondField.Index(j))) {
					return false
				}
			}
		case reflect.Pointer, reflect.Interface:
			if !sameSyntax(asNode(firstField), asNode(secondField)) {
				return false
			}
		default:
			if firstField.Interface() != secondField.Interface() {
				return false
			}
		}
	}
	return true
}

func asNode(value reflect.Value) ast.Node {
	if value.Kind() == reflect.Interface && value.IsNil() {
		return nil
	}
	node, _ := value.Interface().(ast.Node)
	return node
}

func unparenNode(node ast.Node) ast.Node {
	for {
		paren, ok := node.(*ast.ParenExpr)
		if !ok || paren == nil {
			return node
		}
		node = paren.X
	}
}

// `report.Render`: la expresión impresa con go/printer.
func (c *collector) render(node ast.Node) string {
	var buffer bytes.Buffer
	_ = printer.Fprint(&buffer, c.fset, node)
	return buffer.String()
}

// S1038: `fmt.Print(fmt.Sprintf(...))`, `t.Error(fmt.Sprintf(...))`, `log.Print(fmt.Sprintf(...))` y
// sus pares, que tienen su versión con formato.
func (c *collector) checkSprintfInPrint(call *ast.CallExpr) {
	if replacement, ok := c.fmtPrintReplacement(call); ok {
		c.addReport("s1038", call.Pos(), replacement)
		return
	}
	if replacement, ok := c.methodPrintReplacement(call); ok {
		c.addReport("s1038", call.Pos(), replacement)
		return
	}
	if len(call.Args) == 1 && c.isSprintfCall(call.Args[0]) && logPrinters[c.symbolName(call.Fun)] {
		if mapped, ok := sprintfAlternatives[c.callName(call)]; ok {
			c.addReport("s1038", call.Pos(), mapped.alternative)
		}
	}
}

var logPrinters = map[string]bool{
	"log.Fatal": true, "log.Fatalln": true, "log.Panic": true, "log.Panicln": true, "log.Print": true, "log.Println": true,
}

func (c *collector) isSprintfCall(expression ast.Expr) bool {
	call, ok := ast.Unparen(expression).(*ast.CallExpr)
	return ok && c.symbolName(call.Fun) == "fmt.Sprintf"
}

func (c *collector) fmtPrintReplacement(call *ast.CallExpr) (string, bool) {
	var sprintf ast.Expr
	switch c.symbolName(call.Fun) {
	case "fmt.Print", "fmt.Sprint", "fmt.Println", "fmt.Sprintln":
		if len(call.Args) == 1 {
			sprintf = call.Args[0]
		}
	case "fmt.Fprint", "fmt.Fprintln":
		if len(call.Args) == 2 {
			sprintf = call.Args[1]
		}
	}
	if sprintf == nil || !c.isSprintfCall(sprintf) {
		return "", false
	}
	format := ast.Unparen(sprintf).(*ast.CallExpr).Args
	if len(format) == 0 {
		return "", false
	}
	name := strings.TrimPrefix(c.symbolName(call.Fun), "fmt.")
	if !strings.HasSuffix(name, "ln") {
		return "fmt." + name + "f", true
	}
	if _, ok := ast.Unparen(format[0]).(*ast.BasicLit); !ok {
		return "", false
	}
	return "fmt." + strings.TrimSuffix(name, "ln") + "f", true
}

var sprintfMethods = map[string]bool{
	"Error": true, "Fatal": true, "Fatalln": true, "Log": true, "Panic": true, "Panicln": true,
	"Print": true, "Println": true, "Skip": true,
}

type sprintfAlternative struct {
	receiver    string
	alternative string
}

var sprintfAlternatives = map[string]sprintfAlternative{
	"(*testing.common).Error": {"(*testing.common)", "Errorf"},
	"(testing.TB).Error":      {"(testing.TB)", "Errorf"},
	"(*testing.common).Fatal": {"(*testing.common)", "Fatalf"},
	"(testing.TB).Fatal":      {"(testing.TB)", "Fatalf"},
	"(*testing.common).Log":   {"(*testing.common)", "Logf"},
	"(testing.TB).Log":        {"(testing.TB)", "Logf"},
	"(*testing.common).Skip":  {"(*testing.common)", "Skipf"},
	"(testing.TB).Skip":       {"(testing.TB)", "Skipf"},
	"(*log.Logger).Fatal":     {"(*log.Logger)", "Fatalf"},
	"(*log.Logger).Fatalln":   {"(*log.Logger)", "Fatalf"},
	"(*log.Logger).Panic":     {"(*log.Logger)", "Panicf"},
	"(*log.Logger).Panicln":   {"(*log.Logger)", "Panicf"},
	"(*log.Logger).Print":     {"(*log.Logger)", "Printf"},
	"(*log.Logger).Println":   {"(*log.Logger)", "Printf"},
	"log.Fatal":               {"", "log.Fatalf"},
	"log.Fatalln":             {"", "log.Fatalf"},
	"log.Panic":               {"", "log.Panicf"},
	"log.Panicln":             {"", "log.Panicf"},
	"log.Print":               {"", "log.Printf"},
	"log.Println":             {"", "log.Printf"},
}

func (c *collector) methodPrintReplacement(call *ast.CallExpr) (string, bool) {
	selector, ok := ast.Unparen(call.Fun).(*ast.SelectorExpr)
	if !ok || !sprintfMethods[selector.Sel.Name] || len(call.Args) != 1 || !c.isSprintfCall(call.Args[0]) {
		return "", false
	}
	mapped, ok := sprintfAlternatives[c.callName(call)]
	if !ok {
		return "", false
	}
	receiver := ast.Unparen(selector.X)
	receiverType, ok := c.info.Types[receiver]
	if !ok {
		return "", false
	}
	object, _, _ := types.LookupFieldOrMethod(receiverType.Type, receiverType.Addressable(), nil, mapped.alternative)
	function, ok := object.(*types.Func)
	if !ok || function.FullName() != mapped.receiver+"."+mapped.alternative {
		return "", false
	}
	return c.render(receiver) + "." + mapped.alternative, true
}

var dynamicFormatPrinters = map[string]bool{
	"fmt.Errorf": true, "fmt.Printf": true, "fmt.Sprintf": true, "log.Fatalf": true, "log.Panicf": true,
	"log.Printf": true, "(*log.Logger).Printf": true, "(*testing.common).Logf": true,
	"(*testing.common).Errorf": true, "(*testing.common).Fatalf": true, "(*testing.common).Skipf": true,
	"(testing.TB).Logf": true, "(testing.TB).Errorf": true, "(testing.TB).Fatalf": true, "(testing.TB).Skipf": true,
}

// SA1006: una función con formato que recibe sólo un formato que no es constante.
func (c *collector) checkDynamicFormat(call *ast.CallExpr) {
	name := c.symbolName(call.Fun)
	var format ast.Expr
	switch {
	case dynamicFormatPrinters[name] && len(call.Args) == 1:
		format = call.Args[0]
	case name == "fmt.Fprintf" && len(call.Args) == 2:
		format = call.Args[1]
	default:
		return
	}
	switch ast.Unparen(format).(type) {
	case *ast.CallExpr, *ast.Ident:
	default:
		return
	}
	if _, ok := c.info.TypeOf(format).(*types.Tuple); !ok {
		c.addReport("sa1006", call.Pos(), name)
	}
}

// SA2001: `mu.Lock()` seguido de `mu.Unlock()`, una sección crítica vacía.
func (c *collector) checkEmptyCriticalSection(block *ast.BlockStmt) {
	if c.pkg.Path() == "sync_test" || len(block.List) < 2 {
		return
	}
	for i := range block.List[:len(block.List)-1] {
		firstReceiver, firstMethod, firstOk := c.mutexCall(block.List[i])
		secondReceiver, secondMethod, secondOk := c.mutexCall(block.List[i+1])
		if !firstOk || !secondOk || c.render(firstReceiver) != c.render(secondReceiver) {
			continue
		}
		if (firstMethod == "Lock" && secondMethod == "Unlock") || (firstMethod == "RLock" && secondMethod == "RUnlock") {
			c.addReport("sa2001", block.List[i+1].Pos(), c.render(firstReceiver))
		}
	}
}

func (c *collector) mutexCall(statement ast.Stmt) (ast.Expr, string, bool) {
	expression, ok := statement.(*ast.ExprStmt)
	if !ok {
		return nil, "", false
	}
	call, ok := ast.Unparen(expression.X).(*ast.CallExpr)
	if !ok {
		return nil, "", false
	}
	selector, ok := call.Fun.(*ast.SelectorExpr)
	if !ok {
		return nil, "", false
	}
	function, ok := c.info.ObjectOf(selector.Sel).(*types.Func)
	if !ok {
		return nil, "", false
	}
	signature := function.Type().(*types.Signature)
	if signature.Params().Len() != 0 || signature.Results().Len() != 0 {
		return nil, "", false
	}
	return selector.X, function.Name(), true
}

// SA4004: un loop cuyo cuerpo termina siempre con `return` o `break` en la primera vuelta.
func (c *collector) checkTerminatedLoops(body *ast.BlockStmt, exits map[token.Pos]bool) {
	if body == nil {
		return
	}
	labels := map[types.Object]ast.Stmt{}
	ast.Inspect(body, func(node ast.Node) bool {
		if label, ok := node.(*ast.LabeledStmt); ok {
			labels[c.info.ObjectOf(label.Label)] = label.Stmt
		}
		return true
	})
	ast.Inspect(body, func(node ast.Node) bool {
		var loopBody *ast.BlockStmt
		switch node := node.(type) {
		case *ast.ForStmt:
			loopBody = node.Body
		case *ast.RangeStmt:
			if !allTerms(c.info.TypeOf(node.X), rangesOnce) {
				return true
			}
			loopBody = node.Body
		default:
			return true
		}
		if len(loopBody.List) < 2 {
			return true
		}
		exit, keepLooking := c.unconditionalExit(node, loopBody, labels)
		if exit != nil {
			exits[exit.Pos()] = true
		}
		return keepLooking
	})
}

func rangesOnce(typ types.Type) bool {
	switch typ.Underlying().(type) {
	case *types.Slice, *types.Chan, *types.Basic, *types.Pointer, *types.Array:
		return true
	}
	return false
}

func (c *collector) unconditionalExit(loop ast.Node, body *ast.BlockStmt, labels map[types.Object]ast.Stmt) (ast.Node, bool) {
	targetsLoop := func(branch *ast.BranchStmt) bool {
		return branch.Label == nil || labels[c.info.ObjectOf(branch.Label)] == loop
	}
	var exit ast.Node
	branching := false
	for _, statement := range body.List {
		switch statement := statement.(type) {
		case *ast.BranchStmt:
			if statement.Tok == token.BREAK && targetsLoop(statement) {
				exit = statement
			}
			if statement.Tok == token.CONTINUE && targetsLoop(statement) {
				return nil, false
			}
		case *ast.ReturnStmt:
			exit = statement
		case *ast.IfStmt, *ast.ForStmt, *ast.RangeStmt, *ast.SwitchStmt, *ast.SelectStmt:
			branching = true
		}
	}
	if exit == nil || !branching {
		return nil, false
	}
	ast.Inspect(body, func(node ast.Node) bool {
		branch, ok := node.(*ast.BranchStmt)
		if !ok || exit == nil {
			return exit != nil
		}
		if branch.Tok == token.GOTO || (branch.Tok == token.CONTINUE && targetsLoop(branch)) {
			exit = nil
		}
		return exit != nil
	})
	return exit, true
}

// SA4011: un `break` sin etiqueta al final de un case dentro de un loop, que sólo sale del switch.
func (c *collector) checkIneffectiveBreaks(body *ast.BlockStmt) {
	for _, statement := range body.List {
		var clauses [][]ast.Stmt
		switch statement := statement.(type) {
		case *ast.SwitchStmt:
			for _, clause := range statement.Body.List {
				clauses = append(clauses, clause.(*ast.CaseClause).Body)
			}
		case *ast.SelectStmt:
			for _, clause := range statement.Body.List {
				clauses = append(clauses, clause.(*ast.CommClause).Body)
			}
		}
		for _, clause := range clauses {
			for _, last := range clauseEnds(clause) {
				if branch, ok := last.(*ast.BranchStmt); ok && branch.Tok == token.BREAK && branch.Label == nil {
					c.addReport("sa4011", branch.Pos(), "")
				}
			}
		}
	}
}

func clauseEnds(clause []ast.Stmt) []ast.Stmt {
	if len(clause) == 0 {
		return nil
	}
	last := clause[len(clause)-1]
	branch, ok := last.(*ast.IfStmt)
	if !ok {
		return []ast.Stmt{last}
	}
	if len(branch.Body.List) == 0 {
		return nil
	}
	ends := []ast.Stmt{branch.Body.List[len(branch.Body.List)-1]}
	if alternative, ok := branch.Else.(*ast.BlockStmt); ok && len(alternative.List) != 0 {
		ends = append(ends, alternative.List[len(alternative.List)-1])
	}
	return ends
}

// SA4016: `x & 0`, `x | 0` y `x ^ 0`, también con una constante del paquete que vale 0 por ser
// `iota`.
func (c *collector) checkIneffectiveBitOperation(expression *ast.BinaryExpr) {
	if expression.Op != token.AND && expression.Op != token.OR && expression.Op != token.XOR {
		return
	}
	if !allTerms(c.info.TypeOf(expression), isInteger) {
		return
	}
	if ident, ok := expression.Y.(*ast.Ident); ok {
		if named, ok := c.info.ObjectOf(ident).(*types.Const); ok && c.iotaZeroConstants[named] {
			c.addReport("sa4016", expression.Pos(), c.render(expression))
		}
		return
	}
	if value, ok := c.integerLiteral(expression.Y); ok && constant.Compare(value, token.EQL, constant.MakeInt64(0)) {
		c.addReport("sa4016", expression.Pos(), c.render(expression))
	}
}

func isInteger(typ types.Type) bool {
	basic, ok := typ.Underlying().(*types.Basic)
	return ok && basic.Info()&types.IsInteger != 0
}

// Las constantes del paquete declaradas solas como `X = iota` que valen 0.
func (c *collector) indexIotaConstants(files []*ast.File) {
	c.iotaZeroConstants = map[*types.Const]bool{}
	for _, file := range files {
		ast.Inspect(file, func(node ast.Node) bool {
			if value, ok := node.(*ast.ValueSpec); ok {
				c.indexIotaConstant(value)
			}
			return true
		})
	}
}

func (c *collector) indexIotaConstant(value *ast.ValueSpec) {
	if len(value.Names) != 1 || len(value.Values) != 1 {
		return
	}
	ident, ok := value.Values[0].(*ast.Ident)
	if !ok || c.info.ObjectOf(ident) != types.Universe.Lookup("iota") {
		return
	}
	named, ok := c.info.Defs[value.Names[0]].(*types.Const)
	if !ok {
		return
	}
	if number, _ := constant.Int64Val(named.Val()); number == 0 {
		c.iotaZeroConstants[named] = true
	}
}

// SA6002: `pool.Put(x)` de un sync.Pool con algo que no es un puntero, que se copia a una interfaz.
func (c *collector) checkPoolPut(call *ast.CallExpr) {
	if c.symbolName(call.Fun) != "(*sync.Pool).Put" || len(call.Args) != 1 {
		return
	}
	typ := types.Default(c.info.TypeOf(call.Args[0]))
	if typ == nil || c.info.Types[call.Args[0]].IsNil() {
		return
	}
	_, isSlice := typ.Underlying().(*types.Slice)
	if !maybePointerLike(typ) || isSlice {
		c.addReport("sa6002", call.Args[0].Pos(), "")
	}
}

// `typeutil.MaybePointerLike`, con los términos de `typeTerms`.
func maybePointerLike(typ types.Type) bool {
	switch underlying := typ.Underlying().(type) {
	case *types.Interface:
		if underlying.IsMethodSet() {
			return true
		}
		terms, ok := typeTerms(typ)
		if !ok {
			return false
		}
		for _, term := range terms {
			if maybePointerLike(term) {
				return true
			}
		}
		return len(terms) == 0
	case *types.Chan, *types.Map, *types.Signature, *types.Pointer, *types.Slice:
		return true
	case *types.Basic:
		return underlying.Kind() == types.UnsafePointer
	}
	return false
}

// SA6005: `strings.ToLower(a) == strings.ToLower(b)` en vez de `strings.EqualFold`.
func (c *collector) checkCaseInsensitiveComparison(expression *ast.BinaryExpr) {
	if expression.Op != token.EQL && expression.Op != token.NEQ {
		return
	}
	first, ok := ast.Unparen(expression.X).(*ast.CallExpr)
	if !ok || len(first.Args) != 1 {
		return
	}
	name := c.symbolName(first.Fun)
	if name != "strings.ToLower" && name != "strings.ToUpper" {
		return
	}
	second, ok := ast.Unparen(expression.Y).(*ast.CallExpr)
	if !ok || len(second.Args) != 1 || c.symbolName(second.Fun) != name {
		return
	}
	replacement := "strings.EqualFold"
	if expression.Op == token.NEQ {
		replacement = "!" + replacement
	}
	c.addReport("sa6005", expression.Pos(), replacement)
}

// SA9009: `// go:noinline` en la columna 1, una directiva que el espacio deja sin efecto.
func (c *collector) checkSpacedDirectives(file *ast.File) {
	for _, group := range file.Comments {
		for _, comment := range group.List {
			if !strings.HasPrefix(comment.Text, "//") || c.fset.PositionFor(comment.Pos(), false).Column != 1 {
				continue
			}
			text := strings.TrimLeft(comment.Text[2:], " \t")
			if len(text) == len(comment.Text[2:]) || !strings.HasPrefix(text, "go:") {
				continue
			}
			if directive := text[3:]; directive != "" && directive[0] >= 'a' && directive[0] <= 'z' {
				c.addReport("sa9009", comment.Pos(), comment.Text)
			}
		}
	}
}

// ST1008: un `error` que no es el último resultado, salvo `(…, error, bool)`.
func (c *collector) checkErrorPosition(typ types.Type) {
	signature, ok := typ.(*types.Signature)
	if !ok {
		return
	}
	results := signature.Results()
	if results.Len() < 2 {
		return
	}
	errorType := types.Universe.Lookup("error").Type()
	isResult := func(index int, expected types.Type) bool {
		return types.Unalias(results.At(index).Type()) == expected
	}
	last := results.Len() - 1
	if isResult(last, errorType) || (isResult(last, types.Universe.Lookup("bool").Type()) && isResult(last-1, errorType)) {
		return
	}
	for i := last - 1; i >= 0; i-- {
		if isResult(i, errorType) {
			c.addReport("st1008", results.At(i).Pos(), "")
			return
		}
	}
}

// QF1007: `x := false` seguido de `if cond { x = true }`, que se junta en `x := cond`.
func (c *collector) checkConditionalAssignments(body *ast.BlockStmt) {
	if body == nil {
		return
	}
	for i := 0; i+1 < len(body.List); i++ {
		declared, declaredValue, ok := c.boolAssignment(body.List[i], token.DEFINE)
		if !ok {
			continue
		}
		branch, ok := body.List[i+1].(*ast.IfStmt)
		if !ok || branch.Init != nil || branch.Else != nil || len(branch.Body.List) != 1 {
			continue
		}
		assigned, assignedValue, ok := c.boolAssignment(branch.Body.List[0], token.ASSIGN)
		if ok && declared == assigned && declaredValue != assignedValue {
			c.addReport("qf1007", body.List[i].Pos(), "")
		}
	}
}

func (c *collector) boolAssignment(statement ast.Stmt, operator token.Token) (types.Object, string, bool) {
	assignment, ok := statement.(*ast.AssignStmt)
	if !ok || assignment.Tok != operator || len(assignment.Lhs) != 1 || len(assignment.Rhs) != 1 {
		return nil, "", false
	}
	target, ok := c.patternObject(assignment.Lhs[0])
	if !ok {
		return nil, "", false
	}
	for _, value := range []string{"true", "false"} {
		if c.isBuiltin(assignment.Rhs[0], value) {
			return target, value, true
		}
	}
	return nil, "", false
}
