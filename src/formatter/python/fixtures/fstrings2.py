x = f"{a:{b}}" f'{c!r:^{d}}'
x = f"""{
    a
}"""
x = f"""{a:{
    b}}"""
x = f"{a}" f"""{b}"""
x = f'{a["b"]}' 'c'
x = f"{'\n'}"
x = f'{x:\'}' if 0 else 1
foo(f"{very_long_function_name(argument_one, argument_two)} and {another_very_long_function(argument_three)}")
x = f"{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + ccccccccccccccccccccc}"
x = f"{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb +
    ccccccccccccccccccccc}"
x = f"{x!r}" f'{y!s}'
x = f'{x:{"a"}}' f'{y}'
x = f"{x}" f'{y["a"]}'
x = b"abc" b'def'
x = f'\N{BULLET} {x}' 'a\N{bullet}'
x = f"a" "b"
x = f'{x}' 'it\'s'
x = f"{x}" "{y}"
x = f'' f''
x = f"{x:}"
x = f"{x!r:}"
x = f"{ x }"
x = f"{x=!r:^20}"
x = f"{3.14:10.10}" f"{3.14!r:10.10}"
x = f"{a['x']=}"
x = f'{a["x"]=}'
x = f'{a["x"]=}' f"{b}"
x = f"{ {} }"
x = f"{ {}[1] }"
x = f"{ {'a': 1}.get('a') }"
x = f"{yield}"
x = f"{(yield)}"
x = f"{await y}"
x = f"{*a, b}"
x = f"{(a := 1)}"
x = f"{(lambda: 1)}"
x = f"{ a if b else c }"
x = f'{a}\{b}'
x = f"\{a}"
x = r"\{a}"
x = f"{a := 1}"
x = f"{a:= 1}"
with (open(f) as g):
    pass
with (open(f) as g, a as b):
    pass
y = container[int:obj, int]
