"""Un módulo que se llama como uno de la biblioteca estándar: sus definiciones son `gzip.open`."""


def open(name):
    return name


def reads_without_with(name):
    handle = open(name)
    return handle.read()
