#!/usr/bin/env python
#comentario sin espacio
#: documentación de sphinx
import os  # al final de un import
import sys

# antes de un from
from a import (  # del paréntesis
    b,  # de b
    # antes de c
    c,
)
from d import (
    e  # antes de la coma
    ,
    f,
)

x = 1  # al final
# pegado a y
y = 2
# después de y, separado de z

z = 3


# antes de la función
def function(  # del paréntesis de los parámetros
    first,  # del primero
    # antes del segundo
    second=2,
    /,  # de la barra
    third: int = 3,
    # antes del asterisco
    *,  # del asterisco
    fourth,
    **kwargs,  # del último
):  # de los dos puntos
    # primer comentario del cuerpo
    value = first  # de la asignación
    # antes del if
    if value:  # del if
        pass  # del pass
        # al final del cuerpo del if
    # antes del elif
    elif second:  # del elif
        pass
    # antes del else
    else:  # del else
        pass
    # después del if

    for item in third:  # del for
        continue
    # antes del else del for
    else:  # del else del for
        pass

    while fourth:  # del while
        break
        # al final del while

    try:  # del try
        pass
    # antes del except
    except ValueError:  # del except
        pass
    # antes del segundo except
    except (TypeError, KeyError) as error:  # del segundo
        pass
    # antes del else del try
    else:  # del else del try
        pass
    # antes del finally
    finally:  # del finally
        pass

    with open(first) as handle:  # del with
        pass

    return value  # del return
    # al final de la función


# entre la función y la clase


# antes del decorador
@decorator  # del decorador
# entre el decorador y la clase
class Example(Base):  # de la clase
    # antes del docstring
    """Docstring."""

    # antes del atributo
    attribute = 1  # del atributo

    # antes del método
    def method(self):  # del método
        # sólo un comentario antes del pass
        pass

    @property
    # entre el decorador y el def
    def other(self):
        return [  # del corchete
            1,  # del uno
            # antes del dos
            2,
            3,
            # al final de la lista
        ]

    def empty(self): ...  # de los puntos


call(  # del paréntesis de la llamada
    a,  # de a
    # antes de b
    b,
    key=value,  # del argumento con nombre
    # antes de los kwargs
    **kwargs,
)

call(
    # sólo un comentario
)

empty = []  # de la lista vacía
values = [
    # sólo un comentario en la lista
]
mapping = {  # de la llave
    "a": 1,  # del par
    # antes de b
    "b": 2,
    **other,  # del desempaquetado
}
pair = (  # del paréntesis de la tupla
    1,
    2,  # del dos
)

result = (
    first  # del primer operando
    + second  # del segundo
    # antes del tercero
    + third
)

condition = (
    a  # de a
    and b  # de b
    # antes de c
    or c
)

if (
    a  # de la condición
    # después de la condición
):
    pass

chained = (
    obj.method()  # del método
    .other()  # del otro
    # antes del último
    .last()
)

ternary = (
    a  # del cuerpo
    if b  # de la condición
    else c  # del else
)

comprehension = [
    item  # del elemento
    for item in items  # del for
    # antes del if
    if item  # del if
]

text = (
    "primera parte "  # de la primera
    "segunda parte"  # de la segunda
)

sliced = values[
    1:  # del límite de abajo
    2
]

lambda_value = lambda x: (  # del paréntesis de la lambda
    x
)


def trailing():
    pass


# al final del archivo
