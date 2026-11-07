"""Copia una gramática de .parity/grammar/ts-<nombre> a grammars/tree-sitter-<nombre> de kerbo,
con su Cargo.toml propio.

Uso: python vendor_grammar.py python 0.25.0 "descripción"
"""
import os
import shutil
import sys

name, version, description = sys.argv[1:4]
repository = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
parity = os.path.join(repository, ".parity")
source = os.path.join(parity, "grammar", f"ts-{name}")
target = os.path.join(repository, "grammars", f"tree-sitter-{name}")
os.makedirs(target, exist_ok=True)
for entry in ("LICENSE", "grammar.js", "tree-sitter.json"):
    shutil.copy(os.path.join(source, entry), os.path.join(target, entry))
for folder in ("queries", "src"):
    shutil.rmtree(os.path.join(target, folder), ignore_errors=True)
    shutil.copytree(os.path.join(source, folder), os.path.join(target, folder))
os.makedirs(os.path.join(target, "bindings", "rust"), exist_ok=True)
for entry in ("lib.rs", "build.rs"):
    shutil.copy(os.path.join(source, "bindings", "rust", entry), os.path.join(target, "bindings", "rust", entry))
manifest = f"""[package]
edition = "2021"
name = "kerbo-tree-sitter-{name}"
version = "{version}"
authors = [
    "Max Brunsfeld <maxbrunsfeld@gmail.com>",
    "Amaan Qureshi <amaanq12@gmail.com>",
]
build = "bindings/rust/build.rs"
description = "{description}"
license = "MIT"
repository = "https://github.com/mati-caliz/kerbo"
readme = "README.md"
include = ["LICENSE", "README.md", "bindings/rust/*", "grammar.js", "queries/*", "src/*", "src/tree_sitter/*", "tree-sitter.json"]

[lib]
name = "tree_sitter_{name}"
path = "bindings/rust/lib.rs"

[dependencies.tree-sitter-language]
version = "0.1"

[build-dependencies.cc]
version = "1.2"
"""
with open(os.path.join(target, "Cargo.toml"), "w", encoding="utf-8", newline="\n") as handle:
    handle.write(manifest)
