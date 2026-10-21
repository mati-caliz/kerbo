import json
from dataclasses import dataclass


@dataclass
class Product:
    sku: str
    quantity: int


def restock(product, amount):
    return Product(product.sku, product.quantity + amount)


def is_empty(product):
    return product.quantity == 0
