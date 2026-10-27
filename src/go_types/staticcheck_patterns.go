// Más reglas chicas de staticcheck v0.8.2 calcadas sobre go/ast y go/types: ST1019, SA1029, SA9004,
// QF1009, ST1013, S1000, ST1017, ST1006, S1008 y S1002.
package main

import (
	"go/ast"
	"go/constant"
	"go/token"
	"go/types"
	"strconv"
	"strings"
)

func (c *collector) collectStaticcheckPatterns(file *ast.File, filename string) {
	inTest := strings.HasSuffix(filename, "_test.go")
	comments := ast.NewCommentMap(c.fset, file, file.Comments)
	c.checkDuplicateImports(file)
	selectsInLoops := map[*ast.SelectStmt]bool{}
	ast.Inspect(file, func(node ast.Node) bool {
		switch node := node.(type) {
		case *ast.CallExpr:
			c.checkContextKey(node)
			c.checkHTTPStatusLiteral(node)
		case *ast.GenDecl:
			c.checkConstGroupTypes(node)
		case *ast.BinaryExpr:
			c.checkTimeEquality(node)
			c.checkYodaCondition(node)
			if !inTest {
				c.checkBoolComparison(node)
			}
		case *ast.ForStmt:
			c.checkSelectLoop(node, selectsInLoops)
		case *ast.SelectStmt:
			if len(node.Body.List) == 1 && !selectsInLoops[node] {
				c.addReport("s1000", node.Pos(), "")
			}
		case *ast.FuncDecl:
			c.checkReceiverName(node)
		case *ast.BlockStmt:
			c.checkIfReturnBool(node, comments)
		}
		return true
	})
}

// ST1019: el mismo paquete importado más de una vez; golangci-lint también marca cada import de más.
func (c *collector) checkDuplicateImports(file *ast.File) {
	byPath := map[string][]*ast.ImportSpec{}
	var paths []string
	for _, spec := range file.Imports {
		if spec.Name != nil && spec.Name.Name == "_" {
			continue
		}
		if _, seen := byPath[spec.Path.Value]; !seen {
			paths = append(paths, spec.Path.Value)
		}
		byPath[spec.Path.Value] = append(byPath[spec.Path.Value], spec)
	}
	for _, path := range paths {
		specs := byPath[path]
		if path == strconv.Quote("unsafe") || len(specs) < 2 {
			continue
		}
		for _, spec := range specs {
			c.addReport("st1019", spec.Pos(), path)
		}
	}
}

// SA1029: la clave de `context.WithValue` de un tipo básico, un struct vacío o un tipo no comparable.
func (c *collector) checkContextKey(call *ast.CallExpr) {
	if c.symbolName(call.Fun) != "context.WithValue" || len(call.Args) != 3 {
		return
	}
	key := call.Args[1]
	typ := c.info.TypeOf(key)
	if typ == nil {
		return
	}
	if basic, ok := typ.(*types.Basic); ok && basic.Info()&types.IsUntyped != 0 {
		if basic.Kind() == types.UntypedNil {
			return
		}
		typ = types.Default(basic)
	}
	if _, ok := types.Unalias(typ).(*types.Basic); ok {
		c.addReport("sa1029", key.Pos(), types.TypeString(typ, types.RelativeTo(c.pkg)))
		return
	}
	if structure, ok := typ.(*types.Struct); (ok && structure.NumFields() == 0) || !types.Comparable(typ) {
		c.addReport("sa1029", key.Pos(), types.TypeString(typ, types.RelativeTo(c.pkg)))
	}
}

// SA9004: en un grupo de constantes sólo la primera tiene tipo y las demás son literales sueltos.
func (c *collector) checkConstGroupTypes(declaration *ast.GenDecl) {
	if !declaration.Lparen.IsValid() || declaration.Tok != token.CONST {
		return
	}
	for _, group := range groupSpecs(c.fset, declaration.Specs) {
		if len(group) < 2 || group[0].(*ast.ValueSpec).Type == nil {
			continue
		}
		if c.untypedLiteralsFollow(group) {
			c.addReport("sa9004", group[0].Pos(), types.ExprString(group[0].(*ast.ValueSpec).Type))
		}
	}
}

// `astutil.GroupSpecs`: las especificaciones en líneas seguidas, sin las líneas en blanco entre medio.
func groupSpecs(fset *token.FileSet, specs []ast.Spec) [][]ast.Spec {
	if len(specs) == 0 {
		return nil
	}
	groups := [][]ast.Spec{{specs[0]}}
	for _, spec := range specs[1:] {
		last := groups[len(groups)-1]
		if fset.PositionFor(spec.Pos(), false).Line-1 != fset.PositionFor(last[len(last)-1].End(), false).Line {
			groups = append(groups, nil)
		}
		groups[len(groups)-1] = append(groups[len(groups)-1], spec)
	}
	return groups
}

