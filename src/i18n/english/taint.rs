use crate::i18n::catalog::RuleText;
use crate::rules::TaintRule;

const UNKNOWN: RuleText = RuleText::new(
    "User data reaching a dangerous place unsanitized",
    "sanitize it or pass it as a parameter before using it",
    "Something the user sent reaches a query, a command, a path or the response unsanitized.",
    "See the rule explanation.",
);
const TEXTS: [RuleText; TaintRule::ALL.len()] = [
    RuleText::new(
        "SQL query built from user data",
        "pass the value as a parameter (? with setString, or JdbcTemplate arguments)",
        "Something the user sent (a parameter, a header, a cookie, a @RequestParam) reaches the text of a query unsanitized: with a quote they can change it and read or delete other people's data. Like FindSecBugs' and Semgrep's injection rules.",
        "statement.executeQuery(\"SELECT * FROM users WHERE name = '\" + name + \"'\") becomes prepareStatement(\"SELECT * FROM users WHERE name = ?\") with setString(1, name).",
    ),
    RuleText::new(
        "System command built from user data",
        "do not pass user data to the command; if needed, check it against a closed list",
        "Something the user sent reaches Runtime.exec or a ProcessBuilder unsanitized: with a ; or a && they can run any command on the server. Like FindSecBugs' COMMAND_INJECTION.",
        "Runtime.getRuntime().exec(\"ping \" + host) becomes a ProcessBuilder with host checked against the allowed values.",
    ),
    RuleText::new(
        "File path built from user data",
        "resolve the path against a fixed directory and check it does not leave it",
        "Something the user sent reaches a File, a FileInputStream or Paths.get unsanitized: with ../ they can read or overwrite any file the process can see. Like FindSecBugs' PATH_TRAVERSAL_IN.",
        "new File(base, name) becomes base.resolve(name).normalize() with a check that it starts with base.",
    ),
    RuleText::new(
        "HTML response built from user data",
        "escape it before writing it (HtmlUtils.htmlEscape, Encode.forHtml or your template's escaping)",
        "Something the user sent reaches what is written to the response unescaped: with a <script> they can run code in another user's browser and steal their session. Like FindSecBugs' XSS_SERVLET.",
        "response.getWriter().print(\"Hi \" + name) becomes response.getWriter().print(\"Hi \" + HtmlUtils.htmlEscape(name)).",
    ),
];

pub fn taint(rule: TaintRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
