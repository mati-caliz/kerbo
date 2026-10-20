# Cuenta los tokens de las salidas de kerbo con tokenizadores locales: o200k_base (GPT-4o y
# posteriores) y cl100k_base (GPT-4) de tiktoken, y el tokenizador público de Claude previo a
# Claude 3 (Xenova/claude-tokenizer, el único de Anthropic que se puede correr local). Con eso se
# eligió el formato de `--format agent`.
#
#   uv run --no-project --with tiktoken --with tokenizers python -I scripts/count-tokens.py salida.txt...
import sys
from pathlib import Path

import tiktoken
from tokenizers import Tokenizer

CLAUDE_TOKENIZER = "Xenova/claude-tokenizer"
TIKTOKEN_ENCODINGS = ["o200k_base", "cl100k_base"]


def encoders():
    found = {name: tiktoken.get_encoding(name).encode for name in TIKTOKEN_ENCODINGS}
    claude = Tokenizer.from_pretrained(CLAUDE_TOKENIZER)
    found["claude"] = lambda text: claude.encode(text).ids
    return found


def main(paths):
    counters = encoders()
    for path in paths:
        text = Path(path).read_text(encoding="utf-8")
        counts = " ".join(f"{name}={len(encode(text))}" for name, encode in counters.items())
        print(f"{path}: bytes={len(text.encode())} {counts}")


if __name__ == "__main__":
    main(sys.argv[1:])
