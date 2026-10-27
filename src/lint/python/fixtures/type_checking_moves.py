"""Bordes del arreglo de TC004."""

# Comentario del principio.

if TYPE_CHECKING:
    from alpha import (
        first,  # se queda el comentario
        second,
    )
    from beta import (
        third,  # otro
        fourth  # el último
    )
    from gamma import (fifth,
                       sixth)
    from delta import seventh as eighth, ninth
    import epsilon.zeta, eta
    from theta import iota  # al costado

print(first, third, fifth, eighth, epsilon.zeta, iota)


def nested():
    if TYPE_CHECKING:
        from kappa import lam, mu

    return lam


class Holder:
    if TYPE_CHECKING:
        from nu import xi

    value = xi


def annotations(a: "second", b: "fourth", c: "sixth", d: "ninth", e: "eta", f: "mu") -> None:
    return None
