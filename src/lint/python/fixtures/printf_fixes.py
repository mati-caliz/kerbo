value = 1
values = (1, 2)
mapping = {"a": 1}


class Holder:
    items = [1, 2]

    def get(self):
        return values


holder = Holder()

flags = "%-5f %+f %#x % f %+ f %05.2f %-05f" % (1.0, 2.0, 3, 4.0, 5.0, 6.0, 7.0)
precisions = "%.3x %.2e %.f %10.4g" % (1, 2.0, 3.0, 4.0)
conversions = "%r %a %s %o %X %E %G %%" % (value, value, value, 8, 255, 1.0, 2.0)
braces = "{literal} %s {}" % (value,)
named_escape = "\N{BULLET} {x} %s" % value
single_tuple = "%s" % (value,)
single_tuple_spaced = "%s" % ( value , )
parenthesized = ("%s and %s") % values
concatenated = "first %s " "second %s" % (1, 2)
split = ("first %s "
         "second %s") % (1, 2)
name_many = "%s %s" % values
attribute_many = "%s %s" % holder.items
call_many = "%s %s" % holder.get()
subscript_one = "%s" % values[0]
keyword_name = "%(a)s" % mapping
keyword_dictionary = "%(a)s %(b)r" % {"a": 1, "b": (2)}
splat_dictionary = "%(a)s %(b)s" % {"a": 1, **mapping}
multiline_dictionary = "%(a)s %(b)s" % {
    "a": 1,
    "b": 2,
}
literal_right = "%s" % "text"
number_right = "%s" % 3
none_right = "%s" % None
prefixed = r"%s\d" % value
unicode_prefixed = u"%s" % value
single_quotes = '%s "quoted"' % value
triple = """%s
line""" % value
nested_key = "%((a))s" % {"(a)": 1}
length_modifier = "%ld %lf" % (1, 2.0)
commented = ("%s"  # comment
             ) % value


def inside():
    return "%(first)s %(second)s" % {
        "first": 1,
        "second": 2,
    }
