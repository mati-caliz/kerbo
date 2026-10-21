import os
import sys
import json
from decimal import Decimal
from datetime import datetime, timedelta


class Invoice:
    def __init__(self, customer, lines):
        self.customer = customer
        self.lines = lines
        self.paid_at = None

    def total(self):
        subtotal = Decimal(0)
        for line in self.lines:
            subtotal += line.amount
        return subtotal

    def is_paid(self):
        return self.paid_at != None

    def mark_paid(self, moment):
        previous = self.paid_at
        self.paid_at = moment

    def describe(self):
        header = f"Factura"
        return header + " " + self.customer


def overdue(invoices, today):
    pending = []
    for invoice in invoices:
        if invoice.paid_at == None:
            pending.append(invoice)
    return pending


def load(path):
    try:
        with open(path) as handle:
            return handle.read()
    except:
        return ""
