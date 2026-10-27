// Hechos de tipos de un módulo de Go para kerbo: compila con `go list -export` y chequea cada
// paquete con go/types, la biblioteca estándar. Escribe una línea JSON por archivo.
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"go/ast"
	"go/importer"
	"go/parser"
	"go/token"
	"go/types"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"sync/atomic"
)

// Si a algún paquete le faltó lo exportado de una dependencia, la salida sirve pero no se guarda.
const incompleteExit = 3

var incomplete atomic.Bool

type listedModule struct {
	GoVersion string
}

type listedPackage struct {
	Dir             string
	ImportPath      string
	Name            string
	ForTest         string
	Export          string
	CompiledGoFiles []string
	ImportMap       map[string]string
	DepOnly         bool
	Module          *listedModule
}

type callFact struct {
	Line         int      `json:"line"`
	Column       int      `json:"column"`
	Names        []string `json:"names"`
	Argument     string   `json:"argument"`
	ErrorsAsType bool     `json:"errors_as_type"`
	Function     string   `json:"function"`
	Selector     string   `json:"selector"`
}

type positionFact struct {
	Line   int `json:"line"`
	Column int `json:"column"`
}

type rangeFact struct {
	Line   int       `json:"line"`
	Column int       `json:"column"`
	Call   *callFact `json:"call"`
}

type fileFacts struct {
	File     string         `json:"file"`
	Calls    []callFact     `json:"calls"`
	Receives []positionFact `json:"receives"`
	Ranges   []rangeFact    `json:"ranges"`
}

var errorInterface = types.Universe.Lookup("error").Type().Underlying().(*types.Interface)

func main() {
	tags := flag.String("tags", "", "build tags")
	flag.Parse()
	packages, err := listPackages(flag.Arg(0), *tags)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	exports := map[string]string{}
	for _, listed := range packages {
		exports[listed.ImportPath] = listed.Export
	}
	targets := checkedPackages(packages)
	output := json.NewEncoder(os.Stdout)
	var writing sync.Mutex
	var group sync.WaitGroup
	slots := make(chan struct{}, runtime.GOMAXPROCS(0))
	for _, target := range targets {
		group.Add(1)
		slots <- struct{}{}
		go func(target listedPackage) {
			defer group.Done()
			defer func() { <-slots }()
			facts := checkPackage(target, exports)
			writing.Lock()
			defer writing.Unlock()
			for _, file := range facts {
				_ = output.Encode(file)
			}
		}(target)
	}
	group.Wait()
	if incomplete.Load() {
		os.Exit(incompleteExit)
	}
}

func listPackages(directory, tags string) ([]listedPackage, error) {
	arguments := []string{"list", "-e", "-export", "-deps", "-test", "-compiled",
		"-json=Dir,ImportPath,Name,ForTest,Export,CompiledGoFiles,ImportMap,DepOnly,Module"}
	if tags != "" {
		arguments = append(arguments, "-tags", tags)
	}
	command := exec.Command("go", append(arguments, "./...")...)
	command.Dir = directory
	command.Stderr = io.Discard
	stdout, err := command.StdoutPipe()
	if err != nil {
		return nil, err
	}
	if err := command.Start(); err != nil {
		return nil, err
	}
	var packages []listedPackage
	decoder := json.NewDecoder(stdout)
	for {
		var listed listedPackage
		if err := decoder.Decode(&listed); err == io.EOF {
			break
		} else if err != nil {
			return nil, err
		}
		packages = append(packages, listed)
	}
	return packages, command.Wait()
}

// Los paquetes del módulo con sus tests: si hay variante de test, reemplaza al paquete solo; el
// main generado de los tests no se mira.
func checkedPackages(packages []listedPackage) []listedPackage {
	withTests := map[string]bool{}
	for _, listed := range packages {
		if listed.ForTest != "" && listed.ForTest == strings.Fields(listed.ImportPath)[0] {
			withTests[listed.ForTest] = true
		}
	}
	var targets []listedPackage
	for _, listed := range packages {
		if listed.DepOnly || strings.HasSuffix(listed.ImportPath, ".test") {
			continue
		}
		if listed.ForTest == "" && withTests[listed.ImportPath] {
			continue
		}
		targets = append(targets, listed)
	}
	return targets
}