func (c *collector) untypedLiteralsFollow(group []ast.Spec) bool {
	firstType := c.info.TypeOf(group[0].(*ast.ValueSpec).Values[0])
	for i, spec := range group {
		value := spec.(*ast.ValueSpec)
		if i > 0 && value.Type != nil {
			return false
		}
		if len(value.Names) != 1 || len(value.Values) != 1 {
			return false
		}
		if !types.ConvertibleTo(c.info.TypeOf(value.Values[0]), firstType) {
			return false
		}
		switch literal := value.Values[0].(type) {
		case *ast.BasicLit:
		case *ast.UnaryExpr:
			if _, ok := literal.X.(*ast.BasicLit); !ok {
				return false
			}
		default:
			return false
		}
	}
	return true
}

// QF1009: dos `time.Time` comparados con `==` en vez de con `Equal`.
func (c *collector) checkTimeEquality(expression *ast.BinaryExpr) {
	if expression.Op == token.EQL && isTypeWithName(c.info.TypeOf(expression.X), "time.Time") &&
		isTypeWithName(c.info.TypeOf(expression.Y), "time.Time") {
		c.addReport("qf1009", expression.Pos(), "")
	}
}

// `typeutil.IsTypeWithName` de staticcheck: el tipo (o lo que nombra su alias) se llama así.
func isTypeWithName(typ types.Type, name string) bool {
	switch typ := typ.(type) {
	case *types.Basic:
		return typ.Name() == name
	case *types.Named:
		return isTypeName(typ.Obj(), name)
	case *types.Alias:
		return isTypeName(typ.Obj(), name) || isTypeWithName(types.Unalias(typ), name)
	}
	return false
}

func isTypeName(object *types.TypeName, name string) bool {
	qualifier := ""
	if index := strings.LastIndex(name, "."); index != -1 {
		qualifier, name = name[:index], name[index+1:]
	}
	if object.Name() == name &&
		((qualifier == "" && object.Pkg() == nil) || (object.Pkg() != nil && object.Pkg().Path() == qualifier)) {
		return true
	}
	if !object.IsAlias() {
		return false
	}
	if named, ok := types.Unalias(object.Type()).(interface{ Obj() *types.TypeName }); ok {
		return isTypeName(named.Obj(), name)
	}
	return false
}

// Los códigos que staticcheck (y golangci-lint) dejan escribir como número por defecto.
var allowedHTTPStatusCodes = map[int64]bool{200: true, 400: true, 404: true, 500: true}

// La posición del código de estado en cada función de net/http que lo recibe.
var httpStatusArguments = map[string]int{
	"net/http.Error":           2,
	"net/http.Redirect":        3,
	"net/http.StatusText":      0,
	"net/http.RedirectHandler": 1,
}

// ST1013: un código de estado HTTP escrito como número donde net/http tiene la constante.
func (c *collector) checkHTTPStatusLiteral(call *ast.CallExpr) {
	argument, ok := httpStatusArguments[c.symbolName(call.Fun)]
	if !ok || argument >= len(call.Args) {
		return
	}
	literal := call.Args[argument]
	value, ok := c.integerLiteral(literal)
	if !ok {
		return
	}
	code, ok := constant.Int64Val(value)
	if !ok || allowedHTTPStatusCodes[code] {
		return
	}
	if name, ok := httpStatusCodes[code]; ok {
		c.addReport("st1013", literal.Pos(), "http."+name)
	}
}

// `code.IntegerLiteral`: un literal entero, con signo o entre paréntesis.
func (c *collector) integerLiteral(expression ast.Expr) (constant.Value, bool) {
	switch inner := ast.Unparen(expression).(type) {
	case *ast.BasicLit:
		if inner.Kind != token.INT {
			return nil, false
		}
	case *ast.UnaryExpr:
		if inner.Op != token.ADD && inner.Op != token.SUB {
			return nil, false
		}
		if _, ok := c.integerLiteral(inner.X); !ok {
			return nil, false
		}
	default:
		return nil, false
	}
	value := c.info.Types[ast.Unparen(expression)].Value
	return value, value != nil
}

