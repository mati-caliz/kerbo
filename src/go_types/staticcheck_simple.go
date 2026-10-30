// Las reglas de `simple` de staticcheck v0.8.2 calcadas sobre go/ast y go/types: S1009, S1011, S1017,
// S1029, S1030, S1032, S1035, S1038, S1039 y S1040.
package main

import (
	"go/ast"
	"go/constant"
	"go/token"
	"go/types"
	"strings"
)

func (c *collector) collectStaticcheckSimple(file *ast.File) {
	sortReports := map[token.Pos]string{}
	elseBranches := map[ast.Stmt]bool{}
	var stack []ast.Node
	ast.Inspect(file, func(node ast.Node) bool {
		if node == nil {
			stack = stack[:len(stack)-1]
			return true
		}
		var parent ast.Node
		if len(stack) > 0 {
			parent = stack[len(stack)-1]
		}
		stack = append(stack, node)
		switch node := node.(type) {
		case *ast.BinaryExpr:
			c.checkRedundantNilCheck(node)
		case *ast.RangeStmt:
			c.checkLoopAppend(node)
			c.checkRangeStringRunes(node)
		case *ast.IfStmt:
			c.checkTrimIf(node, elseBranches)
		case *ast.CallExpr:
			c.checkBufferConversion(node, parent)
			c.checkCanonicalHeaderKey(node)
			c.checkSprintfInPrint(node)
			c.checkSprintLiteral(node)
		case *ast.FuncDecl:
			c.collectSortConversions(node.Body, sortReports)
		case *ast.FuncLit:
			c.collectSortConversions(node.Body, sortReports)
		case *ast.TypeAssertExpr:
			c.checkSameTypeAssertion(node)
		}
		return true
	})
	for pos, subject := range sortReports {
		c.addReport("s1032", pos, subject)
	}
}

// `typeutil.All` sobre los términos de `typeTerms`.
func allTerms(typ types.Type, accepts func(types.Type) bool) bool {
	if typ == nil {
		return false
	}
	terms, ok := typeTerms(typ)
	if !ok || len(terms) == 0 {
		return false
	}
	for _, term := range terms {
		if !accepts(term) {
			return false
		}
	}
	return true
}

func (c *collector) isBuiltin(expression ast.Expr, name string) bool {
	ident, ok := ast.Unparen(expression).(*ast.Ident)
	return ok && ident.Name == name && c.info.ObjectOf(ident) == types.Universe.Lookup(name)
}

// La variable de un patrón de staticcheck (`Object`): un identificador, y su objeto.
func (c *collector) patternObject(expression ast.Expr) (types.Object, bool) {
	ident, ok := ast.Unparen(expression).(*ast.Ident)
	if !ok {
		return nil, false
	}
	return c.info.ObjectOf(ident), true
}

// Un objeto ya ligado en el patrón vuelve a aparecer: un identificador o un selector que lo nombra.
func (c *collector) refersToObject(expression ast.Expr, object types.Object) bool {
	switch inner := ast.Unparen(expression).(type) {
	case *ast.Ident:
		return c.info.ObjectOf(inner) == object
	case *ast.SelectorExpr:
		return c.info.ObjectOf(inner.Sel) == object
	}
	return false
}

func (c *collector) mentions(expression ast.Expr, object types.Object) bool {
	found := false
	ast.Inspect(expression, func(node ast.Node) bool {
		if ident, ok := node.(*ast.Ident); ok && c.info.ObjectOf(ident) == object {
			found = true
		}
		return !found
	})
	return found
}