func checkPackage(listed listedPackage, exports map[string]string) []fileFacts {
	fset := token.NewFileSet()
	var files []*ast.File
	for _, name := range listed.CompiledGoFiles {
		if !filepath.IsAbs(name) {
			name = filepath.Join(listed.Dir, name)
		}
		file, err := parser.ParseFile(fset, name, nil, parser.SkipObjectResolution)
		if err == nil {
			files = append(files, file)
		}
	}
	lookup := func(path string) (io.ReadCloser, error) {
		if mapped, ok := listed.ImportMap[path]; ok {
			path = mapped
		}
		export := exports[path]
		if export == "" {
			incomplete.Store(true)
		}
		return os.Open(export)
	}
	config := types.Config{
		Importer: importer.ForCompiler(fset, "gc", lookup),
		Error:    func(error) {},
		Sizes:    types.SizesFor("gc", goarch()),
	}
	if listed.Module != nil && listed.Module.GoVersion != "" {
		config.GoVersion = "go" + listed.Module.GoVersion
	}
	info := &types.Info{
		Types:      map[ast.Expr]types.TypeAndValue{},
		Uses:       map[*ast.Ident]types.Object{},
		Defs:       map[*ast.Ident]types.Object{},
		Selections: map[*ast.SelectorExpr]*types.Selection{},
	}
	_, _ = config.Check(listed.ImportPath, fset, files, info)
	byFile := map[string]*fileFacts{}
	collector := &collector{info: info, fset: fset, directory: filepath.Clean(listed.Dir), byFile: byFile}
	for _, file := range files {
		ast.Inspect(file, collector.visit)
	}
	var facts []fileFacts
	for _, file := range byFile {
		facts = append(facts, *file)
	}
	return facts
}

func goarch() string {
	if arch := os.Getenv("GOARCH"); arch != "" {
		return arch
	}
	return runtime.GOARCH
}

type collector struct {
	info      *types.Info
	fset      *token.FileSet
	directory string
	byFile    map[string]*fileFacts
}

func (c *collector) file(pos token.Pos) (*fileFacts, token.Position, bool) {
	position := c.fset.PositionFor(pos, true)
	if filepath.Dir(position.Filename) != c.directory {
		return nil, position, false
	}
	facts, ok := c.byFile[position.Filename]
	if !ok {
		facts = &fileFacts{File: position.Filename}
		c.byFile[position.Filename] = facts
	}
	return facts, position, true
}

func (c *collector) visit(node ast.Node) bool {
	switch expression := node.(type) {
	case *ast.CallExpr:
		if c.callReturnsError(expression) {
			if facts, position, ok := c.file(expression.Lparen); ok {
				fact := c.callFact(expression)
				fact.Line, fact.Column = position.Line, position.Column
				facts.Calls = append(facts.Calls, fact)
			}
		}
	case *ast.UnaryExpr:
		if expression.Op == token.ARROW && c.receiveReturnsError(expression) {
			if facts, position, ok := c.file(expression.OpPos); ok {
				facts.Receives = append(facts.Receives, positionFact{position.Line, position.Column})
			}
		}
	case *ast.RangeStmt:
		if c.rangeReportsError(expression) {
			if facts, position, ok := c.file(expression.Pos()); ok {
				fact := rangeFact{Line: position.Line, Column: position.Column}
				if call := unwrapCall(expression.X); call != nil {
					called := c.callFact(call)
					fact.Call = &called
				}
				facts.Ranges = append(facts.Ranges, fact)
			}
		}
	}
	return true
}

func (c *collector) callFact(call *ast.CallExpr) callFact {
	fact := callFact{Names: c.namesForExcludeCheck(call), ErrorsAsType: c.isErrorsAsType(call)}
	if len(call.Args) > 0 {
		fact.Argument = c.argName(call.Args[0])
	}
	if selector, function, ok := c.selectorAndFunc(call); ok {
		fact.Function = function.FullName()
		fact.Selector = selectorName(selector)
	}
	return fact
}