// S1000: `for { select { case <-ch: } }` con un solo case de recepción; el `select` de un solo
// case suelto se marca aparte.
func (c *collector) checkSelectLoop(loop *ast.ForStmt, selectsInLoops map[*ast.SelectStmt]bool) {
	if loop.Init != nil || loop.Cond != nil || loop.Post != nil || len(loop.Body.List) != 1 {
		return
	}
	statement, ok := unlabel(loop.Body.List[0]).(*ast.SelectStmt)
	if !ok || len(statement.Body.List) != 1 {
		return
	}
	if !isReceiveClause(statement.Body.List[0].(*ast.CommClause).Comm) {
		return
	}
	selectsInLoops[statement] = true
	c.addReport("s1000", loop.Pos(), "")
}

func unlabel(statement ast.Stmt) ast.Stmt {
	for {
		labeled, ok := statement.(*ast.LabeledStmt)
		if !ok {
			return statement
		}
		statement = labeled.Stmt
	}
}

func isReceiveClause(statement ast.Stmt) bool {
	switch statement := statement.(type) {
	case *ast.ExprStmt:
		return isReceive(statement.X)
	case *ast.AssignStmt:
		return len(statement.Rhs) == 1 && isReceive(statement.Rhs[0])
	}
	return false
}

func isReceive(expression ast.Expr) bool {
	unary, ok := ast.Unparen(expression).(*ast.UnaryExpr)
	return ok && unary.Op == token.ARROW
}

// ST1017: la constante a la izquierda de `==` o `!=` y lo que varía a la derecha.
func (c *collector) checkYodaCondition(expression *ast.BinaryExpr) {
	if expression.Op != token.EQL && expression.Op != token.NEQ {
		return
	}
	if c.trulyConstant(expression.X) && !c.trulyConstant(expression.Y) {
		c.addReport("st1017", expression.Pos(), "")
	}
}

// `TrulyConstantExpression`: una expresión constante sin identificadores (sólo literales).
func (c *collector) trulyConstant(expression ast.Expr) bool {
	expression = ast.Unparen(expression)
	if c.info.Types[expression].Value == nil {
		return false
	}
	truly := true
	ast.Inspect(expression, func(node ast.Node) bool {
		if _, ok := node.(*ast.Ident); ok {
			truly = false
		}
		return truly
	})
	return truly
}

// ST1006: un receptor que se llama `self`, `this` o `_`, salvo en los tipos genéricos.
func (c *collector) checkReceiverName(function *ast.FuncDecl) {
	if function.Recv == nil || len(function.Recv.List) != 1 || len(function.Recv.List[0].Names) != 1 {
		return
	}
	name := function.Recv.List[0].Names[0]
	if name.Name != "self" && name.Name != "this" && name.Name != "_" {
		return
	}
	receiver, ok := c.info.Defs[name].(*types.Var)
	if !ok {
		return
	}
	typ := receiver.Type()
	if pointer, ok := typ.(*types.Pointer); ok {
		typ = pointer.Elem()
	}
	named, ok := typ.(*types.Named)
	if !ok || named.TypeArgs().Len() > 0 || named.Obj().Parent() != c.pkg.Scope() {
		return
	}
	c.addReport("st1006", name.Pos(), name.Name)
}

// S1008: `if cond { return true }; return false` al final de un bloque, sin comentarios en medio.
func (c *collector) checkIfReturnBool(block *ast.BlockStmt, comments ast.CommentMap) {
	count := len(block.List)
	if count < 2 {
		return
	}
	if count >= 3 {
		if _, ok := block.List[count-3].(*ast.IfStmt); ok {
			return
		}
	}
	first, last := unlabel(block.List[count-2]), unlabel(block.List[count-1])
	statement, ok := first.(*ast.IfStmt)
	if !ok || statement.Init != nil || statement.Else != nil || len(statement.Body.List) != 1 {
		return
	}
	returned, ok := c.returnedBool(unlabel(statement.Body.List[0]))
	if !ok {
		return
	}
	fallback, ok := c.returnedBool(last)
	if !ok || returned == fallback {
		return
	}
	if comparison, ok := ast.Unparen(statement.Cond).(*ast.BinaryExpr); ok {
		switch comparison.Op {
		case token.EQL, token.LSS, token.GTR, token.NEQ, token.LEQ, token.GEQ:
		default:
			return
		}
	}
	if hasComments(comments, first) || hasComments(comments, last) {
		return
	}
	c.addReport("s1008", first.Pos(), "")
}

func (c *collector) returnedBool(statement ast.Stmt) (string, bool) {
	ret, ok := statement.(*ast.ReturnStmt)
	if !ok || len(ret.Results) != 1 {
		return "", false
	}
	ident, ok := ast.Unparen(ret.Results[0]).(*ast.Ident)
	if !ok || (ident.Name != "true" && ident.Name != "false") {
		return "", false
	}
	if c.info.ObjectOf(ident) != types.Universe.Lookup(ident.Name) {
		return "", false
	}
	return ident.Name, true
}

