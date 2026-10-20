use std::collections::HashMap;

const COMMENT_START: &str = "<!--";
const COMMENT_END: &str = "-->";
const TAG_START: char = '<';
const TAG_END: char = '>';
const CLOSING_MARK: char = '/';
const DECLARATION_MARKS: [char; 2] = ['?', '!'];
const PROJECT: &str = "project";
const PROPERTIES: &str = "properties";
const DEPENDENCY: &str = "dependency";
const DEPENDENCIES: &str = "dependencies";
const MANAGEMENT: &str = "dependencyManagement";
const GROUP: &str = "groupId";
const ARTIFACT: &str = "artifactId";
const VERSION: &str = "version";
const PARENT: &str = "parent";
const RELATIVE_PATH: &str = "relativePath";
const FIRST_LINE: usize = 1;
const NEWLINE: char = '\n';

#[derive(Debug, Clone, Default)]
pub struct Declared {
    pub group: String,
    pub artifact: String,
    pub version: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ParentRef {
    pub artifact: String,
    pub group: String,
    pub version: String,
    pub relative_path: Option<String>,
    pub line: usize,
}

#[derive(Debug, Default)]
pub struct PomModel {
    pub group: Option<String>,
    pub artifact: Option<String>,
    pub version: Option<String>,
    pub parent: Option<ParentRef>,
    pub properties: HashMap<String, String>,
    pub dependencies: Vec<Declared>,
    pub managed: Vec<Declared>,
    pub stack: Vec<String>,
    pub pending: Option<Declared>,
}

impl PomModel {
    pub fn parse(text: &str) -> Self {
        let mut model = Self::default();
        let mut rest = text;
        let mut line = FIRST_LINE;
        while let Some(start) = rest.find(TAG_START) {
            let before = rest.get(..start).unwrap_or_default();
            line = line.saturating_add(newlines(before));
            let value = before.trim();
            if !value.is_empty() {
                model.text(value);
            }
            let tag_text = rest.get(start..).unwrap_or_default();
            let consumed = if tag_text.starts_with(COMMENT_START) {
                tag_text
                    .find(COMMENT_END)
                    .map_or(tag_text.len(), |end| end.saturating_add(COMMENT_END.len()))
            } else {
                let end = tag_text
                    .find(TAG_END)
                    .map_or(tag_text.len(), |end| end.saturating_add(1));
                model.tag(tag_text.get(1..end.saturating_sub(1)).unwrap_or_default(), line);
                end
            };
            line = line.saturating_add(newlines(tag_text.get(..consumed).unwrap_or_default()));
            rest = tag_text.get(consumed..).unwrap_or_default();
        }
        model
    }

    fn tag(&mut self, inside: &str, line: usize) {
        if inside.starts_with(DECLARATION_MARKS) {
            return;
        }
        if let Some(closing) = inside.strip_prefix(CLOSING_MARK) {
            if !closing.is_empty() {
                self.close();
            }
            return;
        }
        let name = inside
            .trim_end_matches(CLOSING_MARK)
            .split_whitespace()
            .next()
            .unwrap_or_default();
        self.open(name, line);
        if inside.ends_with(CLOSING_MARK) {
            self.close();
        }
    }

    fn path(&self) -> Vec<&str> {
        self.stack.iter().map(String::as_str).collect()
    }

    fn open(&mut self, name: &str, line: usize) {
        self.stack.push(name.to_owned());
        let opens_parent = matches!(self.path().as_slice(), [PROJECT, PARENT]);
        let opens_dependency = matches!(
            self.path().as_slice(),
            [PROJECT, DEPENDENCIES, DEPENDENCY] | [PROJECT, MANAGEMENT, DEPENDENCIES, DEPENDENCY]
        );
        if opens_parent {
            self.parent = Some(ParentRef {
                line,
                ..ParentRef::default()
            });
        }
        if opens_dependency {
            self.pending = Some(Declared {
                line,
                ..Declared::default()
            });
        }
    }

    fn close(&mut self) {
        let managed = matches!(
            self.path().as_slice(),
            [PROJECT, MANAGEMENT, DEPENDENCIES, DEPENDENCY]
        );
        let direct = matches!(self.path().as_slice(), [PROJECT, DEPENDENCIES, DEPENDENCY]);
        self.stack.pop();
        if let Some(declared) = (managed || direct).then(|| self.pending.take()).flatten() {
            if managed {
                self.managed.push(declared);
            } else {
                self.dependencies.push(declared);
            }
        }
    }

    fn text(&mut self, value: &str) {
        let owned = value.to_owned();
        let stack: Vec<String> = self.stack.clone();
        let path: Vec<&str> = stack.iter().map(String::as_str).collect();
        match path.as_slice() {
            [PROJECT, PROPERTIES, name] => {
                self.properties.insert((*name).to_owned(), owned);
            }
            [PROJECT, VERSION] => self.version = Some(owned),
            [PROJECT, GROUP] => self.group = Some(owned),
            [PROJECT, ARTIFACT] => self.artifact = Some(owned),
            [PROJECT, PARENT, field] => {
                if let Some(parent) = self.parent.as_mut() {
                    parent.set(field, owned);
                }
            }
            [.., DEPENDENCY, field] => {
                if let Some(pending) = self.pending.as_mut() {
                    pending.set(field, owned);
                }
            }
            _ => {}
        }
    }
}

impl ParentRef {
    fn set(&mut self, field: &str, value: String) {
        match field {
            GROUP => self.group = value,
            ARTIFACT => self.artifact = value,
            VERSION => self.version = value,
            RELATIVE_PATH => self.relative_path = Some(value),
            _ => {}
        }
    }
}

impl Declared {
    fn set(&mut self, field: &str, value: String) {
        match field {
            GROUP => self.group = value,
            ARTIFACT => self.artifact = value,
            VERSION => self.version = Some(value),
            _ => {}
        }
    }
}

fn newlines(text: &str) -> usize {
    text.matches(NEWLINE).count()
}
