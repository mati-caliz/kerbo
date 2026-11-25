a = t'hello {name}'
b = t"{x!r:>10}" t'{y}'
c = tr'{x}\d'
d = t'{x["a"]}' t'{{braces}}'
e = (t'template with {a_value} and {another_value} ' t'that goes on past the line limit {third_value}')
f = t"""triple {x}
second {y}"""
g = t'{a=}' t"{ b = }"