func (c *collector) selectorAndFunc(call *ast.CallExpr) (*ast.SelectorExpr, *types.Func, bool) {
	selector, ok := baseCallExpr(call.Fun).(*ast.SelectorExpr)
	if !ok {
		return nil, nil, false
	}
	function, ok := c.info.ObjectOf(selector.Sel).(*types.Func)
	return selector, function, ok
}

func selectorName(selector *ast.SelectorExpr) string {
	if ident, ok := selector.X.(*ast.Ident); ok {
		return ident.Name + "." + selector.Sel.Name
	}
	if inner, ok := selector.X.(*ast.SelectorExpr); ok {
		return selectorName(inner) + "." + selector.Sel.Name
	}
	return ""
}

func (c *collector) namesForExcludeCheck(call *ast.CallExpr) []string {
	selector, function, ok := c.selectorAndFunc(call)
	if !ok {
		return nil
	}
	name := function.FullName()
	selection, ok := c.info.Selections[selector]
	if !ok {
		return []string{name}
	}
	embedded, ok := walkThroughEmbeddedInterfaces(selection)
	if !ok {
		return []string{name}
	}
	names := make([]string, len(embedded))
	for i, typ := range embedded {
		names[i] = fmt.Sprintf("(%s).%s", typ.String(), function.Name())
	}
	return names
}

func (c *collector) argName(expression ast.Expr) string {
	if selector, ok := expression.(*ast.SelectorExpr); ok {
		if variable, ok := c.info.ObjectOf(selector.Sel).(*types.Var); ok && variable.Pkg() != nil &&
			variable.Pkg().Name() == "os" && (variable.Name() == "Stderr" || variable.Name() == "Stdout") {
			return "os." + variable.Name()
		}
	}
	if typ := c.info.TypeOf(expression); typ != nil {
		return typ.String()
	}
	return ""
}

func (c *collector) isErrorsAsType(call *ast.CallExpr) bool {
	selector, ok := baseCallExpr(call.Fun).(*ast.SelectorExpr)
	if !ok || selector.Sel.Name != "AsType" {
		return false
	}
	packageIdent, ok := selector.X.(*ast.Ident)
	if !ok {
		return false
	}
	if function, ok := c.info.ObjectOf(selector.Sel).(*types.Func); ok && function.Pkg() != nil &&
		nonVendoredPath(function.Pkg().Path()) == "errors" && function.Name() == "AsType" {
		return true
	}
	if name, ok := c.info.Uses[packageIdent].(*types.PkgName); ok && name.Imported() != nil &&
		nonVendoredPath(name.Imported().Path()) == "errors" {
		return true
	}
	return packageIdent.Name == "errors"
}

func nonVendoredPath(path string) string {
	if index := strings.LastIndex(path, "/vendor/"); index != -1 {
		return path[index+len("/vendor/"):]
	}
	return path
}

func (c *collector) errorsByResult(call *ast.CallExpr) []bool {
	if c.info.Types[call.Fun].IsType() {
		return []bool{false}
	}
	switch typ := c.info.Types[call].Type.(type) {
	case *types.Named, *types.Pointer:
		return []bool{isErrorType(typ)}
	case *types.Tuple:
		results := make([]bool, typ.Len())
		for i := 0; i < typ.Len(); i++ {
			switch element := typ.At(i).Type().(type) {
			case *types.Named, *types.Pointer:
				results[i] = isErrorType(element)
			}
		}
		return results
	}
	return []bool{false}
}

func (c *collector) callReturnsError(call *ast.CallExpr) bool {
	if c.isRecover(call) {
		return true
	}
	for _, isError := range c.errorsByResult(call) {
		if isError {
			return true
		}
	}
	return false
}

func (c *collector) isRecover(call *ast.CallExpr) bool {
	if function, ok := call.Fun.(*ast.Ident); ok {
		if _, ok := c.info.Uses[function].(*types.Builtin); ok {
			return function.Name == "recover"
		}
	}
	return false
}

func (c *collector) receiveReturnsError(unary *ast.UnaryExpr) bool {
	typ := c.info.TypeOf(unwrapExpr(unary.X))
	if typ == nil {
		if typ = c.info.TypeOf(unary.X); typ == nil {
			return false
		}
	}
	channel, ok := types.Unalias(typ).Underlying().(*types.Chan)
	return ok && isErrorType(channel.Elem())
}

