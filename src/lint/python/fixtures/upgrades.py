# -*- coding: utf-8 -*-
"""Módulo de prueba."""
import builtins
import typing
from typing import TYPE_CHECKING, Protocol
from abc import abstractmethod

saludo = u"hola"
bytes_saludo = b"hola"
juntos = u"a" "b"


class Viejo(object):
    pass


class Calificado(builtins.object, Base):
    """Doc."""

    pass


class Prueba(TestCase):
    def test_algo(self):
        self.assertEquals(1, 1)
        self.failUnless(True)
        otro.assertEquals(1, 1)
        self.assertEqual(1, 1)


conjunto = set([1, 2])
conjunto_tupla = set((1, 2))
conjunto_vacio = set([])
conjunto_generador = set(x for x in lista)


def sombra(set):
    return set([1])


def expresiones(x, lista):
    x == 1
    (x < 2)
    x
    x.atributo
    llamada().atributo
    x[0]
    x + 1
    1 + 2
    -x
    not x
    lista()
    list()
    dict()
    [x, x]
    x, x
    b"bytes"
    f"{x}"
    ...
    await_algo = None
    lambda: llamada()
    lambda: x


def placeholders():
    """Doc."""
    pass


def solo_pass():
    pass


def elipsis():
    x = 1
    ...


if TYPE_CHECKING:
    def tipado():
        x = 1
        ...


class Interfaz(Protocol):
    def metodo(self):
        """Doc."""
        ...


class Abstracta:
    @abstractmethod
    def metodo(self):
        """Doc."""
        ...


if typing.TYPE_CHECKING:
    y = 1
    ...


def plantillas(nombre):
    "Hola, " + t"{nombre}"
    t"{nombre}"
