// QF1001 de staticcheck: `!(a == b && c)` se puede escribir sin la negación de afuera, salvo que
// algo de adentro sea un float o que staticcheck no pueda copiar la expresión.
package main

import (
	"go/ast"
	"go/token"
	"go/types"
)

const deMorganRule = "qf1001"

func (c *collector) checkDeMorgan(negation *ast.UnaryExpr) {
	if negation.Op != token.NOT {
		return
	}
	expression, ok := ast.Unparen(negation.X).(*ast.BinaryExpr)
	if !ok || c.hasFloats(expression) || !copiable(expression) {
		return
	}
	facts, position, ok := c.file(negation.Pos())
	if !ok {
		return
	}
	facts.Reports = append(facts.Reports, report{
		Rule:    deMorganRule,
		Line:    position.Line,
		Column:  position.Column,
		Subject: types.ExprString(negation),
	})
}

func (c *collector) hasFloats(expression ast.Expr) bool {
	found := false
	ast.Inspect(expression, func(node ast.Node) bool {
		inner, ok := node.(ast.Expr)
		if !ok {
			return true
		}
		if typ := c.info.TypeOf(inner); typ != nil {
			if basic, ok := typ.Underlying().(*types.Basic); ok && basic.Info()&types.IsFloat != 0 {
				found = true
				return false
			}
		}
		return true
	})
	return found
}

// `astutil.CopyExpr` no copia funciones literales ni tipos de función, salvo dentro de un tipo
// struct o interfaz, que copia sin mirar adentro.
func copiable(expression ast.Expr) bool {
	ok := true
	ast.Inspect(expression, func(node ast.Node) bool {
		switch node.(type) {
		case *ast.FuncLit, *ast.FuncType:
			ok = false
		case *ast.StructType, *ast.InterfaceType:
			return false
		}
		return ok
	})
	return ok
}
