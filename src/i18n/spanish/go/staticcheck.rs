use crate::i18n::catalog::RuleText;
use crate::rules::GoRule;

// Las reglas de staticcheck que decide el helper de tipos.
pub const STATICCHECK_TEXTS: [(GoRule, RuleText); 41] = [
    (
        GoRule::S1009,
        RuleText::new(
            "Chequeo de nil antes de len() que len() ya cubre (S1009 de staticcheck)",
            "sacá el chequeo de nil y dejá sólo la comparación con len()",
            "len() de un slice, mapa o canal nil da cero, así que el chequeo de nil repite el del largo.",
            "if s == nil || len(s) == 0 se vuelve if len(s) == 0.",
        ),
    ),
    (
        GoRule::S1011,
        RuleText::new(
            "Loop que agrega de a uno lo que un solo append puede agregar (S1011 de staticcheck)",
            "reemplazá el loop por append(dst, src...)",
            "append con ... agrega el slice entero de una: es más corto y agranda el destino una sola vez.",
            "for _, v := range src { dst = append(dst, v) } se vuelve dst = append(dst, src...).",
        ),
    ),
    (
        GoRule::S1017,
        RuleText::new(
            "if que mira un prefijo o sufijo antes de sacarlo (S1017 de staticcheck)",
            "llamá a strings.TrimPrefix, TrimSuffix o Replace sin el if",
            "Las funciones Trim ya no hacen nada si el prefijo o sufijo no está, así que el if chequea dos veces.",
            "if strings.HasPrefix(s, p) { s = s[len(p):] } se vuelve s = strings.TrimPrefix(s, p).",
        ),
    ),
    (
        GoRule::S1029,
        RuleText::new(
            "range sobre []rune(s) sin usar el índice (S1029 de staticcheck)",
            "recorré el string directamente",
            "Recorrer un string ya da sus runas, sin reservar un slice tan largo como el string.",
            "for _, r := range []rune(s) se vuelve for _, r := range s.",
        ),
    ),
    (
        GoRule::S1030,
        RuleText::new(
            "Conversión del contenido de un bytes.Buffer que el buffer ya ofrece (S1030 de staticcheck)",
            "usá buf.String() o buf.Bytes() directamente",
            "string(buf.Bytes()) es buf.String(), y []byte(buf.String()) copia dos veces lo que da buf.Bytes().",
            "string(buf.Bytes()) se vuelve buf.String().",
        ),
    ),
    (
        GoRule::S1032,
        RuleText::new(
            "sort.Sort(sort.IntSlice(x)) y sus pares (S1032 de staticcheck)",
            "usá sort.Ints, sort.Float64s o sort.Strings",
            "El paquete sort ya tiene una función para cada uno de estos slices; la conversión sólo agrega ruido.",
            "sort.Sort(sort.StringSlice(nombres)) se vuelve sort.Strings(nombres).",
        ),
    ),
    (
        GoRule::S1038,
        RuleText::new(
            "fmt.Sprintf pasado a una función de imprimir que tiene su versión con formato (S1038 de staticcheck)",
            "llamá a la variante con f (Printf, Errorf, Fatalf...) con el formato directo",
            "Formatear a un string y después imprimirlo hace en dos pasos lo que la variante con f hace en uno.",
            "t.Error(fmt.Sprintf(\"vino %d\", n)) se vuelve t.Errorf(\"vino %d\", n).",
        ),
    ),
    (
        GoRule::S1039,
        RuleText::new(
            "fmt.Sprint o fmt.Sprintf de un solo string literal (S1039 de staticcheck)",
            "usá el string literal directamente",
            "Formatear un string constante sin verbos devuelve el mismo string.",
            "fmt.Sprintf(\"listo\") se vuelve \"listo\".",
        ),
    ),
    (
        GoRule::S1040,
        RuleText::new(
            "Aserción de tipo a la interfaz que el valor ya tiene (S1040 de staticcheck)",
            "sacá la aserción",
            "El valor ya es de esa interfaz: la aserción siempre sale bien y no cambia nada.",
            "var r io.Reader = f; r.(io.Reader).Read(p) se vuelve r.Read(p).",
        ),
    ),
    (
        GoRule::Sa1006,
        RuleText::new(
            "Llamada estilo printf con un formato dinámico y sin otros argumentos (SA1006 de staticcheck)",
            "usá la función sin formato, o pasá el valor como argumento de un formato constante",
            "Si el texto tiene un %, se lee como verbo y la salida sale rota.",
            "fmt.Printf(mensaje) se vuelve fmt.Print(mensaje), y fmt.Errorf(mensaje) se vuelve errors.New(mensaje).",
        ),
    ),
    (
        GoRule::Sa2001,
        RuleText::new(
            "Sección crítica vacía: Lock seguido de Unlock (SA2001 de staticcheck)",
            "poné entre las dos llamadas el código que necesita el lock, o sacalas",
            "Bloquear y desbloquear enseguida no protege nada; casi siempre se perdió el código del medio o el Unlock iba con defer.",
            "mu.Lock(); mu.Unlock() se vuelve mu.Lock(); defer mu.Unlock() antes del código protegido.",
        ),
    ),
    (
        GoRule::Sa4004,
        RuleText::new(
            "Loop que siempre termina en la primera vuelta (SA4004 de staticcheck)",
            "usá un if, o poné el return o el break dentro de una condición",
            "Un return o break sin condición al final del cuerpo hace que el loop no repita nunca, y casi nunca es lo que se quería.",
            "for _, x := range xs { if ok(x) { usar(x) }; return } corre sólo para xs[0].",
        ),
    ),
    (
        GoRule::Sa4011,
        RuleText::new(
            "break al final de un case dentro de un loop, que sólo sale del switch (SA4011 de staticcheck)",
            "usá un break con etiqueta para salir del loop, o sacá el break",
            "En Go un case ya termina solo; un break suelto ahí sale del switch o del select, no del loop que lo rodea.",
            "for { switch x { case 1: break } } sigue dando vueltas; usá break afuera con una etiqueta en el for.",
        ),
    ),
    (
        GoRule::Sa4016,
        RuleText::new(
            "Operación de bits con cero que no hace nada (SA4016 de staticcheck)",
            "revisá el operando: una bandera definida como iota seguramente iba con 1 << iota",
            "x & 0 siempre da 0 y x | 0 o x ^ 0 siempre dan x; muchas veces la constante tenía que ser un bit.",
            "const FlagA = iota; modo & FlagA siempre da 0; declarala FlagA = 1 << iota.",
        ),
    ),
    (
        GoRule::Sa6002,
        RuleText::new(
            "sync.Pool.Put con un valor que no es un puntero (SA6002 de staticcheck)",
            "guardá un puntero en el pool, como *[]byte",
            "Meter un valor que no es puntero en la interfaz del pool reserva memoria en cada llamada, justo lo que el pool tenía que evitar.",
            "pool.Put(buf) con buf []byte se vuelve pool.Put(&buf).",
        ),
    ),
    (
        GoRule::Sa6003,
        RuleText::new(
            "Conversión de un string a []rune sólo para recorrerlo (SA6003 de staticcheck)",
            "recorré el string directamente",
            "La conversión reserva un slice tan largo como el string; recorrer el string da las mismas runas sin eso.",
            "for _, r := range []rune(s) se vuelve for _, r := range s.",
        ),
    ),
    (
        GoRule::Sa6005,
        RuleText::new(
            "Comparación sin mayúsculas con strings.ToLower o ToUpper (SA6005 de staticcheck)",
            "usá strings.EqualFold",
            "Pasar los dos strings a minúscula reserva dos copias, y EqualFold compara bien las mayúsculas de Unicode sin ellas.",
            "strings.ToLower(a) == strings.ToLower(b) se vuelve strings.EqualFold(a, b).",
        ),
    ),
    (
        GoRule::Sa9009,
        RuleText::new(
            "Directiva del compilador con un espacio después de // (SA9009 de staticcheck)",
            "sacá el espacio: //go:directiva",
            "El compilador sólo lee las directivas escritas //go: sin espacio, así que ésta se ignora sin avisar.",
            "// go:noinline se vuelve //go:noinline.",
        ),
    ),
    (
        GoRule::St1008,
        RuleText::new(
            "error que no es el último resultado (ST1008 de staticcheck)",
            "devolvé el error al final",
            "Por convención el error va último, y quien llama y las herramientas lo esperan ahí.",
            "func cargar() (error, []byte) se vuelve func cargar() ([]byte, error).",
        ),
    ),
    (
        GoRule::Qf1007,
        RuleText::new(
            "Bool declarado y después asignado en un if que se puede juntar (QF1007 de staticcheck)",
            "asigná la condición en la declaración",
            "x := false seguido de if cond { x = true } es lo mismo que x := cond, en una línea.",
            "hay := false; if n > 0 { hay = true } se vuelve hay := n > 0.",
        ),
    ),
    (
        GoRule::S1004,
        RuleText::new(
            "bytes.Compare comparado con cero (S1004 de staticcheck)",
            "usá bytes.Equal",
            "bytes.Equal dice lo mismo directo y es más rápido.",
            "bytes.Compare(a, b) == 0 se vuelve bytes.Equal(a, b).",
        ),
    ),
    (
        GoRule::S1006,
        RuleText::new(
            "for true {} en vez de for {} (S1006 de staticcheck)",
            "sacá el true",
            "En Go el loop infinito se escribe for {}; la condición no agrega nada.",
            "for true { ... } se vuelve for { ... }.",
        ),
    ),
    (
        GoRule::S1010,
        RuleText::new(
            "Slice hasta su propio len() (S1010 de staticcheck)",
            "sacá el segundo índice",
            "s[a:len(s)] es idéntico a s[a:].",
            "s[1:len(s)] se vuelve s[1:].",
        ),
    ),
    (
        GoRule::S1012,
        RuleText::new(
            "time.Now().Sub(t) en vez de time.Since(t) (S1012 de staticcheck)",
            "usá time.Since",
            "time.Since es la forma de siempre de decirlo y se lee mejor.",
            "time.Now().Sub(inicio) se vuelve time.Since(inicio).",
        ),
    ),
    (
        GoRule::S1023,
        RuleText::new(
            "break al final de un case o return al final de una función que sobran (S1023 de staticcheck)",
            "sacalo",
            "Un case de Go no sigue en el próximo y una función sin resultados vuelve al llegar al final; la sentencia repite lo que ya pasa.",
            "case 1: f(); break se vuelve case 1: f().",
        ),
    ),
    (
        GoRule::S1024,
        RuleText::new(
            "t.Sub(time.Now()) en vez de time.Until(t) (S1024 de staticcheck)",
            "usá time.Until",
            "time.Until es la forma de siempre de decirlo y se lee mejor.",
            "limite.Sub(time.Now()) se vuelve time.Until(limite).",
        ),
    ),
    (
        GoRule::S1028,
        RuleText::new(
            "errors.New(fmt.Sprintf(...)) en vez de fmt.Errorf (S1028 de staticcheck)",
            "usá fmt.Errorf con los mismos argumentos",
            "fmt.Errorf formatea y arma el error en un paso, y además puede envolver con %w.",
            "errors.New(fmt.Sprintf(\"id inválido %d\", id)) se vuelve fmt.Errorf(\"id inválido %d\", id).",
        ),
    ),
    (
        GoRule::S1033,
        RuleText::new(
            "if que mira una clave antes del delete (S1033 de staticcheck)",
            "llamá a delete sin el if",
            "delete de una clave que no está no hace nada, así que la guarda sobra.",
            "if _, ok := m[k]; ok { delete(m, k) } se vuelve delete(m, k).",
        ),
    ),
    (
        GoRule::S1037,
        RuleText::new(
            "select con un solo case de time.After en vez de time.Sleep (S1037 de staticcheck)",
            "usá time.Sleep",
            "Un select que sólo espera a time.After es una forma rebuscada de dormir.",
            "select { case <-time.After(d): } se vuelve time.Sleep(d).",
        ),
    ),
    (
        GoRule::Sa1012,
        RuleText::new(
            "nil pasado como context.Context (SA1012 de staticcheck)",
            "pasá context.TODO() o el contexto de quien llama",
            "El código que recibe un contexto espera que funcione: uno nil entra en pánico apenas se usa.",
            "cliente.Do(nil, pedido) se vuelve cliente.Do(ctx, pedido).",
        ),
    ),
    (
        GoRule::Sa1013,
        RuleText::new(
            "Seek con la constante io.Seek* como desplazamiento (SA1013 de staticcheck)",
            "invertí los argumentos: Seek(desplazamiento, desde)",
            "El primer argumento de Seek es el desplazamiento; pasar io.SeekStart ahí mueve a una posición equivocada.",
            "f.Seek(io.SeekStart, 0) se vuelve f.Seek(0, io.SeekStart).",
        ),
    ),
    (
        GoRule::Sa2000,
        RuleText::new(
            "sync.WaitGroup.Add dentro de la goroutine que cuenta (SA2000 de staticcheck)",
            "llamá a Add antes de arrancar la goroutine",
            "La goroutine puede arrancar cuando Wait ya volvió, y entonces Wait no la espera.",
            "go func() { wg.Add(1); ... }() se vuelve wg.Add(1); go func() { ... }().",
        ),
    ),
    (
        GoRule::Sa3001,
        RuleText::new(
            "Asignación a b.N en un benchmark (SA3001 de staticcheck)",
            "no asignes b.N; dejá que lo elija el paquete testing",
            "El paquete testing fija b.N para que la medición sea confiable; pisarlo deja el resultado sin sentido.",
            "b.N = 1000 se borra y el loop usa for range b.N o b.Loop().",
        ),
    ),
    (
        GoRule::Sa4001,
        RuleText::new(
            "&*x o *&x, que no copian x (SA4001 de staticcheck)",
            "usá x directamente, o copialo a mano a una variable nueva",
            "El compilador simplifica las dos formas a x, así que quien las escribió esperando una copia recibe el mismo puntero.",
            "p2 := &*p se vuelve copia := *p; p2 := &copia.",
        ),
    ),
    (
        GoRule::Sa4013,
        RuleText::new(
            "Doble negación de un bool (SA4013 de staticcheck)",
            "sacá las dos negaciones, o dejá una si era la idea",
            "!!x es x; casi siempre es un error de tipeo.",
            "if !!ok se vuelve if ok.",
        ),
    ),
    (
        GoRule::Sa4021,
        RuleText::new(
            "append sin nada que agregar (SA4021 de staticcheck)",
            "agregá los elementos que faltan o usá el slice directamente",
            "x = append(y) es lo mismo que x = y; casi siempre se olvidó un argumento.",
            "items = append(items) se vuelve items = append(items, item).",
        ),
    ),
    (
        GoRule::Sa4022,
        RuleText::new(
            "Comparación de la dirección de una variable con nil (SA4022 de staticcheck)",
            "compará la variable misma, o sacá el chequeo",
            "La dirección de una variable nunca es nil, así que la comparación da siempre lo mismo.",
            "if &cfg == nil se vuelve if cfg == nil, si cfg es un puntero.",
        ),
    ),
    (
        GoRule::Sa4024,
        RuleText::new(
            "len o cap comparado como menor que cero (SA4024 de staticcheck)",
            "corregí la comparación: len y cap nunca son negativos",
            "La condición nunca se cumple; casi siempre se quería == 0.",
            "if len(items) < 0 se vuelve if len(items) == 0.",
        ),
    ),
    (
        GoRule::Sa4028,
        RuleText::new(
            "x % 1, que siempre da cero (SA4028 de staticcheck)",
            "revisá el divisor",
            "Cualquier entero módulo 1 da 0, así que la expresión es una constante disfrazada.",
            "if n % 1 == 0 se vuelve if n % 2 == 0, si se quería la paridad.",
        ),
    ),
    (
        GoRule::Sa5002,
        RuleText::new(
            "for vacío que gira sin parar (SA5002 de staticcheck)",
            "esperá en un canal, una primitiva de sync o time.Sleep",
            "Un loop vacío usa una CPU entera, y si la condición depende de otra goroutine además es una carrera de datos.",
            "for !listo {} se vuelve <-canalListo.",
        ),
    ),
    (
        GoRule::Sa6006,
        RuleText::new(
            "io.WriteString con un []byte convertido a string (SA6006 de staticcheck)",
            "llamá a w.Write con los bytes",
            "Convertir los bytes a string los copia sólo para que WriteString los escriba.",
            "io.WriteString(w, string(datos)) se vuelve w.Write(datos).",
        ),
    ),
];
