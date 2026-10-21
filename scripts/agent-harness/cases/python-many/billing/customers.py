import re
import logging
from collections import defaultdict, OrderedDict
from typing import Optional, List


class Customer:
    def __init__(self, name, email):
        self.name = name
        self.email = email
        self.blocked = False

    def contact(self):
        label = f"Contacto"
        return self.name + " <" + self.email + ">"

    def is_blocked(self):
        return self.blocked == True


def by_domain(customers):
    grouped = {}
    for customer in customers:
        domain = customer.email.split("@")[1]
        unused_count = len(customers)
        grouped.setdefault(domain, []).append(customer)
    return grouped


def find(customers, name):
    for customer in customers:
        if customer.name == name:
            return customer
    return None


def first_active(customers):
    for customer in customers:
        if not customer.blocked == True:
            return customer
    return None


def safe_email(customer):
    try:
        return customer.email.lower()
    except:
        return None
