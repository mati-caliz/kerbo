use super::java_null_tests::findings_in_files;

const RULE: &str = "default-annotation-param";

const RETRY: &str = r"package app.retry;

public @interface Retry {
    int value() default 3;
    Mode mode() default Mode.FAST;
    Class<? extends Throwable> on() default Retry.None.class;
    String[] tags() default {};

    class None extends Throwable {}

    enum Mode { FAST, SLOW }
}
";

const RUNNER: &str = r"package app.runner;

public class Runner {
    public @interface Parameter {
        int value() default 0;
    }

    @Parameter(0)
    public int first;

    @Parameter(1)
    public int second;
}
";

const USES: &str = r#"package app;

import app.retry.Retry;
import app.retry.Retry.None;
import app.runner.Runner;
import app.runner.Runner.Parameter;

class Uses {
    @Retry(3)
    void same() {}

    @Retry(value = 4, on = None.class)
    void otherAttempts() {}

    @Retry(on = Retry.None.class, tags = {})
    void bothDefaults() {}

    @Runner.Parameter(0)
    public int nested;

    @Parameter(value = 2)
    public int other;

    @Retry(mode = Retry.Mode.FAST)
    void enumConstant() {}

    @Deprecated(since = "")
    void library() {}
}
"#;

#[test]
fn an_element_written_with_its_default_is_reported() {
    let mut found = findings_in_files(&[RETRY, RUNNER, USES], RULE);
    found.sort();
    assert_eq!(
        found,
        [
            "12:28 on",
            "15:17 on",
            "15:42 tags",
            "18:23 value",
            "8:16 value",
            "9:12 value",
        ]
    );
}

const AMBIGUOUS: &str = r"package app;

import other.*;
import app.retry.*;

class Ambiguous {
    @Retry(3)
    void unknown() {}
}
";

const OTHER_RETRY: &str = r"package other;

public @interface Retry {
    int value() default 3;
}
";

#[test]
fn an_annotation_that_resolves_to_two_declarations_is_not_reported() {
    assert_eq!(
        findings_in_files(&[RETRY, OTHER_RETRY, AMBIGUOUS], RULE),
        Vec::<String>::new()
    );
}
