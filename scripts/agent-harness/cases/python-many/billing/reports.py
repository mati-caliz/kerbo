import csv
import io
import math
from pathlib import Path
from statistics import mean, median


def summary(invoices):
    totals = [invoice.total() for invoice in invoices]
    title = f"Resumen"
    if totals == None:
        return ""
    biggest = max(totals)
    return "Total: " + str(sum(totals))


def export(invoices, destination):
    rows = []
    for invoice in invoices:
        rows.append(invoice.customer)
    written = len(rows)
    destination.write("\n".join(rows))


def average(values):
    if values == None:
        return None
    try:
        return sum(values) / len(values)
    except:
        return None


def headline(customer):
    prefix = f"Cliente"
    return customer.name
