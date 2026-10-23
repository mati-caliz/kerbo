import builtins

vacio = dict()
con_nombres = dict(a=1, b=2)
con_splat = dict(**base)
mezclado = dict(a=1, **base)
posicional = dict(pares)
lista = list()
lista_nombrada = list(x=1)
tupla = tuple()
generador = dict(x for x in pares)
estrellado = list(*partes)
calificado = builtins.dict()


def sombra(dict, list):
    return dict(), list()


texto = "hola %s" % nombre
tupla_formato = "%s y %s" % (a, b)
diccionario = "%(a)s" % {"a": 1}
diccionario_mixto = "%s" % {"a": 1}
entero = "%d" % numero
ancho = "%5s" % nombre
repr_simple = "%r" % objeto
repr_ancho = "%10r" % objeto
estrella = "%*s" % (5, nombre)
negativo = "%s" % -numero
lista_derecha = "%s" % [1]
llamada = "%s" % f(x)
literal = "%s" % 1
bytes_formato = b"%s" % valor
fstring_parte = f"{a}" "%s" % valor
concatenado = "a %s" " b %s" % (x, y)
escapado = "100%%" % ()
invalido = "%y" % valor
clave_vacia = "%()s" % {"": 1}
precision = "%.2f" % numero
flags = "%-5s" % nombre
parentesis = ("hola %s") % nombre
clave_rara = "%(a-b)s" % {"a-b": 1}
clave_repetida = "%(a)s" % {"a": 1, "a": 2}
clave_palabra = "%(class)s" % {"class": 1}
clave_no_string = "%(a)s" % {1: 2}
dict_vacio = "%(a)s" % {}
solo_splat = "%(a)s" % {**base}
varias_lineas = "%(a)s %(b)s" % {"a": 1,
    "b": 2}
varias_lineas_sin_inicio = "%(a)s" % {"a": 1, "b":
    2}
