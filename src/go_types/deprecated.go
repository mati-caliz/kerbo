// SA1019 de staticcheck: el uso de lo que el paquete que lo declara marcó con `Deprecated:`. Lo
// exportado no trae los comentarios, así que se leen del fuente de cada paquete listado.
package main

import (
	"bytes"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"go/types"
	"go/version"
	"os"
	"path/filepath"
	"strings"
)

const deprecatedPrefix = "Deprecated: "

type deprecations struct {
	objects  map[string]string
	packages map[string]string
}

func objectKey(path, name string) string {
	return path + "\x00" + name
}

// Las deprecaciones de todos los paquetes listados, por paquete y nombre (`Tipo.Metodo`,
// `Tipo.Campo`). Sólo se parsean los archivos que nombran `Deprecated: `.
func indexDeprecations(packages []listedPackage) deprecations {
	index := deprecations{objects: map[string]string{}, packages: map[string]string{}}
	seen := map[string]bool{}
	for _, listed := range packages {
		path := strings.Fields(listed.ImportPath)[0]
		names := append(append(append([]string{}, listed.GoFiles...), listed.TestGoFiles...), listed.XTestGoFiles...)
		for _, name := range names {
			file := filepath.Join(listed.Dir, name)
			if seen[file] {
				continue
			}
			seen[file] = true
			content, err := os.ReadFile(file)
			if err != nil || !bytes.Contains(content, []byte(deprecatedPrefix)) {
				continue
			}
			parsed, err := parser.ParseFile(token.NewFileSet(), file, content, parser.ParseComments|parser.SkipObjectResolution)
			if err != nil {
				continue
			}
			filePath := path
			if strings.HasSuffix(parsed.Name.Name, "_test") && !strings.HasSuffix(path, "_test") {
				filePath = path + "_test"
			}
			index.collect(filePath, parsed)
		}
	}
	return index
}

func deprecatedMessage(docs []*ast.CommentGroup) string {
	for _, doc := range docs {
		if doc == nil {
			continue
		}
		for _, part := range strings.Split(doc.Text(), "\n\n") {
			if strings.HasPrefix(part, deprecatedPrefix) {
				return strings.ReplaceAll(part[len(deprecatedPrefix):], "\n", " ")
			}
		}
	}
	return ""
}

// El recorrido del analizador `deprecated` de staticcheck sobre un archivo.
func (index deprecations) collect(path string, file *ast.File) {
	if message := deprecatedMessage([]*ast.CommentGroup{file.Doc}); message != "" && path != "syscall" {
		if _, ok := index.packages[path]; !ok {
			index.packages[path] = message
		}
	}
	mark := func(names []string, docs []*ast.CommentGroup) {
		message := deprecatedMessage(docs)
		if message == "" {
			return
		}
		for _, name := range names {
			index.objects[objectKey(path, name)] = message
		}
	}
	for _, declaration := range file.Decls {
		switch declaration := declaration.(type) {
		case *ast.FuncDecl:
			name := declaration.Name.Name
			if declaration.Recv != nil && len(declaration.Recv.List) > 0 {
				name = receiverName(declaration.Recv.List[0].Type) + "." + name
			}
			mark([]string{name}, []*ast.CommentGroup{declaration.Doc})
		case *ast.GenDecl:
			if declaration.Tok != token.TYPE && declaration.Tok != token.CONST && declaration.Tok != token.VAR {
				continue
			}
			var names []string
			for _, spec := range declaration.Specs {
				switch spec := spec.(type) {
				case *ast.ValueSpec:
					names = append(names, identNames(spec.Names)...)
				case *ast.TypeSpec:
					names = append(names, spec.Name.Name)
				}
			}
			mark(names, []*ast.CommentGroup{declaration.Doc})
			for _, spec := range declaration.Specs {
				switch spec := spec.(type) {
				case *ast.ValueSpec:
					mark(identNames(spec.Names), []*ast.CommentGroup{spec.Doc})
				case *ast.TypeSpec:
					mark([]string{spec.Name.Name}, []*ast.CommentGroup{spec.Doc})
					index.collectMembers(path, spec)
				}
			}
		}
	}
}

