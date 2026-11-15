use super::tests::{lints, strict};
use crate::limits::{JavaSettings, Limits};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;

class Io {
    String read(Path path, Charset charset) throws Exception {
        String first = new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
        String second = new String(Files.readAllBytes(path), charset);
        String third = new String(Files.readAllBytes(path), \"UTF-8\");
        String fourth = new String(Files.readAllBytes(path));
        return first + second + third + fourth;
    }

    void write(Path path, String text, byte[] raw) throws Exception {
        Files.write(path, text.getBytes(StandardCharsets.UTF_8));
        byte[] bytes = text.getBytes(StandardCharsets.UTF_8);
        Files.write(path, bytes, StandardOpenOption.APPEND);
        Files.write(path, raw);
        Files.write(path, text.getBytes());
    }
}
";

fn reported(limits: &Limits) -> Vec<usize> {
    lints(SOURCE, limits)
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::ReadWriteString))
        .map(|finding| finding.line)
        .collect()
}

#[test]
fn reports_bytes_turned_into_a_string_with_a_charset() {
    assert_eq!(reported(&strict()), [9, 10, 17, 19]);
}

#[test]
fn reading_needs_eleven_and_writing_twelve() {
    let with = |release| {
        strict().with_java(JavaSettings {
            release: Some(release),
            ..JavaSettings::default()
        })
    };
    assert_eq!(reported(&with(11)), [9, 10]);
    assert_eq!(reported(&with(10)), Vec::<usize>::new());
}