// S1009: `x == nil || len(x) == 0` y sus variantes, con `x` un slice, un mapa o un canal.
func (c *collector) checkRedundantNilCheck(expression *ast.BinaryExpr) {
	nilCheck, ok := ast.Unparen(expression.X).(*ast.BinaryExpr)
	if !ok || (nilCheck.Op != token.EQL && nilCheck.Op != token.NEQ) {
		return
	}
	lengthCheck, ok := ast.Unparen(expression.Y).(*ast.BinaryExpr)
	if !ok || (expression.Op != token.LOR && expression.Op != token.LAND) {
		return
	}
	call, ok := ast.Unparen(lengthCheck.X).(*ast.CallExpr)
	if !ok || !c.isBuiltin(call.Fun, "len") || len(call.Args) != 1 {
		return
	}
	subject := ast.Unparen(nilCheck.X)
	if !sameSyntax(subject, call.Args[0]) || mayHaveSideEffects(subject) {
		return
	}
	equalsNil := expression.Op == token.LOR
	if (equalsNil && nilCheck.Op != token.EQL) || (!equalsNil && nilCheck.Op != token.NEQ) {
		return
	}
	if !c.info.Types[ast.Unparen(nilCheck.Y)].IsNil() {
		return
	}
	isConstant, isZero := c.constantZero(ast.Unparen(lengthCheck.Y))
	if !isConstant || !redundantLengthComparison(equalsNil, lengthCheck.Op, isZero) {
		return
	}
	nilType := ""
	if allTerms(c.info.TypeOf(subject), func(term types.Type) bool {
		switch term.Underlying().(type) {
		case *types.Slice:
			nilType = "nil slices"
		case *types.Map:
			nilType = "nil maps"
		case *types.Chan:
			nilType = "nil channels"
		default:
			return false
		}
		return true
	}) {
		c.addReport("s1009", expression.Pos(), nilType)
	}
}

func (c *collector) constantZero(expression ast.Expr) (isConstant bool, isZero bool) {
	if _, ok := expression.(*ast.BasicLit); ok {
		value, isInteger := c.integerLiteral(expression)
		return true, isInteger && constant.Compare(value, token.EQL, constant.MakeInt64(0))
	}
	ident, ok := expression.(*ast.Ident)
	if !ok {
		return false, false
	}
	named, ok := c.info.ObjectOf(ident).(*types.Const)
	if !ok {
		return false, false
	}
	return true, named.Val().Kind() == constant.Int && named.Val().String() == "0"
}

func redundantLengthComparison(equalsNil bool, operator token.Token, isZero bool) bool {
	if equalsNil {
		switch operator {
		case token.EQL:
			return isZero
		case token.LEQ:
			return true
		case token.LSS:
			return !isZero
		}
		return false
	}
	switch operator {
	case token.EQL, token.GEQ:
		return !isZero
	case token.NEQ:
		return isZero
	case token.GTR:
		return true
	}
	return false
}

// S1011: un loop que agrega de a uno los elementos de un slice a otro del mismo tipo.
func (c *collector) checkLoopAppend(loop *ast.RangeStmt) {
	lhs, value, index, ok := c.loopAppendParts(loop)
	if !ok {
		return
	}
	if value != nil && c.mentions(lhs, value) {
		return
	}
	if index != nil && (mayHaveSideEffects(loop.X) || c.mentions(lhs, index)) {
		return
	}
	if mayHaveSideEffects(lhs) || !types.Identical(c.info.TypeOf(ast.Unparen(loop.X)), c.info.TypeOf(lhs)) {
		return
	}
	c.addReport("s1011", loop.Pos(), "")
}

// Las tres formas del patrón de S1011: por valor, por índice y por índice con el elemento en una
// variable.
func (c *collector) loopAppendParts(loop *ast.RangeStmt) (lhs ast.Expr, value, index types.Object, ok bool) {
	statements := loop.Body.List
	if isBlank(loop.Key) && loop.Value != nil && len(statements) == 1 {
		if value, ok = c.patternObject(loop.Value); ok {
			if lhs, ok = c.appendOf(statements[0], func(element ast.Expr) bool { return c.refersToObject(element, value) }); ok {
				return lhs, value, nil, true
			}
		}
	}
	if loop.Key == nil || loop.Value != nil {
		return nil, nil, nil, false
	}
	if index, ok = c.patternObject(loop.Key); !ok {
		return nil, nil, nil, false
	}
	indexesRange := func(expression ast.Expr) bool {
		element, ok := ast.Unparen(expression).(*ast.IndexExpr)
		return ok && sameSyntax(element.X, loop.X) && c.refersToObject(element.Index, index)
	}
	switch len(statements) {
	case 1:
		lhs, ok = c.appendOf(statements[0], indexesRange)
		return lhs, nil, index, ok
	case 2:
		assignment, isAssignment := statements[0].(*ast.AssignStmt)
		if !isAssignment || assignment.Tok != token.DEFINE || len(assignment.Lhs) != 1 || len(assignment.Rhs) != 1 || !indexesRange(assignment.Rhs[0]) {
			return nil, nil, nil, false
		}
		if value, ok = c.patternObject(assignment.Lhs[0]); !ok {
			return nil, nil, nil, false
		}
		lhs, ok = c.appendOf(statements[1], func(element ast.Expr) bool { return c.refersToObject(element, value) })
		return lhs, value, index, ok
	}
	return nil, nil, nil, false
}