func (index deprecations) collectMembers(path string, spec *ast.TypeSpec) {
	var fields *ast.FieldList
	switch typ := spec.Type.(type) {
	case *ast.StructType:
		fields = typ.Fields
	case *ast.InterfaceType:
		fields = typ.Methods
	default:
		return
	}
	for _, field := range fields.List {
		message := deprecatedMessage([]*ast.CommentGroup{field.Doc})
		if message == "" {
			continue
		}
		for _, name := range field.Names {
			index.objects[objectKey(path, spec.Name.Name+"."+name.Name)] = message
		}
	}
}

func identNames(idents []*ast.Ident) []string {
	names := make([]string, len(idents))
	for i, ident := range idents {
		names[i] = ident.Name
	}
	return names
}

func receiverName(expression ast.Expr) string {
	for {
		switch inner := expression.(type) {
		case *ast.StarExpr:
			expression = inner.X
		case *ast.ParenExpr:
			expression = inner.X
		case *ast.IndexExpr:
			expression = inner.X
		case *ast.IndexListExpr:
			expression = inner.X
		case *ast.Ident:
			return inner.Name
		default:
			return ""
		}
	}
}

// El nombre con que el índice guarda un objeto usado: el método con su tipo, el campo con el
// struct o la interfaz con nombre que lo declara.
func usedObjectName(object types.Object) string {
	switch object := object.(type) {
	case *types.Func:
		signature, ok := object.Type().(*types.Signature)
		if !ok || signature.Recv() == nil {
			return object.Name()
		}
		receiver := types.Unalias(signature.Recv().Type())
		if pointer, ok := receiver.(*types.Pointer); ok {
			receiver = types.Unalias(pointer.Elem())
		}
		if named, ok := receiver.(*types.Named); ok {
			return named.Origin().Obj().Name() + "." + object.Name()
		}
		return ownerMember(object)
	case *types.Var:
		if object.IsField() {
			return ownerMember(object.Origin())
		}
	}
	return object.Name()
}

// El tipo con nombre del paquete que declara el campo o el método de interfaz.
func ownerMember(member types.Object) string {
	scope := member.Pkg().Scope()
	for _, name := range scope.Names() {
		typeName, ok := scope.Lookup(name).(*types.TypeName)
		if !ok {
			continue
		}
		switch underlying := typeName.Type().Underlying().(type) {
		case *types.Struct:
			for i := 0; i < underlying.NumFields(); i++ {
				if underlying.Field(i) == member {
					return name + "." + member.Name()
				}
			}
		case *types.Interface:
			for i := 0; i < underlying.NumExplicitMethods(); i++ {
				if underlying.ExplicitMethod(i) == member {
					return name + "." + member.Name()
				}
			}
		}
	}
	return member.Name()
}

type report struct {
	Rule    string `json:"rule"`
	Line    int    `json:"line"`
	Column  int    `json:"column"`
	Subject string `json:"subject"`
}

const deprecatedRule = "sa1019"

// El recorrido de SA1019 sobre un archivo del paquete: `tfn` es la última función declarada, que
// puede usar lo deprecado si ella misma lo está.
func (c *collector) collectDeprecatedUses(file *ast.File, index deprecations) {
	var function types.Object
	ast.Inspect(file, func(node ast.Node) bool {
		switch node := node.(type) {
		case *ast.FuncDecl:
			function = c.info.ObjectOf(node.Name)
		case *ast.SelectorExpr:
			c.checkSelector(file, node, function, index)
		case *ast.CompositeLit:
			c.checkCompositeKeys(file, node, function, index)
		case *ast.ImportSpec:
			c.checkImport(file, node, index)
		}
		return true
	})
}

func (c *collector) checkCompositeKeys(file *ast.File, literal *ast.CompositeLit, function types.Object, index deprecations) {
	literalType := c.info.Types[literal.Type]
	if !literalType.IsType() {
		return
	}
	if _, ok := literalType.Type.Underlying().(*types.Struct); !ok {
		return
	}
	for _, element := range literal.Elts {
		pair, ok := element.(*ast.KeyValueExpr)
		if !ok {
			return
		}
		key, ok := pair.Key.(*ast.Ident)
		if !ok {
			return
		}
		c.checkSelector(file, &ast.SelectorExpr{X: literal.Type, Sel: key}, function, index)
	}
}

