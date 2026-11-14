// Las firmas genéricas de un `.class` (el atributo `Signature`), que el descriptor borra:
// `List.get` devuelve `E`, no `Object`, y `ArrayList<E>` implementa `List<E>`.

use super::class_file::binary_to_source;

const PARAMETERS_OPEN: u8 = b'<';
const PARAMETERS_CLOSE: u8 = b'>';
const BOUND_SEPARATOR: u8 = b':';
const CLASS_START: u8 = b'L';
const VARIABLE_START: u8 = b'T';
const ARRAY_START: u8 = b'[';
const TYPE_END: u8 = b';';
const INNER_SEPARATOR: u8 = b'.';
const NESTED_SEPARATOR: char = '$';
const PARAMETERS_START: u8 = b'(';
const PARAMETERS_END: u8 = b')';
const ANY_ARGUMENT: u8 = b'*';
const UPPER_BOUND: u8 = b'+';
const LOWER_BOUND: u8 = b'-';
const PRIMITIVES: &[u8] = b"BCDFIJSZV";

// Un tipo de una firma. `Other` es lo que no sirve para seguir una cadena: un primitivo, un arreglo
// o un comodín sin cota de arriba.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Generic {
    Class(String, Vec<Self>),
    Variable(String),
    Other,
}

// Los parámetros de tipo de una clase y de quién hereda, con los argumentos que les pasa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassSignature {
    pub parameters: Vec<String>,
    pub parents: Vec<Generic>,
}

// Lo que devuelve un método, los tipos de sus parámetros, y sus propios parámetros de tipo, que
// tapan a los de la clase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSignature {
    pub parameters: Vec<String>,
    pub arguments: Vec<Generic>,
    pub returned: Generic,
}

pub fn class_signature(signature: &str) -> Option<ClassSignature> {
    let mut reader = Reader::new(signature);
    let parameters = reader.type_parameters()?;
    let mut parents = Vec::new();
    while !reader.is_done() {
        parents.push(reader.reference()?);
    }
    Some(ClassSignature { parameters, parents })
}

pub fn method_signature(signature: &str) -> Option<MethodSignature> {
    let mut reader = Reader::new(signature);
    let parameters = reader.type_parameters()?;
    reader.expect(PARAMETERS_START)?;
    let mut arguments = Vec::new();
    while reader.peek()? != PARAMETERS_END {
        arguments.push(reader.java_type()?);
    }
    reader.expect(PARAMETERS_END)?;
    let returned = reader.java_type()?;
    Some(MethodSignature {
        parameters,
        arguments,
        returned,
    })
}

// El tipo de un campo (`Ljava/util/List<Ljava/lang/String;>;`).
pub fn field_signature(signature: &str) -> Option<Generic> {
    let mut reader = Reader::new(signature);
    let found = reader.reference()?;
    reader.is_done().then_some(found)
}

struct Reader<'text> {
    bytes: &'text [u8],
    offset: usize,
}

impl<'text> Reader<'text> {
    const fn new(text: &'text str) -> Self {
        Self {
            bytes: text.as_bytes(),
            offset: 0,
        }
    }

    const fn is_done(&self) -> bool {
        self.offset >= self.bytes.len()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let found = self.peek()?;
        self.offset = self.offset.saturating_add(1);
        Some(found)
    }

    fn expect(&mut self, wanted: u8) -> Option<()> {
        (self.next()? == wanted).then_some(())
    }

    fn until(&mut self, stops: &[u8]) -> Option<&'text str> {
        let start = self.offset;
        while !stops.contains(&self.peek()?) {
            self.offset = self.offset.saturating_add(1);
        }
        std::str::from_utf8(self.bytes.get(start..self.offset)?).ok()
    }

    fn type_parameters(&mut self) -> Option<Vec<String>> {
        let mut parameters = Vec::new();
        if self.peek() != Some(PARAMETERS_OPEN) {
            return Some(parameters);
        }
        self.expect(PARAMETERS_OPEN)?;
        while self.peek()? != PARAMETERS_CLOSE {
            parameters.push(self.until(&[BOUND_SEPARATOR])?.to_owned());
            self.skip_bounds()?;
        }
        self.expect(PARAMETERS_CLOSE)?;
        Some(parameters)
    }

    // Las cotas de un parámetro de tipo: la de clase puede faltar (`T::Ljava/lang/Runnable;`).
    fn skip_bounds(&mut self) -> Option<()> {
        while self.peek() == Some(BOUND_SEPARATOR) {
            self.expect(BOUND_SEPARATOR)?;
            if !matches!(self.peek()?, BOUND_SEPARATOR | PARAMETERS_CLOSE) {
                self.reference()?;
            }
        }
        Some(())
    }

    fn java_type(&mut self) -> Option<Generic> {
        if PRIMITIVES.contains(&self.peek()?) {
            self.next()?;
            return Some(Generic::Other);
        }
        self.reference()
    }

    fn reference(&mut self) -> Option<Generic> {
        match self.next()? {
            CLASS_START => self.class_type(),
            VARIABLE_START => {
                let name = self.until(&[TYPE_END])?.to_owned();
                self.expect(TYPE_END)?;
                Some(Generic::Variable(name))
            }
            ARRAY_START => {
                self.java_type()?;
                Some(Generic::Other)
            }
            _ => None,
        }
    }

    fn class_type(&mut self) -> Option<Generic> {
        let mut binary = String::new();
        loop {
            binary.push_str(self.until(&[PARAMETERS_OPEN, INNER_SEPARATOR, TYPE_END])?);
            let arguments = if self.peek()? == PARAMETERS_OPEN {
                self.type_arguments()?
            } else {
                Vec::new()
            };
            match self.next()? {
                INNER_SEPARATOR => binary.push(NESTED_SEPARATOR),
                TYPE_END => return Some(Generic::Class(binary_to_source(&binary), arguments)),
                _ => return None,
            }
        }
    }

    fn type_arguments(&mut self) -> Option<Vec<Generic>> {
        self.expect(PARAMETERS_OPEN)?;
        let mut arguments = Vec::new();
        while self.peek()? != PARAMETERS_CLOSE {
            let argument = match self.peek()? {
                ANY_ARGUMENT => {
                    self.next()?;
                    Generic::Other
                }
                UPPER_BOUND => {
                    self.next()?;
                    self.reference()?
                }
                LOWER_BOUND => {
                    self.next()?;
                    self.reference()?;
                    Generic::Other
                }
                _ => self.reference()?,
            };
            arguments.push(argument);
        }
        self.expect(PARAMETERS_CLOSE)?;
        Some(arguments)
    }
}
