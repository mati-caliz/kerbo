use super::class_file::binary_to_source;

const ACCESS_PRIVATE: u16 = 0x0002;
const ACCESS_STATIC: u16 = 0x0008;
const ACCESS_VARARGS: u16 = 0x0080;
pub(super) const DESCRIPTOR_OPEN: u8 = b'(';
pub(super) const DESCRIPTOR_CLOSE: u8 = b')';
pub(super) const OBJECT_START: u8 = b'L';
pub(super) const OBJECT_END: u8 = b';';
pub(super) const ARRAY: u8 = b'[';
const CLASS_DESCRIPTOR: &str = "Ljava/lang/Class;";

// Un método de un `.class`: su firma, los bits de acceso y las anotaciones de cada parámetro y del
// retorno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    pub name: String,
    pub descriptor: String,
    pub access: u16,
    pub parameter_annotations: Vec<Vec<String>>,
    pub return_annotations: Vec<String>,
}

impl Method {
    pub const fn is_private(&self) -> bool {
        self.access & ACCESS_PRIVATE != 0
    }

    pub const fn is_static(&self) -> bool {
        self.access & ACCESS_STATIC != 0
    }

    pub const fn parameter_count(&self) -> usize {
        self.parameter_annotations.len()
    }

    // Si una llamada con esa cantidad de argumentos puede ir a este método: la misma cantidad, o con
    // varargs, la de los fijos o más.
    pub const fn accepts(&self, arguments: usize) -> bool {
        let parameters = self.parameter_count();
        arguments == parameters || (self.access & ACCESS_VARARGS != 0 && arguments + 1 >= parameters)
    }

    // Si el parámetro en esa posición es un `Class`: elige la sobrecarga de una llamada con
    // `Long.class`, como lo hace el compilador.
    pub fn takes_class_at(&self, position: usize) -> bool {
        self.parameter_descriptors().get(position) == Some(&CLASS_DESCRIPTOR)
    }

    fn parameter_descriptors(&self) -> Vec<&str> {
        let parameters = self
            .descriptor
            .strip_prefix(char::from(DESCRIPTOR_OPEN))
            .and_then(|rest| rest.split(char::from(DESCRIPTOR_CLOSE)).next())
            .unwrap_or_default();
        let mut found = Vec::new();
        let mut start = 0;
        let bytes = parameters.as_bytes();
        let mut index = 0;
        while let Some(letter) = bytes.get(index) {
            if *letter == ARRAY {
                index += 1;
                continue;
            }
            if *letter == OBJECT_START {
                index = parameters
                    .get(index..)
                    .and_then(|rest| rest.find(char::from(OBJECT_END)))
                    .map_or(bytes.len(), |end| index + end);
            }
            index += 1;
            found.extend(parameters.get(start..index));
            start = index;
        }
        found
    }

    // La clase que devuelve, si es un objeto (`Ljava/lang/String;` da `java.lang.String`).
    pub fn return_class(&self) -> Option<String> {
        let (_, returned) = self.descriptor.split_once(char::from(DESCRIPTOR_CLOSE))?;
        let binary = returned
            .strip_prefix(char::from(OBJECT_START))?
            .strip_suffix(char::from(OBJECT_END))?;
        Some(binary_to_source(binary))
    }

    // Si devuelve un objeto o un arreglo (lo único que puede ser null).
    pub fn returns_reference(&self) -> bool {
        self.descriptor
            .bytes()
            .skip_while(|letter| *letter != DESCRIPTOR_CLOSE)
            .nth(1)
            .is_some_and(|letter| letter == OBJECT_START || letter == ARRAY)
    }
}
