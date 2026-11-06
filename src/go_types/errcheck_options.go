package main

import (
	"go/ast"
	"go/token"
)

const blankIdentifier = "_"

// Una aserción de tipo sin mirar el `ok`. Con `blank`, sólo cuenta si además se pidió `check-blank`.
type assertionFact struct {
	Line   int  `json:"line"`
	Column int  `json:"column"`
	Blank  bool `json:"blank"`
}

// `visitor.Visit` de errcheck en lo que sólo mira con `check-blank` y `check-type-assertions`. Se
// junta siempre: kerbo elige qué usar según su configuración, sin invalidar los hechos en caché.
func (c *collector) visitErrcheckOptions(node ast.Node) bool {
	switch statement := node.(type) {
	case *ast.GenDecl:
		return c.visitVarDeclaration(statement)
	case *ast.AssignStmt:
		return c.checkAssignment(statement.Lhs, statement.Rhs)
	case *ast.TypeAssertExpr:
		if statement.Type != nil {
			c.addAssertion(statement.Pos(), false)
		}
		return false
	}
	return true
}

func (c *collector) visitVarDeclaration(declaration *ast.GenDecl) bool {
	if declaration.Tok != token.VAR {
		return true
	}
	for _, spec := range declaration.Specs {
		value, ok := spec.(*ast.ValueSpec)
		if !ok || len(value.Values) == 0 {
			continue
		}
		names := make([]ast.Expr, 0, len(value.Names))
		for _, name := range value.Names {
			names = append(names, name)
		}
		if !c.checkAssignment(names, value.Values) {
			return false
		}
	}
	return true
}

// `checkAssignment` de errcheck: el resultado dice si hay que entrar en la asignación.
func (c *collector) checkAssignment(lhs, rhs []ast.Expr) bool {
	if len(rhs) == 1 {
		return c.checkSingleValue(lhs, rhs[0])
	}
	for index, target := range lhs {
		identifier, ok := target.(*ast.Ident)
		if !ok || index >= len(rhs) {
			continue
		}
		c.checkPairedValue(identifier, rhs[index])
	}
	return true
}

func (c *collector) checkSingleValue(lhs []ast.Expr, value ast.Expr) bool {
	if call, ok := value.(*ast.CallExpr); ok {
		c.checkCallResults(lhs, call)
		return true
	}
	if assertion, ok := value.(*ast.TypeAssertExpr); ok {
		c.checkAssertionAssignment(lhs, assertion)
		return false
	}
	if unary := unwrapUnary(value); unary != nil && unary.Op == token.ARROW && len(lhs) >= 1 {
		if identifier, ok := lhs[0].(*ast.Ident); ok && identifier.Name == blankIdentifier && c.receiveReturnsError(unary) {
			c.addBlank(identifier.NamePos, nil)
		}
	}
	return true
}

func (c *collector) checkCallResults(lhs []ast.Expr, call *ast.CallExpr) {
	isError := c.errorsByResult(call)
	for index, target := range lhs {
		identifier, ok := target.(*ast.Ident)
		if !ok || identifier.Name != blankIdentifier {
			continue
		}
		if c.isRecover(call) || (index < len(isError) && isError[index]) {
			c.addBlank(identifier.NamePos, call)
		}
	}
}

func (c *collector) checkAssertionAssignment(lhs []ast.Expr, assertion *ast.TypeAssertExpr) {
	if assertion.Type == nil {
		return
	}
	if len(lhs) < 2 {
		c.addAssertion(assertion.Pos(), false)
		return
	}
	if identifier, ok := lhs[1].(*ast.Ident); ok && identifier.Name == blankIdentifier {
		c.addAssertion(identifier.NamePos, true)
	}
}

func (c *collector) checkPairedValue(identifier *ast.Ident, value ast.Expr) {
	if call, ok := value.(*ast.CallExpr); ok {
		if identifier.Name == blankIdentifier && c.callReturnsError(call) {
			c.addBlank(identifier.NamePos, call)
		}
		return
	}
	if assertion, ok := value.(*ast.TypeAssertExpr); ok {
		if assertion.Type != nil {
			c.addAssertion(identifier.NamePos, false)
		}
		return
	}
	unary := unwrapUnary(value)
	if unary != nil && unary.Op == token.ARROW && identifier.Name == blankIdentifier && c.receiveReturnsError(unary) {
		c.addBlank(identifier.NamePos, nil)
	}
}

func unwrapUnary(expression ast.Expr) *ast.UnaryExpr {
	unary, _ := unwrapExpr(expression).(*ast.UnaryExpr)
	return unary
}

func (c *collector) addBlank(pos token.Pos, call *ast.CallExpr) {
	facts, position, ok := c.file(pos)
	if !ok {
		return
	}
	fact := rangeFact{Line: position.Line, Column: position.Column}
	if call != nil {
		called := c.callFact(call)
		fact.Call = &called
	}
	facts.Blanks = append(facts.Blanks, fact)
}

func (c *collector) addAssertion(pos token.Pos, blank bool) {
	if facts, position, ok := c.file(pos); ok {
		facts.Assertions = append(facts.Assertions, assertionFact{position.Line, position.Column, blank})
	}
}
