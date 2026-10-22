use crate::lint::taint::{Flow, Literal, Value};

const CONFIG_SEPARATOR: char = '\u{1f}';

fn constant_index(argument: Option<&Value>, length: usize) -> Option<usize> {
    let Some(Literal::Integer(number)) = argument?.literal() else {
        return None;
    };
    let length = i64::try_from(length).ok()?;
    let position = if *number < 0 { length + number } else { *number };
    usize::try_from(position).ok()
}

fn constant_key(argument: Option<&Value>) -> Option<String> {
    match argument?.literal()? {
        Literal::Text(key) => Some(key.clone()),
        Literal::Integer(number) => Some(number.to_string()),
        Literal::Character(_) | Literal::Boolean(_) => None,
    }
}

fn everything(collection: &Value, arguments: &[Value]) -> Flow {
    arguments
        .iter()
        .fold(collection.flow(), |flow, argument| flow.union(&argument.flow()))
}

// Lo que devuelve una llamada sobre una lista o un diccionario que se siguen, y cómo quedan. Lo
// que no se puede seguir junta todos sus datos y deja de seguirse por posición.
pub fn collection_call(collection: &mut Value, method: &str, arguments: &[Value]) -> Value {
    let followed = match collection {
        Value::List(items) => list_call(items, method, arguments),
        Value::Map(entries) => map_call(entries, method, arguments),
        Value::Constant(_) | Value::Data(_) => None,
    };
    followed.unwrap_or_else(|| {
        let flow = everything(collection, arguments);
        *collection = Value::Data(flow.clone());
        Value::Data(flow)
    })
}

fn list_call(items: &mut Vec<Value>, method: &str, arguments: &[Value]) -> Option<Value> {
    match (method, arguments) {
        ("append", [item]) => {
            items.push(item.clone());
            Some(Value::clean())
        }
        ("insert", [_, item]) => {
            let index = constant_index(arguments.first(), items.len().saturating_add(1))?;
            items.insert(index.min(items.len()), item.clone());
            Some(Value::clean())
        }
        ("pop", []) => items.pop(),
        ("pop", [_]) => {
            let index =
                constant_index(arguments.first(), items.len()).filter(|index| *index < items.len())?;
            Some(items.remove(index))
        }
        ("clear", []) => {
            items.clear();
            Some(Value::clean())
        }
        _ => None,
    }
}

fn map_call(entries: &mut Vec<(String, Value)>, method: &str, arguments: &[Value]) -> Option<Value> {
    let key = constant_key(arguments.first());
    match (method, arguments) {
        ("get", [_]) => Some(value_at(entries, &key?).unwrap_or_else(Value::clean)),
        ("get", [_, fallback]) => Some(value_at(entries, &key?).unwrap_or_else(|| fallback.clone())),
        ("pop", [_]) => {
            let index = position_of(entries, &key?)?;
            Some(entries.remove(index).1)
        }
        ("clear", []) => {
            entries.clear();
            Some(Value::clean())
        }
        ("keys" | "__contains__", _) => Some(Value::clean()),
        _ => None,
    }
}

fn position_of(entries: &[(String, Value)], wanted: &str) -> Option<usize> {
    entries.iter().position(|(existing, _)| existing == wanted)
}

fn value_at(entries: &[(String, Value)], wanted: &str) -> Option<Value> {
    position_of(entries, wanted)
        .and_then(|index| entries.get(index))
        .map(|(_, item)| item.clone())
}

// Un `ConfigParser` se sigue como un diccionario con la sección y la opción como clave.
pub fn config_call(entries: &mut Vec<(String, Value)>, method: &str, arguments: &[Value]) -> Value {
    let key = match (constant_key(arguments.first()), constant_key(arguments.get(1))) {
        (Some(section), Some(option)) => Some(format!("{section}{CONFIG_SEPARATOR}{option}")),
        _ => None,
    };
    match (method, arguments, key) {
        ("set", [_, _, value], Some(key)) => {
            match position_of(entries, &key).and_then(|position| entries.get_mut(position)) {
                Some((_, slot)) => *slot = value.clone(),
                None => entries.push((key, value.clone())),
            }
            Value::clean()
        }
        ("get", [_, _, ..], Some(key)) => value_at(entries, &key).unwrap_or_else(Value::clean),
        _ => Value::Data(
            entries
                .iter()
                .fold(Flow::default(), |flow, (_, item)| flow.union(&item.flow())),
        ),
    }
}

// `coleccion[indice]`: el elemento si la posición o la clave son constantes, si no, todo junto. Un
// texto constante con un índice constante da el carácter, como `"ABC"[1]`.
pub fn subscript(collection: &Value, index: &Value) -> Value {
    match collection {
        Value::List(items) => constant_index(Some(index), items.len())
            .and_then(|position| items.get(position))
            .cloned()
            .unwrap_or_else(|| Value::Data(collection.flow())),
        Value::Map(entries) => constant_key(Some(index))
            .and_then(|key| value_at(entries, &key))
            .unwrap_or_else(|| Value::Data(collection.flow())),
        Value::Constant(Literal::Text(written)) => {
            let characters: Vec<char> = written.chars().collect();
            constant_index(Some(index), characters.len())
                .and_then(|position| characters.get(position))
                .map_or_else(Value::clean, |character| {
                    Value::Constant(Literal::Text(character.to_string()))
                })
        }
        Value::Constant(_) | Value::Data(_) => Value::Data(collection.flow()),
    }
}

// `coleccion[indice] = valor`: con posición o clave constante se reemplaza; si no, se junta todo.
pub fn assign_subscript(collection: &mut Value, index: &Value, assigned: Value) {
    match collection {
        Value::List(items) => {
            if let Some(slot) =
                constant_index(Some(index), items.len()).and_then(|position| items.get_mut(position))
            {
                *slot = assigned;
                return;
            }
        }
        Value::Map(entries) => {
            if let Some(key) = constant_key(Some(index)) {
                match position_of(entries, &key).and_then(|position| entries.get_mut(position)) {
                    Some((_, slot)) => *slot = assigned,
                    None => entries.push((key, assigned)),
                }
                return;
            }
        }
        Value::Constant(_) | Value::Data(_) => {}
    }
    *collection = Value::Data(collection.flow().union(&assigned.flow()));
}
