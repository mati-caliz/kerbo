"""Argumentos de más en `%` y `.format`: comas, espacios y efectos."""


def compute():
    return 1


"%(a)s" % {"a": 1, "b": 2, }
"%(a)s" % {"a": 1, "b": 2 }
"%(a)s" % { "b": 2, "a": 1 }
"%(a)s" % {"a": 1, "b": compute()}
"%(a)s" % {"a": compute(), "b": 2}
"%(a)s" % {"a": 1, 2: 3, "c": 4}
"{a}".format(a=1, b=2, )
"{a}".format(a=1, b=2 )
"{a}".format( b=2, a=1)
"{a}".format(b=2, a=1)
"{a}".format(a=1, b=compute())
"{a}".format(a=compute(), b=2)
"{0}".format(1, 2, )
"{0}".format(1, 2 )
"{0}".format(1, compute())
"{0}".format(1,  # comentario
             2)
("hola").format(1)
"hola" "chau".format(1)
"{{}}".format(1)
"{0}".format(1, 2, a=3)
"{a}".format(1, a=2)
"{0}{2}".format(1, 2, 3)
"{0}".format(
    1,
    2,
)
"%(a)s" % {
    "a": 1,
    "b": 2,
}
