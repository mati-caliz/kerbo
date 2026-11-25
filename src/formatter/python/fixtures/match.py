match x:
    case 1:
        pass
    case 1 | -2 | 3+4j | 'a' 'b' | None | True | a.b:
        pass
    case [a, *rest] | (b, c) | (d,) | (e):
        pass
    case {'k': v, **rest} | Point(x=1, y=2) | Foo(1, a.b) as z if z > 1:
        pass
    case a, b:
        pass
    case []:
        pass
    case ():
        pass
    case {}:
        pass
    case Foo():
        pass
    case Foo(a):
        pass
    case Foo(  a  ,  b=2  ):
        pass
    case _:
        pass
match  (x , y):
    case (1, 2): pass
    case _: pass
match x, y:
    case 1, 2: pass
match(x):
    case [1, 2, 3]:
        pass
    # comment before case
    case "string":  # trailing
        pass

    case [
        1, 2
    ]:
        pass
match some_very_long_subject_expression_name.attribute_name_one.attribute_name_two.attribute_three:
    case SomeVeryLongClassNameForPattern(first_argument=1, second_argument=2, third_argument=3, fourth=4):
        pass
    case "first_long_string_pattern_alternative" | "second_long_string_pattern_alternative" | "third_one":
        pass
    case [first_element_pattern, second_element_pattern, third_element_pattern, fourth_element_pattern]:
        pass
    case {"key_one": value_one, "key_two": value_two, "key_three": value_three, "key_four": value_four}:
        pass
    case x if x > 0 and some_other_condition_that_is_long and yet_another_condition_to_make_it_long_enough:
        pass