// `checkRangeStmt` de errcheck sin las exclusiones, que aplica kerbo con el nombre de la llamada.
func (c *collector) rangeReportsError(statement *ast.RangeStmt) bool {
	typ := c.info.TypeOf(statement.X)
	if typ == nil {
		return false
	}
	signature, ok := types.Unalias(typ).Underlying().(*types.Signature)
	if !ok || signature.Params().Len() != 1 || signature.Results().Len() != 0 {
		return false
	}
	yield, ok := types.Unalias(signature.Params().At(0).Type()).Underlying().(*types.Signature)
	if !ok || yield.Results().Len() != 1 {
		return false
	}
	result, ok := yield.Results().At(0).Type().Underlying().(*types.Basic)
	if !ok || result.Info()&types.IsBoolean == 0 {
		return false
	}
	count := yield.Params().Len()
	if count == 0 || count > 2 {
		return false
	}
	for i := 0; i < count; i++ {
		if !isErrorType(yield.Params().At(i).Type()) {
			continue
		}
		bound := statement.Key
		if i == 1 {
			bound = statement.Value
		}
		return bound == nil
	}
	return false
}

func isErrorType(typ types.Type) bool {
	return typ != nil && types.Implements(types.Unalias(typ), errorInterface)
}

func baseCallExpr(function ast.Expr) ast.Expr {
	for {
		switch inner := function.(type) {
		case *ast.IndexExpr:
			function = inner.X
		case *ast.IndexListExpr:
			function = inner.X
		case *ast.ParenExpr:
			function = inner.X
		default:
			return function
		}
	}
}

func unwrapExpr(expression ast.Expr) ast.Expr {
	for {
		inner, ok := expression.(*ast.ParenExpr)
		if !ok {
			return expression
		}
		expression = inner.X
	}
}

func unwrapCall(expression ast.Expr) *ast.CallExpr {
	call, _ := unwrapExpr(expression).(*ast.CallExpr)
	return call
}

func walkThroughEmbeddedInterfaces(selection *types.Selection) ([]types.Type, bool) {
	function, ok := selection.Obj().(*types.Func)
	if !ok {
		return nil, false
	}
	current := selection.Recv()
	indexes := selection.Index()
	for _, field := range indexes[:len(indexes)-1] {
		current = typeAtFieldIndex(current, field)
	}
	iface, ok := unname(current).(*types.Interface)
	if !ok {
		return nil, false
	}
	result := []types.Type{current}
	for !explicitlyDefines(iface, function) {
		embedded, ok := embeddedDefining(iface, function)
		if !ok {
			break
		}
		result = append(result, embedded)
		iface = embedded.Underlying().(*types.Interface)
	}
	return result, true
}

func typeAtFieldIndex(start types.Type, field int) types.Type {
	typ := types.Unalias(start)
	if pointer, ok := typ.(*types.Pointer); ok {
		typ = pointer.Elem()
	}
	structure, ok := unname(types.Unalias(typ)).(*types.Struct)
	if !ok {
		return typ
	}
	return structure.Field(field).Type()
}

func embeddedDefining(iface *types.Interface, function *types.Func) (*types.Named, bool) {
	for i := 0; i < iface.NumEmbeddeds(); i++ {
		embedded := iface.Embedded(i)
		if embedded != nil && defines(embedded.Underlying().(*types.Interface), function) {
			return embedded, true
		}
	}
	return nil, false
}

func explicitlyDefines(iface *types.Interface, function *types.Func) bool {
	for i := 0; i < iface.NumExplicitMethods(); i++ {
		if iface.ExplicitMethod(i) == function {
			return true
		}
	}
	return false
}

func defines(iface *types.Interface, function *types.Func) bool {
	for i := 0; i < iface.NumMethods(); i++ {
		if iface.Method(i) == function {
			return true
		}
	}
	return false
}

func unname(typ types.Type) types.Type {
	if named, ok := typ.(*types.Named); ok {
		return named.Underlying()
	}
	return typ
}
