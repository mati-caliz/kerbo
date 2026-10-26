class Parent:
    def method(self):
        return 1


class Child(Parent):
    def method(self):
        return super.method()

    @staticmethod
    def static_method():
        return super.method()

    @classmethod
    def class_method(cls):
        return (super).method()


def outside():
    return super.method()


extra = {"z": 0}
other_key = "o"

literal_repeated = {"a": 1, "a": 1}
literal_different = {"a": 1, "a": 2}
literal_three = {"b": 1, "b": 2, "b": 1}
literal_parenthesized = {"c": (1), "c": 1}
literal_spread = {"d": 1, **extra, "d": 1}
literal_trailing = {"e": [1], "e": [1],}
literal_multiline = {
    "f": 1,
    "f": 1,  # repeated
    "g": 2,
}
literal_numbers = {1: "one", 1.0: "one", True: "one"}
variable_repeated = {other_key: 1, other_key: 1}
variable_different = {other_key: 1, other_key: 2}
variable_three = {other_key: (1), other_key: 2, other_key: 1}