// `lhs = append(lhs, elemento)`, con lo que se agrega validado aparte.
func (c *collector) appendOf(statement ast.Stmt, isElement func(ast.Expr) bool) (ast.Expr, bool) {
	assignment, ok := statement.(*ast.AssignStmt)
	if !ok || assignment.Tok != token.ASSIGN || len(assignment.Lhs) != 1 || len(assignment.Rhs) != 1 {
		return nil, false
	}
	call, ok := ast.Unparen(assignment.Rhs[0]).(*ast.CallExpr)
	if !ok || !c.isBuiltin(call.Fun, "append") || len(call.Args) != 2 {
		return nil, false
	}
	lhs := ast.Unparen(assignment.Lhs[0])
	return lhs, sameSyntax(call.Args[0], lhs) && isElement(call.Args[1])
}

// S1029 y SA6003: `for _, r := range []rune(s)` convierte el string sólo para recorrerlo.
func (c *collector) checkRangeStringRunes(loop *ast.RangeStmt) {
	if !isBlank(loop.Key) || loop.Value == nil || isBlank(loop.Value) {
		return
	}
	conversion, ok := ast.Unparen(loop.X).(*ast.CallExpr)
	if !ok || len(conversion.Args) != 1 || !c.info.Types[conversion.Fun].IsType() {
		return
	}
	source, ok := coreType(types.Default(c.info.TypeOf(conversion.Args[0]))).(*types.Basic)
	if !ok || source.Kind() != types.String {
		return
	}
	target, ok := coreType(c.info.TypeOf(conversion)).(*types.Slice)
	if !ok {
		return
	}
	if element, ok := types.Unalias(target.Elem()).(*types.Basic); !ok || element.Kind() != types.Int32 {
		return
	}
	c.addReport("s1029", loop.Pos(), "")
	c.addReport("sa6003", loop.Pos(), "")
}

// S1017: `if strings.HasPrefix(s, p) { s = s[len(p):] }` y las demás que hacen lo que ya hace el Trim.
func (c *collector) checkTrimIf(statement *ast.IfStmt, elseBranches map[ast.Stmt]bool) {
	if statement.Init != nil {
		return
	}
	if statement.Else != nil {
		elseBranches[statement.Else] = true
		return
	}
	if elseBranches[statement] || len(statement.Body.List) != 1 {
		return
	}
	condition, ok := statement.Cond.(*ast.CallExpr)
	if !ok {
		return
	}
	conditionName := c.callName(condition)
	pkg, function, ok := strings.Cut(conditionName, ".")
	if !ok || (pkg != "strings" && pkg != "bytes") || (function != "HasPrefix" && function != "HasSuffix" && function != "Contains") {
		return
	}
	assignment, ok := statement.Body.List[0].(*ast.AssignStmt)
	if !ok || assignment.Tok != token.ASSIGN || len(assignment.Lhs) != 1 || len(assignment.Rhs) != 1 {
		return
	}
	if !c.sameNonDynamic(condition.Args[0], assignment.Lhs[0]) {
		return
	}
	switch rhs := assignment.Rhs[0].(type) {
	case *ast.CallExpr:
		if len(rhs.Args) < 2 || !c.sameNonDynamic(condition.Args[0], rhs.Args[0]) || !c.sameNonDynamic(condition.Args[1], rhs.Args[1]) {
			return
		}
		replacement := c.callName(rhs)
		if replacement == pkg+"."+trimReplacements[function] {
			c.addReport("s1017", statement.Pos(), replacement)
		}
	case *ast.SliceExpr:
		if c.trimsWithSlice(rhs, condition, pkg, function) {
			c.addReport("s1017", statement.Pos(), pkg+"."+trimReplacements[function])
		}
	}
}