func hasComments(comments ast.CommentMap, node ast.Node) bool {
	for _, groups := range comments.Filter(node) {
		for _, group := range groups {
			for _, comment := range group.List {
				if !strings.HasPrefix(comment.Text, "//@ diag") {
					return true
				}
			}
		}
	}
	return false
}

// S1002: una comparación con la constante `true` o `false` (sin tipo propio) fuera de los tests.
func (c *collector) checkBoolComparison(expression *ast.BinaryExpr) {
	if expression.Op != token.EQL && expression.Op != token.NEQ {
		return
	}
	var other ast.Expr
	switch {
	case c.isBoolConst(expression.X):
		other = expression.Y
	case c.isBoolConst(expression.Y):
		other = expression.X
	default:
		return
	}
	if allTermsBool(c.info.TypeOf(other)) {
		c.addReport("s1002", expression.Pos(), "")
	}
}

func (c *collector) isBoolConst(expression ast.Expr) bool {
	ident, ok := expression.(*ast.Ident)
	if !ok {
		return false
	}
	value, ok := c.info.ObjectOf(ident).(*types.Const)
	if !ok {
		return false
	}
	basic, ok := value.Type().(*types.Basic)
	return ok && (basic.Kind() == types.UntypedBool || basic.Kind() == types.Bool)
}

// `typeutil.All` con un `bool` como término: un parámetro de tipo pide que todos sus términos lo sean.
func allTermsBool(typ types.Type) bool {
	if typ == nil {
		return false
	}
	parameter, ok := typ.(*types.TypeParam)
	if !ok {
		return isBool(typ)
	}
	constraint, ok := parameter.Constraint().Underlying().(*types.Interface)
	if !ok || constraint.NumEmbeddeds() != 1 {
		return false
	}
	union, ok := constraint.EmbeddedType(0).(*types.Union)
	if !ok {
		return isBool(constraint.EmbeddedType(0))
	}
	for i := 0; i < union.Len(); i++ {
		if !isBool(union.Term(i).Type()) {
			return false
		}
	}
	return union.Len() > 0
}

func isBool(typ types.Type) bool {
	basic, ok := typ.Underlying().(*types.Basic)
	return ok && basic.Kind() == types.Bool
}

var httpStatusCodes = map[int64]string{
	100: "StatusContinue",
	101: "StatusSwitchingProtocols",
	102: "StatusProcessing",
	200: "StatusOK",
	201: "StatusCreated",
	202: "StatusAccepted",
	203: "StatusNonAuthoritativeInfo",
	204: "StatusNoContent",
	205: "StatusResetContent",
	206: "StatusPartialContent",
	207: "StatusMultiStatus",
	208: "StatusAlreadyReported",
	226: "StatusIMUsed",
	300: "StatusMultipleChoices",
	301: "StatusMovedPermanently",
	302: "StatusFound",
	303: "StatusSeeOther",
	304: "StatusNotModified",
	305: "StatusUseProxy",
	307: "StatusTemporaryRedirect",
	308: "StatusPermanentRedirect",
	400: "StatusBadRequest",
	401: "StatusUnauthorized",
	402: "StatusPaymentRequired",
	403: "StatusForbidden",
	404: "StatusNotFound",
	405: "StatusMethodNotAllowed",
	406: "StatusNotAcceptable",
	407: "StatusProxyAuthRequired",
	408: "StatusRequestTimeout",
	409: "StatusConflict",
	410: "StatusGone",
	411: "StatusLengthRequired",
	412: "StatusPreconditionFailed",
	413: "StatusRequestEntityTooLarge",
	414: "StatusRequestURITooLong",
	415: "StatusUnsupportedMediaType",
	416: "StatusRequestedRangeNotSatisfiable",
	417: "StatusExpectationFailed",
	418: "StatusTeapot",
	422: "StatusUnprocessableEntity",
	423: "StatusLocked",
	424: "StatusFailedDependency",
	426: "StatusUpgradeRequired",
	428: "StatusPreconditionRequired",
	429: "StatusTooManyRequests",
	431: "StatusRequestHeaderFieldsTooLarge",
	451: "StatusUnavailableForLegalReasons",
	500: "StatusInternalServerError",
	501: "StatusNotImplemented",
	502: "StatusBadGateway",
	503: "StatusServiceUnavailable",
	504: "StatusGatewayTimeout",
	505: "StatusHTTPVersionNotSupported",
	506: "StatusVariantAlsoNegotiates",
	507: "StatusInsufficientStorage",
	508: "StatusLoopDetected",
	510: "StatusNotExtended",
	511: "StatusNetworkAuthenticationRequired",
}
