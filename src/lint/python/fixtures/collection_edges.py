items = [3, 1, 2]
flag = True
first = reversed(sorted(items, reverse=True))
second = reversed(sorted(items, reverse=not flag))
third = reversed(sorted(items, reverse=flag))
fourth = reversed(sorted(x for x in items))
fifth = reversed(sorted(items, key=len,))
sixth = list(sorted(items))
pairs = dict([(1, 2), (3, 4)])
spread = dict([
    (1, 2),
    (3, 4),
])
tight = dict(((1, 2),))
cast = sorted(list(items), key=len)
nested = set(tuple(items))