var trimReplacements = map[string]string{"HasPrefix": "TrimPrefix", "HasSuffix": "TrimSuffix", "Contains": "Replace"}

func (c *collector) trimsWithSlice(slice *ast.SliceExpr, condition *ast.CallExpr, pkg, function string) bool {
	if slice.Slice3 || !c.sameNonDynamic(slice.X, condition.Args[0]) {
		return false
	}
	switch function {
	case "HasPrefix":
		return slice.High == nil && c.validTrimOffset(slice.Low, condition, pkg)
	case "HasSuffix":
		if slice.Low != nil {
			if start, ok := c.constantInt(slice.Low); !ok || start != 0 {
				return false
			}
		}
		end, ok := slice.High.(*ast.BinaryExpr)
		return ok && end.Op == token.SUB && c.isLenOf(end.X, condition.Args[0]) && c.validTrimOffset(end.Y, condition, pkg)
	}
	return false
}

func (c *collector) validTrimOffset(offset ast.Expr, condition *ast.CallExpr, pkg string) bool {
	switch offset := offset.(type) {
	case *ast.CallExpr:
		return c.isLenOf(offset, condition.Args[1])
	case *ast.BasicLit:
		if _, ok := condition.Args[1].(*ast.BasicLit); !ok || pkg != "strings" {
			return false
		}
		affix := c.info.Types[condition.Args[1]].Value
		length, ok := c.constantInt(offset)
		return ok && affix != nil && affix.Kind() == constant.String && length == int64(len(constant.StringVal(affix)))
	}
	return false
}

func (c *collector) constantInt(expression ast.Expr) (int64, bool) {
	value := c.info.Types[expression].Value
	if value == nil || value.Kind() != constant.Int {
		return 0, false
	}
	return constant.Int64Val(value)
}

func (c *collector) isLenOf(expression ast.Expr, subject ast.Expr) bool {
	call, ok := expression.(*ast.CallExpr)
	return ok && c.builtinCallName(call) == "len" && len(call.Args) == 1 && c.sameNonDynamic(call.Args[0], subject)
}

// `code.CallName` cuando lo llamado es un builtin: el nombre solo.
func (c *collector) builtinCallName(call *ast.CallExpr) string {
	if ident, ok := ast.Unparen(call.Fun).(*ast.Ident); ok {
		if builtin, ok := c.info.ObjectOf(ident).(*types.Builtin); ok {
			return builtin.Name()
		}
	}
	return c.callName(call)
}

func (c *collector) sameNonDynamic(first, second ast.Node) bool {
	switch first := first.(type) {
	case *ast.Ident:
		other, ok := second.(*ast.Ident)
		return ok && c.info.ObjectOf(first) == c.info.ObjectOf(other)
	case *ast.SelectorExpr, *ast.IndexExpr, *ast.BasicLit:
		return astEqual(first, second)
	}
	return false
}

// S1030: `string(buf.Bytes())` y `[]byte(buf.String())` de un bytes.Buffer.
func (c *collector) checkBufferConversion(call *ast.CallExpr, parent ast.Node) {
	if c.pkg.Path() == "bytes" || c.pkg.Path() == "bytes_test" || len(call.Args) != 1 {
		return
	}
	inner, ok := ast.Unparen(call.Args[0]).(*ast.CallExpr)
	if !ok || len(inner.Args) != 0 {
		return
	}
	if _, ok := ast.Unparen(inner.Fun).(*ast.SelectorExpr); !ok {
		return
	}
	argument, isCall := call.Args[0].(*ast.CallExpr)
	argumentName := ""
	if isCall {
		argumentName = c.callName(argument)
	}
	typ := types.Unalias(c.info.TypeOf(call.Fun))
	if typ == types.Universe.Lookup("string").Type() && argumentName == "(*bytes.Buffer).Bytes" {
		if _, ok := parent.(*ast.IndexExpr); !ok {
			c.addReport("s1030", call.Pos(), "String")
		}
		return
	}
	if slice, ok := typ.(*types.Slice); ok && types.Unalias(slice.Elem()) == types.Universe.Lookup("byte").Type() && argumentName == "(*bytes.Buffer).String" {
		c.addReport("s1030", call.Pos(), "Bytes")
	}
}

