#!/usr/bin/env python
r"""Genera src/lint/java_unicode_escapes/kept.rs: los caracteres del plano básico cuyo escape \uXXXX
deja en paz UnnecessaryUnicodeEscape de IntelliJ (controles salvo salto de línea y tab, espacios que
no son el común, formato, uso privado, sustitutos, sin asignar, separadores de línea y párrafo, las
marcas combinantes y U+FFFD). Las categorías salen del Character del JBR de la imagen de Qodana de
versions.toml (25, Unicode 16): con otro Java cambian los caracteres sin asignar. Corre Docker, así
que va por la cola de procesos pesados.

    python scripts/parity/java/unicode_escape_table.py
"""

import os
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[3]
OUTPUT = ROOT / "src" / "lint" / "java_unicode_escapes" / "kept.rs"
VERSIONS = ROOT / "scripts" / "parity" / "versions.py"
QODANA_JAVA = "/opt/idea/jbr/bin/java"
BOUNDS_PER_LINE = 12
GENERATOR = r"""
public class KeptEscapes {
    static boolean kept(char d) {
        if (d == 0xFFFD) return true;
        int type = Character.getType(d);
        if (type == Character.CONTROL) return d != 0x0A && d != 0x09;
        if (type == Character.SPACE_SEPARATOR) return d != 0x20;
        if (type == Character.FORMAT || type == Character.PRIVATE_USE || type == Character.SURROGATE
            || type == Character.UNASSIGNED || type == Character.LINE_SEPARATOR
            || type == Character.PARAGRAPH_SEPARATOR) return true;
        Character.UnicodeBlock block = Character.UnicodeBlock.of(d);
        return block == Character.UnicodeBlock.COMBINING_DIACRITICAL_MARKS
            || block == Character.UnicodeBlock.COMBINING_DIACRITICAL_MARKS_EXTENDED
            || block == Character.UnicodeBlock.COMBINING_DIACRITICAL_MARKS_SUPPLEMENT
            || block == Character.UnicodeBlock.COMBINING_HALF_MARKS
            || block == Character.UnicodeBlock.COMBINING_MARKS_FOR_SYMBOLS;
    }

    public static void main(String[] arguments) {
        StringBuilder out = new StringBuilder();
        int start = -1;
        for (int code = 0; code <= 0x10000; code++) {
            boolean inside = code <= 0xFFFF && kept((char) code);
            if (inside && start < 0) start = code;
            if (!inside && start >= 0) {
                out.append(start).append(' ').append(code - 1).append(' ');
                start = -1;
            }
        }
        System.out.print(out);
    }
}
"""


def main():
    image = subprocess.run([sys.executable, str(VERSIONS), "image", "qodana"], capture_output=True, text=True, check=True).stdout.strip()
    with tempfile.TemporaryDirectory() as folder:
        source = pathlib.Path(folder) / "KeptEscapes.java"
        source.write_text(GENERATOR, encoding="utf-8")
        mounted = folder.replace("\\", "/")
        command = ["docker", "run", "--rm", "--cpus=1", "-m", "1g", "-v", f"{mounted}:/work", "--entrypoint", QODANA_JAVA, image, "/work/KeptEscapes.java"]
        environment = {**os.environ, "MSYS_NO_PATHCONV": "1"}
        ranges = subprocess.run(command, capture_output=True, text=True, check=True, env=environment).stdout.split()
    bounds = [f"0x{int(bound):04X}" for bound in ranges]
    lines = [", ".join(bounds[index:index + BOUNDS_PER_LINE]) + "," for index in range(0, len(bounds), BOUNDS_PER_LINE)]
    body = "\n".join(f"    {line}" for line in lines)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(
        f"pub const KEPT_RANGES: [u16; {len(bounds)}] = [\n{body}\n];\n",
        encoding="utf-8",
        newline="\n",
    )
    print(f"{len(bounds) // 2} rangos en {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
