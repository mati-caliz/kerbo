// Los métodos de librería con contrato escrito a mano en IntelliJ (`HardcodedContracts`): para sus
// parámetros no cuenta el `@NotNull` inferido del bytecode, el contrato ya dice qué pasa con un
// null (`requireNonNull(x)` no avisa aunque `x` pueda serlo).

const VALIDATIONS: &[&str] = &["isTrue", "state", "notNull"];
const BY_CLASS: &[(&str, &[&str])] = &[
    (
        "com.google.common.base.Preconditions",
        &["checkNotNull", "checkArgument", "checkState"],
    ),
    ("java.util.Objects", &["requireNonNull", "equals"]),
    (
        "com.google.common.base.Strings",
        &["emptyToNull", "isNullOrEmpty"],
    ),
    ("com.google.common.base.Objects", &["equal"]),
    (
        "java.util.Arrays",
        &[
            "fill",
            "parallelPrefix",
            "parallelSort",
            "sort",
            "spliterator",
            "stream",
            "binarySearch",
            "equals",
            "deepEquals",
        ],
    ),
    ("org.mockito.ArgumentMatchers", &["argThat", "assertArg"]),
    ("java.lang.System", &["arraycopy"]),
    (
        "java.lang.String",
        &[
            "charAt",
            "codePointAt",
            "substring",
            "subSequence",
            "isEmpty",
            "isBlank",
            "equalsIgnoreCase",
            "startsWith",
            "endsWith",
            "contains",
        ],
    ),
    ("org.springframework.util.CollectionUtils", &["isEmpty"]),
    ("org.apache.commons.lang.Validate", VALIDATIONS),
    ("org.apache.commons.lang3.Validate", VALIDATIONS),
    ("org.springframework.util.Assert", VALIDATIONS),
];
const TEST_FRAMEWORK_PACKAGES: &[&str] = &["junit.framework.", "org.junit.", "org.testng."];
const MATCHER_ASSERTS: &[&str] = &["org.hamcrest.MatcherAssert", "org.hamcrest.junit.MatcherAssert"];
const TEST_CONTRACTS: &[&str] = &[
    "fail",
    "assertInstanceOf",
    "assertTrue",
    "assumeTrue",
    "assertFalse",
    "assumeFalse",
    "assertNull",
    "assertNotNull",
];
const EQUALS: &str = "equals";
const EQUALS_ARITY: usize = 1;

pub(super) fn has_hardcoded_contract(class: &str, method: &str, arity: usize) -> bool {
    let in_test_framework = TEST_FRAMEWORK_PACKAGES
        .iter()
        .any(|package| class.starts_with(package))
        || MATCHER_ASSERTS.contains(&class);
    (method == EQUALS && arity == EQUALS_ARITY)
        || (in_test_framework && TEST_CONTRACTS.contains(&method))
        || BY_CLASS
            .iter()
            .any(|(owner, methods)| *owner == class && methods.contains(&method))
}