// S1032: `sort.Sort(sort.IntSlice(x))` y sus pares, salvo en una función que también ordena otra cosa
// con sort.Sort.
func (c *collector) collectSortConversions(body *ast.BlockStmt, reports map[token.Pos]string) {
	if body == nil {
		return
	}
	found := map[token.Pos]string{}
	permissible := false
	ast.Inspect(body, func(node ast.Node) bool {
		if permissible {
			return false
		}
		call, ok := node.(*ast.CallExpr)
		if !ok || c.callName(call) != "sort.Sort" {
			return true
		}
		name := c.sortConversion(call)
		if name == "" {
			permissible = true
			return false
		}
		found[call.Pos()] = name
		return true
	})
	if permissible {
		return
	}
	for pos, name := range found {
		reports[pos] = name
	}
}

var sortConversions = map[string]bool{"sort.IntSlice": true, "sort.Float64Slice": true, "sort.StringSlice": true}

func (c *collector) sortConversion(call *ast.CallExpr) string {
	conversion, ok := call.Args[0].(*ast.CallExpr)
	if !ok {
		return ""
	}
	selector, ok := conversion.Fun.(*ast.SelectorExpr)
	if !ok {
		return ""
	}
	ident, ok := selector.X.(*ast.Ident)
	if !ok || c.info.Selections[selector] != nil {
		return ""
	}
	pkg, ok := c.info.ObjectOf(ident).(*types.PkgName)
	if !ok {
		return ""
	}
	name := pkg.Imported().Path() + "." + selector.Sel.Name
	if !sortConversions[name] {
		return ""
	}
	return name
}

// S1035: `http.CanonicalHeaderKey` en el nombre que se le pasa a un método de http.Header.
func (c *collector) checkCanonicalHeaderKey(call *ast.CallExpr) {
	method := c.symbolName(call.Fun)
	if !headerMethods[method] || len(call.Args) == 0 {
		return
	}
	canonical, ok := ast.Unparen(call.Args[0]).(*ast.CallExpr)
	if !ok || len(canonical.Args) != 1 || c.symbolName(canonical.Fun) != "net/http.CanonicalHeaderKey" {
		return
	}
	c.addReport("s1035", canonical.Pos(), method)
}

var headerMethods = map[string]bool{
	"(net/http.Header).Add": true, "(net/http.Header).Del": true,
	"(net/http.Header).Get": true, "(net/http.Header).Set": true,
}

// S1039: `fmt.Sprint("x")` y `fmt.Sprintf("x")` sin verbos.
func (c *collector) checkSprintLiteral(call *ast.CallExpr) {
	name := c.symbolName(call.Fun)
	if (name != "fmt.Sprint" && name != "fmt.Sprintf") || len(call.Args) != 1 {
		return
	}
	literal, ok := ast.Unparen(call.Args[0]).(*ast.BasicLit)
	if !ok || literal.Kind != token.STRING || (name == "fmt.Sprintf" && strings.ContainsRune(literal.Value, '%')) {
		return
	}
	c.addReport("s1039", call.Pos(), name)
}

// S1040: una aserción a la interfaz que la expresión ya tiene.
func (c *collector) checkSameTypeAssertion(assertion *ast.TypeAssertExpr) {
	if assertion.Type == nil {
		return
	}
	asserted := c.info.TypeOf(assertion.Type)
	if asserted != nil && types.IsInterface(asserted) && types.Identical(asserted, c.info.TypeOf(assertion.X)) {
		c.addReport("s1040", assertion.Pos(), "")
	}
}