func (c *collector) checkSelector(file *ast.File, selector *ast.SelectorExpr, function types.Object, index deprecations) {
	object := c.info.ObjectOf(selector.Sel)
	if called, ok := object.(*types.Func); ok {
		object = called.Origin()
	}
	if object == nil || object.Pkg() == nil || object.Pkg() == c.pkg || exemptPackage(c.pkg.Path(), object.Pkg().Path()) {
		return
	}
	if _, ok := index.objects[objectKey(object.Pkg().Path(), usedObjectName(object))]; !ok {
		return
	}
	var node ast.Node = selector
	if c.info.Types[selector.X].IsType() {
		node = selector.Sel
	}
	name, ok := c.selectorName(selector)
	if !ok {
		return
	}
	c.reportDeprecation(file, node, name, object.Pkg().Path(), function, index)
}

func (c *collector) checkImport(file *ast.File, spec *ast.ImportSpec, index deprecations) {
	path := strings.Trim(spec.Path.Value, "\"`")
	if _, ok := index.packages[path]; !ok || exemptPackage(c.pkg.Path(), path) {
		return
	}
	c.reportDeprecation(file, spec.Path, path, path, nil, index)
}

// Un paquete puede usar lo propio, y sus tests externos y el main de los tests, lo de él.
func exemptPackage(current, used string) bool {
	return strings.TrimSuffix(current, "_test") == used ||
		strings.TrimSuffix(current, ".test") == used ||
		strings.TrimSuffix(current, ".test") == strings.TrimSuffix(used, "_test")
}

// `handleDeprecation`: lo de la biblioteca estándar sólo si está en la tabla y el código apunta a
// una versión en la que ya estaba deprecado.
func (c *collector) reportDeprecation(file *ast.File, node ast.Node, name, path string, function types.Object, index deprecations) {
	known, inTable := stdlibDeprecations[name]
	if !inTable && !strings.Contains(path, ".") {
		return
	}
	if inTable && version.Compare(c.stdlibVersion(file), known.since) == -1 {
		return
	}
	if function != nil && function.Pkg() != nil {
		if _, ok := index.objects[objectKey(function.Pkg().Path(), usedObjectName(function))]; ok {
			return
		}
	}
	facts, position, ok := c.file(node.Pos())
	if !ok {
		return
	}
	facts.Reports = append(facts.Reports, report{Rule: deprecatedRule, Line: position.Line, Column: position.Column, Subject: name})
}

// `StdlibVersion`: la versión de Go del módulo, o la del archivo si la restricción de build pide otra.
func (c *collector) stdlibVersion(file *ast.File) string {
	module := c.pkg.GoVersion()
	fileVersion := file.GoVersion
	if fileVersion == "" {
		return module
	}
	if version.Compare(module, "go1.21") == -1 || version.Compare(fileVersion, module) == 1 {
		return fileVersion
	}
	return module
}

// `code.SelectorName`: cómo staticcheck nombra lo seleccionado.
func (c *collector) selectorName(selector *ast.SelectorExpr) (string, bool) {
	selection := c.info.Selections[selector]
	if selection == nil {
		switch operand := selector.X.(type) {
		case *ast.Ident:
			if name, ok := c.info.ObjectOf(operand).(*types.PkgName); ok {
				return name.Imported().Path() + "." + selector.Sel.Name, true
			}
			return fmt.Sprintf("(%s).%s", c.info.TypeOf(operand), selector.Sel.Name), true
		case *ast.SelectorExpr:
			inner, ok := c.selectorName(operand)
			return fmt.Sprintf("(%s).%s", inner, selector.Sel.Name), ok
		case *ast.IndexExpr, *ast.IndexListExpr:
			return fmt.Sprintf("(%s).%s", c.info.TypeOf(operand), selector.Sel.Name), true
		default:
			return "", false
		}
	}
	if variable, ok := selection.Obj().(*types.Var); ok && variable.IsField() {
		receiver := selection.Recv()
		if pointer, ok := receiver.(*types.Pointer); ok {
			receiver = pointer.Elem()
		}
		return fmt.Sprintf("(%s).%s", receiver, selection.Obj().Name()), true
	}
	return fmt.Sprintf("(%s).%s", selection.Recv(), selection.Obj().Name()), true
}
