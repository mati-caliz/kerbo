from typing import TypeAlias

Recursive: TypeAlias = list[Recursive]
Plain = list[Plain]
value: Later

def function(argument: Later) -> Later: ...

class Later: ...

missing: Undefined
