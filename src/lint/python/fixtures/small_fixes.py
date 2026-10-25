import os as os
from decimal import Decimal

#
x = 1  #
y = 2 \
    #


def unpack():
    first, second = os.getcwd(), os.sep
    third, fourth_ = os.path.split("a/b")
    return first


def bare():
    print(1)
    return  # sin valor


def alone():
    return None


values = {1, 2, 1, 3, 2}
single = {1, 1}
amounts = [Decimal("1_000"), Decimal("-0_01"), Decimal(" +42 "), Decimal("00"), Decimal(float("-nan"))]
