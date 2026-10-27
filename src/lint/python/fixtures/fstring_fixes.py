name = "mundo"
items = [1, 2]
mapping = {"key": 1}
number = 3


def builder():
    return"{}".format(name)


accessed_sum = "{.real}".format(number + 1)
accessed_integer = "{.real}".format(12)
accessed_separated = "{.real}".format(1_000)
accessed_float = "{.real}".format(1.5)
accessed_parenthesized = "{.real}".format((number + 1))
bare_sum = "{}".format(number + 1)
bare_generator = "{}".format(x for x in items)
bare_set = "{}".format({1, 2})
bare_named = "{}".format(value := 1)
subscript_of_dict = "{}".format({1: 2}[1])
attribute_of_set = "{}".format({1}.pop)
negated = "{.real}".format(-number)
conditional = "{.real}".format(number if number else 0)
string_index_double = "{0[key]}".format(mapping)
string_index_single = '{0[key]}'.format(mapping)
string_index_triple = """{0[key]}""".format(mapping)
numeric_index = "{0[1]}".format(items)
attribute_chain = "{0.real.imag}".format(number)
conversion = "{!r:>10}".format(name)
specified = "{:>{}}".format(name, 10)
keyword = "{greeting}, {target}".format(greeting="hola", target=name) if False else None
keyword_plain = "{greeting}".format(greeting=name)
raw = r"\d{}".format(name)
raw_upper = R"\d{}".format(name)
unicode = u"{}".format(name)
escaped = "{{literal}} {}".format(name)
named_escape = "\N{BULLET} {}".format(name)
concatenated = "a {} " "b {}".format(name, number)
with_literal = "plain " "{}".format(name)
empty_first = "" "{}".format(name)
empty_middle = "{}" "" "{}".format(name, number)
parenthesized = ("{}").format(name)
split = ("first {} "
         "second {}").format(name, number)
commented = "{}".format(
    name,  # comentario
)
repeated_effect = "{0} {0}".format(builder())
repeated_plain = "{0} {0}".format(name)
starred = "{}".format(*items)
quoted_argument = "{}".format("texto")
multiline_argument = "{}".format(
    name
)
method = "{}".format(name).upper()
