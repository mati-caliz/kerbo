use crate::lint::taint::{Flow, Literal, Value};

const LIST_TYPES: &[&str] = &["ArrayList", "LinkedList", "Vector", "CopyOnWriteArrayList"];
const MAP_TYPES: &[&str] = &[
    "HashMap",
    "LinkedHashMap",
    "TreeMap",
    "Hashtable",
    "ConcurrentHashMap",
];

// Una lista o un mapa vacíos recién creados, que se siguen posición por posición o clave por clave.
pub fn empty_collection(created: &str) -> Option<Value> {
    if LIST_TYPES.contains(&created) {
        Some(Value::List(Vec::new()))
    } else if MAP_TYPES.contains(&created) {
        Some(Value::Map(Vec::new()))
    } else {
        None
    }
}

fn constant_index(argument: Option<&Value>, limit: usize) -> Option<usize> {
    match argument?.literal()? {
        Literal::Integer(number) => usize::try_from(*number).ok().filter(|index| *index < limit),
        Literal::Text(_) | Literal::Character(_) | Literal::Boolean(_) => None,
    }
}

fn constant_key(argument: Option<&Value>) -> Option<String> {
    match argument?.literal()? {
        Literal::Text(key) => Some(key.clone()),
        Literal::Integer(_) | Literal::Character(_) | Literal::Boolean(_) => None,
    }
}

fn everything(collection: &Value, arguments: &[Value]) -> Flow {
    arguments
        .iter()
        .fold(collection.flow(), |flow, argument| flow.union(&argument.flow()))
}

// Lo que devuelve una llamada sobre una lista o un mapa que se siguen, y cómo quedan. Lo que no
// se puede seguir junta todos sus datos y deja de seguirse por posición.
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
        ("add", [item]) => {
            items.push(item.clone());
            Some(Value::clean())
        }
        ("add", [_, item]) => {
            let index = constant_index(arguments.first(), items.len().saturating_add(1))?;
            items.insert(index, item.clone());
            Some(Value::clean())
        }
        ("get", [_]) => Some(
            items
                .get(constant_index(arguments.first(), items.len())?)?
                .clone(),
        ),
        ("set", [_, item]) => {
            let index = constant_index(arguments.first(), items.len())?;
            let previous = std::mem::replace(items.get_mut(index)?, item.clone());
            Some(previous)
        }
        ("remove", [_]) => {
            let index = constant_index(arguments.first(), items.len())?;
            Some(items.remove(index))
        }
        ("clear", []) => {
            items.clear();
            Some(Value::clean())
        }
        ("size", []) => i64::try_from(items.len())
            .ok()
            .map(|size| Value::Constant(Literal::Integer(size))),
        ("isEmpty", []) => Some(Value::Constant(Literal::Boolean(items.is_empty()))),
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

fn map_call(entries: &mut Vec<(String, Value)>, method: &str, arguments: &[Value]) -> Option<Value> {
    let key = constant_key(arguments.first());
    match (method, arguments) {
        ("put", [_, item]) => {
            let wanted = key?;
            let previous = position_of(entries, &wanted)
                .and_then(|index| entries.get_mut(index))
                .map(|(_, existing)| std::mem::replace(existing, item.clone()));
            if previous.is_none() {
                entries.push((wanted, item.clone()));
            }
            Some(previous.unwrap_or_else(Value::clean))
        }
        ("get", [_]) => Some(value_at(entries, &key?).unwrap_or_else(Value::clean)),
        ("getOrDefault", [_, fallback]) => Some(value_at(entries, &key?).unwrap_or_else(|| fallback.clone())),
        ("remove", [_]) => {
            let index = position_of(entries, &key?)?;
            Some(entries.remove(index).1)
        }
        ("containsKey", [_]) => Some(Value::clean()),
        ("clear", []) => {
            entries.clear();
            Some(Value::clean())
        }
        _ => None,
    }
}
