use tree_sitter::Node;

use super::{ReferenceId, ScopeId, VariableId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionKind {
    CatchClause,
    ClassName,
    FunctionName,
    ImportBinding { type_only: bool },
    Parameter,
    EnumMember,
    EnumName,
    ModuleName,
    Type,
    Variable,
}

impl DefinitionKind {
    const fn is_type(self) -> bool {
        matches!(
            self,
            Self::ClassName
                | Self::ImportBinding { .. }
                | Self::EnumMember
                | Self::EnumName
                | Self::ModuleName
                | Self::Type
        )
    }

    const fn is_value(self) -> bool {
        !matches!(self, Self::Type)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Definition<'tree> {
    pub kind: DefinitionKind,
    pub name: Node<'tree>,
    pub node: Node<'tree>,
    pub declaration: Option<Node<'tree>>,
}

#[derive(Debug)]
pub struct Variable<'tree> {
    pub name: String,
    pub scope: ScopeId,
    pub definitions: Vec<Definition<'tree>>,
    pub references: Vec<ReferenceId>,
    pub used: bool,
}

impl Variable<'_> {
    pub fn is_type_variable(&self) -> bool {
        self.definitions.is_empty()
            || self
                .definitions
                .iter()
                .any(|definition| definition.kind.is_type())
    }

    pub fn is_value_variable(&self) -> bool {
        self.definitions.is_empty()
            || self
                .definitions
                .iter()
                .any(|definition| definition.kind.is_value())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Value,
    Type,
    Both,
}

#[derive(Debug, Clone, Copy)]
pub struct Reference<'tree> {
    pub identifier: Node<'tree>,
    pub from: ScopeId,
    pub access: Access,
    pub target: Target,
    pub init: bool,
    pub resolved: Option<VariableId>,
}

impl Reference<'_> {
    pub const fn is_read(&self) -> bool {
        matches!(self.access, Access::Read | Access::ReadWrite)
    }

    pub const fn is_write(&self) -> bool {
        matches!(self.access, Access::Write | Access::ReadWrite)
    }

    pub const fn is_value(&self) -> bool {
        matches!(self.target, Target::Value | Target::Both)
    }

    pub const fn is_type(&self) -> bool {
        matches!(self.target, Target::Type | Target::Both)
    }
}
