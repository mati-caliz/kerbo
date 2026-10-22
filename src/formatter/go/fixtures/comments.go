//go:build linux&&(amd64||arm64)

/*
Package comments junta los casos de comentarios.
*/
	package comments

/* comentario de bloque
 * con estrellas
 */
	var 	 a=  1 	 /* al lado */

var(
	b 	 =	2 // dos
	ccc  =3 	 // tres
	// suelto
	d = 4
 	)

type T struct	{
	// Primero tiene documentación.
	First int // y comentario
 	Second  string /* bloque */
    Third bool
}

func f( /* nada */	) { 
	// adentro
	x:= 	 1 // al final
	y := 	 2 	 // también
  _=	x + /* medio */ y
	/*
	   bloque indentado
	*/
 	if x> 0 { // después de la llave
    _=y
	}
	// antes de cerrar
} 

  func g(a 	 int, // primero
 b int, // segundo
){
	switch a {
	// antes del case
	case  1:
		// dentro del case
	} 	 
  _= []int{
			1, // uno
	2,
		// suelto
	}
    }

//line generated.go:10
func	h()  {}

// Última.
