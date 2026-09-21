from typing import Any, Literal, TypeAlias, TypedDict

from typing_extensions import NotRequired, Unpack

TransformProfile = Literal["authoring", "faithful", "corpus", "equiv"]
"""Normalization profile name: ``"authoring"``, ``"faithful"``, ``"corpus"``, or ``"equiv"``."""
RuntimeContentMode = Literal["math", "text"]
"""Runtime content mode reported by the tree API: ``"math"`` or ``"text"``."""
ParseDiagnosticKind = Literal[
    "ambiguous-infix",
    "argument-validation",
    "command-mode-error",
    "comment-truncated-argument",
    "environment-mode-error",
    "environment-name-mismatch",
    "left-right-delimiter",
    "max-group-depth-exceeded",
    "raw-expected-found",
    "text-script-error",
    "unclosed-inline-math",
    "unexpected-math-shift",
    "unknown-command",
    "unknown-environment",
]
"""Category of a parse diagnostic, such as ``"unknown-command"`` or ``"argument-validation"``."""
NodeKind = Literal[
    "Root",
    "Group",
    "Command",
    "Infix",
    "Declarative",
    "Environment",
    "Scripted",
    "Prime",
    "Text",
    "Char",
    "ActiveSpace",
    "Error",
]
"""Node kind as reported by ``Node.kind()``. Python uses PascalCase (``"Root"``, ``"Command"``, ``"ActiveSpace"``)."""


class CommandItem(TypedDict):
    """A custom command injected into parser knowledge.

    Attributes:
        target: Discriminator; always ``"command"``.
        name: Command name without the leading backslash.
        kind: Syntactic role: ``"prefix"``, ``"infix"``, or ``"declarative"``.
        allowed_mode: Modes in which the command is valid: ``"math"``, ``"text"``,
            or ``"both"``.
        argspec: xparse-style argument specification, such as ``"m m"`` or
            ``"o m"``.
        tags: Optional free-form classification tags carried into the knowledge
            entry.
    """

    target: Literal["command"]
    name: str
    kind: Literal["prefix", "infix", "declarative"]
    allowed_mode: Literal["math", "text", "both"]
    argspec: str
    tags: NotRequired[list[str]]


class EnvironmentItem(TypedDict):
    """A custom environment injected into parser knowledge.

    Attributes:
        target: Discriminator; always ``"environment"``.
        name: Environment name as written in ``\\begin{...}``.
        allowed_mode: Modes in which the environment may appear.
        body_mode: Content mode of the environment body (``"math"`` or ``"text"``).
        argspec: xparse-style argument specification for the environment header.
        tags: Optional free-form classification tags.
    """

    target: Literal["environment"]
    name: str
    allowed_mode: Literal["math", "text", "both"]
    body_mode: Literal["math", "text"]
    argspec: str
    tags: NotRequired[list[str]]


class DelimiterItem(TypedDict):
    """A custom delimiter control injected into parser knowledge.

    Attributes:
        target: Discriminator; always ``"delimiter"``.
        name: Delimiter-control name without the leading backslash (for example
            ``"langle"``).
    """

    target: Literal["delimiter"]
    name: str


ContextItem: TypeAlias = CommandItem | EnvironmentItem | DelimiterItem
"""A knowledge-injection item: a command, environment, or delimiter-control dict."""
SyntaxNode: TypeAlias = dict[str, Any]
"""Lossless parse snapshot as a plain dict. The tagged-union shape is documented on the TypeScript ``SyntaxNode`` type; Python keeps this alias unexpanded."""
Span: TypeAlias = dict[str, int]
"""Half-open UTF-8 byte range ``{"start": int, "end": int}`` into the source."""
SerializationTokenKind = Literal[
    "control_sequence", "character", "delimiter", "text", "raw", "error"
]
"""Semantic category of one canonical serialization fragment."""
ArgSpecKindType = Literal[
    "content",
    "operatorname",
    "delimiter",
    "csname",
    "dimension",
    "integer",
    "keyval",
    "column",
    "star",
]
"""Value kind of one parsed argspec slot (content, delimiter, star, ...)."""
ArgSpecFormType = Literal["standard", "star", "group", "delimited", "paired"]
"""Surface form of one parsed argspec slot (mandatory, optional group, paired, ...)."""


class ParseDiagnostic(TypedDict, total=False):
    """A single parse diagnostic emitted alongside a parse result.

    Diagnostics are a channel separate from ``Error`` placeholder nodes: a
    diagnostic describes something the parser noticed, while an ``Error`` node is
    a recovered fragment in the tree. A tree that ``has_errors()`` always carries
    diagnostics, but the converse does not hold. All fields are optional.

    Attributes:
        kind: The diagnostic category, one of the ``ParseDiagnosticKind`` values
            such as ``"unknown-command"`` or ``"argument-validation"``.
        message: A human-readable description of the problem.
        span: A ``{"start": int, "end": int}`` byte range into the source.
        expected: The tokens that would have been accepted (may be empty).
        found: The token actually found, or absent when not applicable.
        contexts: Nested context dicts describing where the diagnostic arose (may
            be empty).
    """

    kind: ParseDiagnosticKind
    message: str
    span: Span
    expected: list[str]
    found: str
    contexts: list[dict[str, Any]]


class ParseResult(TypedDict):
    """The result of ``Parser.parse``: an optional document plus diagnostics.

    There are exactly three honest states, distinguished by ``document`` and the
    document's ``has_errors()``: a hard failure (``document`` is ``None``), a
    clean parse (a ``Document`` whose ``has_errors()`` is ``False``), and a
    partial parse (a read-only ``Document`` whose ``has_errors()`` is ``True``).
    The parser never fabricates a placeholder tree, and empty input is a clean
    parse rather than ``None``.

    Attributes:
        document: The parsed ``Document``, or ``None`` on a hard failure.
        diagnostics: The diagnostics describing what the parser noticed.

    See Also:
        Parser, Document, ParseDiagnostic
    """

    document: Document | None
    diagnostics: list[ParseDiagnostic]


class NodeSpanEntry(TypedDict):
    """One entry mapping a node identifier to its source byte span.

    Attributes:
        id: The node identifier within the document.
        span: A ``{"start": int, "end": int}`` byte range into the source.
    """

    id: str
    span: Span


class SerializationToken(TypedDict):
    """One canonical serialization fragment with UTF-8 byte offsets.

    ``start_byte`` / ``end_byte`` index the accompanying ``TokenizedLatex.latex``
    string and are UTF-8 byte offsets, not Python code-point indices. Spans do
    not overlap. Empty error snippets emit no token.

    Attributes:
        text: The UTF-8 decoding of the byte slice
            ``latex.encode("utf-8")[start_byte:end_byte]``.
        start_byte: Inclusive UTF-8 byte offset into the canonical LaTeX string.
        end_byte: Exclusive UTF-8 byte offset into the canonical LaTeX string.
        kind: Semantic category of the fragment (control sequence, character, ...).
        mode: Semantic math/text mode of the fragment, independent of spacing
            policy.
    """

    text: str
    start_byte: int
    end_byte: int
    kind: SerializationTokenKind
    mode: RuntimeContentMode


class TokenizedLatex(TypedDict):
    """Canonical LaTeX and tokens produced by the same serializer traversal.

    Text and tokenized output share one traversal and spacing policy; this is
    not a second lexer pass.

    Attributes:
        latex: The canonical LaTeX string, identical to ``Document.to_latex()``.
        tokens: Ordered, non-overlapping, non-empty fragments. Gaps between
            spans contain serializer-inserted spacing.
    """

    latex: str
    tokens: list[SerializationToken]


class PackageInfo(TypedDict):
    """Summary of one built-in knowledge package.

    Attributes:
        name: The package identifier accepted by the ``packages`` argument of
            ``Parser`` and ``TransformEngine`` (such as ``"base"`` or ``"ams"``).
        commands: The number of command records in the package.
        environments: The number of environment records in the package.
    """

    name: str
    commands: int
    environments: int


class ErrorParts(TypedDict):
    """The decomposed content of an ``Error`` placeholder node.

    Attributes:
        message: The reason the fragment failed to parse.
        snippet: The captured source text, re-emitted verbatim on serialization.
    """

    message: str
    snippet: str


class FinalizeAstStepReport(TypedDict):
    """The per-step report for one FinalizeAst cleanup step.

    Attributes:
        applied_count: The number of times the step changed the tree.
    """

    applied_count: int


class FinalizeAstStepReports(TypedDict):
    """The set of FinalizeAst step reports.

    Attributes:
        merge_adjacent_primes: The report for merging adjacent ``Prime`` nodes
            produced by rewrite rules.
        normalize_text_sequences: The report for text-sequence merge, ordinary
            whitespace collapse, and empty-text cleanup.
    """

    merge_adjacent_primes: FinalizeAstStepReport
    normalize_text_sequences: FinalizeAstStepReport


class FinalizeAstReport(TypedDict):
    """The FinalizeAst phase report.

    Attributes:
        steps: The per-step reports for this phase.
    """

    steps: FinalizeAstStepReports


class TransformReport(TypedDict):
    """The phase-oriented report of a transform run.

    The fields mirror the engine's phases. The Python report keeps snake_case
    field names; the JavaScript binding exposes the same data with camelCase keys.

    Attributes:
        iterations: The number of fixed-point passes the Rewrite phase ran.
        rules: Per-rule entries, each a dict with ``key``, ``applied_count``, and
            ``skipped_count``.
        finalize_ast: The FinalizeAst phase report.
        flatten_groups: The FlattenGroups phase report, with an ``actions`` dict
            and a ``guards`` dict counting each preserve guard that fired.
        lower_attributes: The LowerAttributes phase report, with an ``attributes``
            list and ``eliminated_empty_segments``.

    See Also:
        TransformEngine, TransformResult
    """

    iterations: int
    rules: list[dict[str, Any]]
    finalize_ast: FinalizeAstReport
    flatten_groups: dict[str, Any]
    lower_attributes: dict[str, Any]


class TransformResult(TypedDict):
    """The result of ``TransformEngine.normalize``: the output plus its report.

    Attributes:
        normalized: The canonical LaTeX string.
        report: The phase-oriented transform report.

    See Also:
        TransformReport
    """

    normalized: str
    report: TransformReport


class ParsedArgSpecSlot(TypedDict):
    """One parsed argument slot of an xparse-style specification.

    A presentation-oriented view of the internal argspec model, so tooling can
    consume argspecs without depending on internal crates.

    Attributes:
        required: Whether the argument is mandatory (``True``) or optional.
        no_leading_space: Whether leading whitespace before the argument is
            disallowed.
        nullable: Whether an absent optional argument yields a null value.
        kind: The value kind the slot accepts; a dict with a ``type`` field, one
            of the ``ArgSpecKindType`` values. A ``content`` kind also carries a
            ``mode`` of ``"math"`` or ``"text"``.
        form: The syntactic form the slot takes; a dict with a ``type`` field, one
            of the ``ArgSpecFormType`` values. A ``paired`` form carries a
            ``pairs`` list of open/close delimiter descriptors.

    See Also:
        ValidateArgspecResult
    """

    required: bool
    no_leading_space: bool
    nullable: bool
    kind: dict[str, Any]
    form: dict[str, Any]


class ValidateArgspecResult(TypedDict):
    """The outcome of validating an argspec string with ``validate_argspec``.

    On success, ``valid`` is ``True`` and ``arg_count`` / ``parsed`` describe the
    slots; on failure, ``valid`` is ``False`` and ``error`` carries the reason
    while ``arg_count`` and ``parsed`` are ``None``.

    Attributes:
        valid: Whether the specification string parsed.
        error: The error message when invalid, otherwise ``None``.
        arg_count: The number of argument slots when valid, otherwise ``None``.
        parsed: The per-slot breakdown when valid, otherwise ``None``.

    See Also:
        validate_argspec, ParsedArgSpecSlot
    """

    valid: bool
    error: str | None
    arg_count: int | None
    parsed: list[ParsedArgSpecSlot] | None


class DelimiterNone(TypedDict):
    """A delimiter value meaning "no delimiter" (e.g. an open `.` in `\\left.`).

    Attributes:
        kind: Discriminator; always ``"None"``.
    """

    kind: Literal["None"]


class DelimiterChar(TypedDict):
    """A delimiter that is a single literal character, such as ``(`` or ``|``.

    Attributes:
        kind: Discriminator; always ``"Char"``.
        value: The literal delimiter character.
    """

    kind: Literal["Char"]
    value: str


class DelimiterControl(TypedDict):
    """A delimiter that is a control sequence, such as ``langle`` or ``lvert``.

    Attributes:
        kind: Discriminator; always ``"Control"``.
        value: Control-sequence name without the leading backslash.
    """

    kind: Literal["Control"]
    value: str


DelimiterValue: TypeAlias = DelimiterNone | DelimiterChar | DelimiterControl
"""A delimiter value: none, a literal character, or a control sequence."""


class GroupKindExplicit(TypedDict):
    """A group written with explicit braces ``{ ... }``.

    Attributes:
        kind: Discriminator; always ``"Explicit"``.
    """

    kind: Literal["Explicit"]


class GroupKindImplicit(TypedDict):
    """A group with no surrounding braces, inferred from structure.

    Attributes:
        kind: Discriminator; always ``"Implicit"``.
    """

    kind: Literal["Implicit"]


class GroupKindDelimited(TypedDict):
    """A group bounded by a delimiter pair, such as ``\\left( ... \\right)``.

    Attributes:
        kind: Discriminator; always ``"Delimited"``.
        left: The opening delimiter value.
        right: The closing delimiter value.
    """

    kind: Literal["Delimited"]
    left: DelimiterValue
    right: DelimiterValue


class GroupKindInlineMath(TypedDict):
    """A group introduced by inline math shift, such as ``$ ... $`` in text mode.

    Attributes:
        kind: Discriminator; always ``"InlineMath"``.
    """

    kind: Literal["InlineMath"]


GroupKindRef: TypeAlias = (
    GroupKindExplicit | GroupKindImplicit | GroupKindDelimited | GroupKindInlineMath
)
"""The kind of a ``Group`` node: explicit, implicit, delimited, or inline-math."""


class MathArg(TypedDict):
    """A command argument carrying math-mode content.

    Attributes:
        kind: Discriminator; always ``"Math"``.
        node: The live argument-body ``Node``.
    """

    kind: Literal["Math"]
    node: Node


class TextArg(TypedDict):
    """A command argument carrying text-mode content.

    Attributes:
        kind: Discriminator; always ``"Text"``.
        node: The live argument-body ``Node``.
    """

    kind: Literal["Text"]
    node: Node


class DelimiterArg(TypedDict):
    """A command argument that is a single delimiter token.

    Attributes:
        kind: Discriminator; always ``"Delimiter"``.
        value: The delimiter value.
    """

    kind: Literal["Delimiter"]
    value: DelimiterValue


class CSNameArg(TypedDict):
    """A command argument that is a control-sequence name.

    Attributes:
        kind: Discriminator; always ``"CSName"``.
        value: The control-sequence name without the leading backslash.
    """

    kind: Literal["CSName"]
    value: str


class DimensionArg(TypedDict):
    """A command argument that is a TeX dimension, such as ``2pt``.

    Attributes:
        kind: Discriminator; always ``"Dimension"``.
        value: The dimension as written.
    """

    kind: Literal["Dimension"]
    value: str


class IntegerArg(TypedDict):
    """A command argument that is an integer literal.

    Attributes:
        kind: Discriminator; always ``"Integer"``.
        value: The integer as written.
    """

    kind: Literal["Integer"]
    value: str


class KeyValArg(TypedDict):
    """A command argument that is a ``key=value`` list.

    Attributes:
        kind: Discriminator; always ``"KeyVal"``.
        value: The raw key-value text.
    """

    kind: Literal["KeyVal"]
    value: str


class ColumnArg(TypedDict):
    """A command argument that is a tabular column specification.

    Attributes:
        kind: Discriminator; always ``"Column"``.
        value: The column specification as written.
    """

    kind: Literal["Column"]
    value: str


class BooleanArg(TypedDict):
    """A command argument that is a star flag, modeled as a boolean.

    Attributes:
        kind: Discriminator; always ``"Boolean"``.
        value: ``True`` when the star was present.
    """

    kind: Literal["Boolean"]
    value: bool


ArgRef: TypeAlias = (
    MathArg
    | TextArg
    | DelimiterArg
    | CSNameArg
    | DimensionArg
    | IntegerArg
    | KeyValArg
    | ColumnArg
    | BooleanArg
)
"""A read command argument: one of the discriminated argument-value dicts."""
ArgValueInput: TypeAlias = ArgRef
"""An argument-value dict accepted when creating or setting command arguments."""

__all__ = [
    "Document",
    "ConfigError",
    "EditError",
    "FinalizeAstConfig",
    "FlattenGroupsConfig",
    "LowerAttributesConfig",
    "Node",
    "ParseConfig",
    "ParseError",
    "Parser",
    "RewriteConfig",
    "TexformError",
    "TransformConfig",
    "TransformEngine",
    "TransformError",
    "count_targets",
    "list_packages",
    "serialize",
    "validate_argspec",
]


class TexformError(Exception):
    """Base class for every exception the library raises.

    Catching ``TexformError`` catches all of them. Note that ``Parser.parse``
    does not raise on malformed input; it reports failures through its result
    dict instead.

    See Also:
        ParseError, EditError, ConfigError, TransformError
    """


class ParseError(TexformError):
    """Raised when an operation required a complete tree but parsing produced none.

    ``TransformEngine.normalize`` raises this on input that cannot produce a
    complete tree, and ``count_targets`` raises it when the source does not parse
    into a complete tree. The exception then carries the diagnostics and the
    partial document for inspection.

    Attributes:
        diagnostics: The diagnostics describing the parse failure.
        document: The partial ``Document``, or ``None`` when no tree was produced.
    """

    diagnostics: list[ParseDiagnostic]
    document: Document | None


class EditError(TexformError):
    """Raised by a ``Document`` editing method on an invalid edit.

    Triggers include editing a read-only (error) tree, detaching or removing the
    root node, an out-of-bounds index, a wrong container shape, and attaching a
    node that belongs to another document. The edit is rejected before it can
    corrupt the tree.
    """


class ConfigError(TexformError):
    """Raised on invalid construction or per-call configuration input.

    Triggers include an unknown knowledge package name or transform profile, a
    ``config`` argument that is not the matching complete-config class (including
    a dict passed as ``config``), unknown override keys, wrong scalar types
    (string or int instead of bool, float instead of int), a list where an object
    is expected, and a config class instance used as a nested override value.
    """


class TransformError(TexformError):
    """Raised on a transform-engine failure.

    Triggers include an eliminated-form contract violation, passing
    ``TransformEngine.transform`` a document that ``has_errors()``, and — in the
    Python build — passing a foreign document not produced by that engine's own
    ``parse``.
    """


class Document:
    """The editable document tree — the working format you read, edit, and output.

    A ``Document`` wraps the internal arena tree behind a fallible, DOM-style
    surface, so no internal panic ever reaches the caller. Reads go through
    read-only ``Node`` handles; edits are validated eagerly and raise on misuse.
    The tree is always structurally valid, even when it contains ``Error`` nodes.

    A tree that ``has_errors()`` is read-only, fixed that way at construction:
    every editing method then raises ``EditError``. The only use for an error
    tree is inspection.

    Construct one by parsing (``Parser.parse``), from scratch (``Document()``),
    or from a snapshot (``Document.from_syntax``). For the conceptual model, see
    the Parsing guide.

    Examples:
        import texform

        doc = texform.Parser().parse(r"\\frac{x}{y}")["document"]
        doc.to_latex()  # '\\frac { x } { y }'

    See Also:
        Node, Parser, ParseResult, Parsing
    """

    def __init__(self) -> None:
        """Construct an empty document holding a single empty root.

        This is a complete tree: ``has_errors()`` is ``False`` and it is fully
        editable. It serializes to the empty string.

        Examples:
            doc = texform.Document()
            doc.to_latex()  # ''
        """

    @staticmethod
    def from_syntax(node: SyntaxNode) -> Document:
        """Build a document from a ``SyntaxNode`` dict, the lossless parse snapshot.

        ``from_syntax`` and ``to_syntax`` are symmetric over every node kind,
        including ``Error`` and ``Prime``. Invalid external syntax is rejected
        rather than corrupting the tree. A document built this way is not produced
        by an engine's parser, so ``TransformEngine.transform`` rejects it.

        Args:
            node: A ``SyntaxNode`` dict, typically produced by ``to_syntax()``.

        Returns:
            A new ``Document`` wrapping the snapshot.

        Examples:
            result = texform.Parser().parse(r"\\frac{x}{y}")
            document = result["document"]
            assert document is not None
            syntax = document.to_syntax()
            doc = texform.Document.from_syntax(syntax)

        See Also:
            Document.to_syntax
        """

    def root(self) -> Node:
        """Return the root ``Node`` of the tree.

        Returns:
            The ``Root`` node handle.

        Examples:
            doc.root().kind()  # 'Root'
        """

    def has_errors(self) -> bool:
        """Report whether the tree contains any ``Error`` placeholder node.

        This is a cheap O(1) signal, separate from structural validity and
        independent of the ``abort_on_error`` parse setting. When ``True``, the
        document is read-only.

        Returns:
            ``True`` if the tree holds at least one ``Error`` node.

        Examples:
            result = texform.Parser().parse(r"\\sqrt[")
            document = result["document"]
            assert document is not None
            document.has_errors()  # True

        See Also:
            Document.errors, Document.is_read_only
        """

    def is_read_only(self) -> bool:
        """Report whether the document is read-only.

        A tree that ``has_errors()`` is read-only and fixed that way at
        construction; its error count cannot change, so its read-only-ness cannot
        either.

        Returns:
            ``True`` if no editing method will succeed on this document.
        """

    def errors(self) -> list[Node]:
        """Return the ``Error`` placeholder nodes in the tree.

        Returns:
            The list of ``Error`` node handles, empty when ``has_errors()`` is
            ``False``.
        """

    def find_commands(self, name: str) -> list[Node]:
        """Return every command node whose name matches.

        Args:
            name: The command name without the leading backslash.

        Returns:
            The matching ``Command`` node handles, in document order.

        Examples:
            doc = texform.Parser().parse(r"\\frac{x}{y} + \\frac{a}{b}")["document"]
            len(doc.find_commands("frac"))  # 2
        """

    def find_environments(self, name: str) -> list[Node]:
        """Return every environment node whose name matches.

        Args:
            name: The environment name.

        Returns:
            The matching ``Environment`` node handles, in document order.
        """

    def to_syntax(self) -> SyntaxNode:
        """Convert the tree to a ``SyntaxNode`` dict for serde and transport.

        This is the structured-data channel, distinct from the text channel
        ``to_latex()``. The conversion is symmetric with ``from_syntax``.

        Returns:
            A ``SyntaxNode`` dict, the single serde DTO for the tree.

        Examples:
            doc.to_syntax()  # {'Root': {...}}

        See Also:
            Document.from_syntax, Document.to_latex
        """

    def node_spans(self) -> list[NodeSpanEntry]:
        """Return source byte spans for the nodes that carry them.

        Returns:
            A list of ``{"id": str, "span": {...}}`` entries mapping node
            identifiers to source byte ranges.
        """

    def to_latex(self, **options: Unpack[SerializeOptions]) -> str:
        """Serialize the tree back to LaTeX text using the canonical serializer.

        ``Error`` nodes round-trip their captured source snippet verbatim, so a
        partial tree round-trips losslessly. The serializer guarantees text
        idempotency: re-parsing and re-serializing canonical output yields the
        same string. There is intentionally no method named ``serialize`` on
        ``Document``; this is the text channel and ``to_syntax()`` is the data
        channel.

        Args:
            **options: Serializer style overlays. Omitted keys and explicit
                ``None`` keep the default (spaced) style. See
                ``SerializeOptions`` for the full option axes.

        Returns:
            The serialized LaTeX string.

        Raises:
            ConfigError: If an option key is unknown or a value has the wrong
                type.
            TypeError: If options are passed as a positional dict instead of
                keyword arguments. Use ``doc.to_latex(**options)`` for a dict.

        Examples:
            doc = texform.Parser().parse(r"x^2")["document"]
            doc.to_latex()                                              # 'x ^ { 2 }'
            doc.to_latex(script_spacing="compact")                           # 'x^{ 2 }'

        See Also:
            SerializeOptions, serialize, Document.to_syntax
        """

    def to_tokenized_latex(self, **options: Unpack[SerializeOptions]) -> TokenizedLatex:
        """Serialize canonical LaTeX together with typed output tokens.

        ``start_byte`` and ``end_byte`` are UTF-8 byte offsets, not Python
        string indices. Empty error snippets produce no zero-width token; use
        ``has_errors()`` to detect whether the document contains error nodes.

        Args:
            options: Serializer style overlays. Same axes as ``to_latex``.
                Omitted keys and explicit ``None`` keep the default (spaced)
                style.

        Returns:
            A ``TokenizedLatex`` dict with ``latex`` and ``tokens``.

        Raises:
            ConfigError: If an option key is unknown or a value has the wrong
                type.

        Examples:
            tok = texform.Parser().parse(r"x^2")["document"].to_tokenized_latex()
            tok["latex"]  # 'x ^ { 2 }'
            tok["tokens"][0]["kind"]  # 'character'
        """

    def create_char(self, value: str) -> Node:
        """Stage a detached ``Char`` node owned by this document.

        Staged nodes are not in the tree until attached with an edit method.

        Args:
            value: The single character.

        Returns:
            The staged ``Char`` node handle.
        """

    def create_text(self, value: str) -> Node:
        """Stage a detached ``Text`` node owned by this document.

        Args:
            value: The text value.

        Returns:
            The staged ``Text`` node handle.
        """

    def create_active_space(self) -> Node:
        """Stage a detached ``ActiveSpace`` node owned by this document.

        Returns:
            The staged ``ActiveSpace`` node handle.
        """

    def create_group(self, mode: RuntimeContentMode) -> Node:
        """Stage a detached ``Group`` node owned by this document.

        Args:
            mode: The content mode of the group, ``"math"`` or ``"text"``.

        Returns:
            The staged ``Group`` node handle.
        """

    def create_command(
        self, name: str, args: list[ArgValueInput] | None = None
    ) -> Node:
        """Stage a detached ``Command`` node owned by this document.

        Args:
            name: The command name without the leading backslash.
            args: The argument-value dicts for the command's slots, or ``None``
                for none. Content kinds carry a ``node``; leaf kinds carry a
                ``value``.

        Returns:
            The staged ``Command`` node handle.

        Examples:
            doc = texform.Document()
            inner = doc.create_char("x")
            cmd = doc.create_command("sqrt", [{"kind": "Math", "node": inner}])
            doc.append_child(doc.root(), cmd)
            doc.to_latex()  # '\\sqrt { x }'
        """

    def create_declarative(
        self, name: str, args: list[ArgValueInput] | None = None
    ) -> Node:
        """Stage a detached ``Declarative`` command node owned by this document.

        Args:
            name: The declarative command name without the leading backslash.
            args: The argument-value dicts for the command's slots, or ``None``.

        Returns:
            The staged ``Declarative`` node handle.
        """

    def create_environment(
        self, name: str, args: list[ArgValueInput] | None, body: Node
    ) -> Node:
        """Stage a detached ``Environment`` node owned by this document.

        Args:
            name: The environment name.
            args: The argument-value dicts for the environment's slots, or
                ``None``.
            body: The body ``Node`` (a group) of the environment.

        Returns:
            The staged ``Environment`` node handle.
        """

    def append_child(self, parent: Node, child: Node) -> None:
        """Append ``child`` as the last child of ``parent``.

        Args:
            parent: The container node to append into.
            child: The staged or detached node to attach.

        Raises:
            EditError: If the edit is invalid (read-only tree, root protection,
                wrong container shape, or a foreign node).
        """

    def insert_before(self, anchor: Node, new: Node) -> None:
        """Insert ``new`` immediately before ``anchor`` among its siblings.

        Args:
            anchor: The node to insert before.
            new: The staged or detached node to attach.

        Raises:
            EditError: If the edit is invalid.
        """

    def insert_after(self, anchor: Node, new: Node) -> None:
        """Insert ``new`` immediately after ``anchor`` among its siblings.

        Args:
            anchor: The node to insert after.
            new: The staged or detached node to attach.

        Raises:
            EditError: If the edit is invalid.
        """

    def insert_child(self, parent: Node, index: int, child: Node) -> None:
        """Insert ``child`` at ``index`` among ``parent``'s children.

        Args:
            parent: The container node to insert into.
            index: The zero-based position to insert at.
            child: The staged or detached node to attach.

        Raises:
            EditError: If the edit is invalid, including an out-of-bounds index.
        """

    def replace_with(self, target: Node, replacement: Node) -> None:
        """Replace ``target`` in the tree with ``replacement``.

        Args:
            target: The node to remove from its position.
            replacement: The staged or detached node to put in its place.

        Raises:
            EditError: If the edit is invalid.
        """

    def wrap(self, target: Node, wrapper: Node) -> Node:
        """Wrap ``target`` inside ``wrapper``, putting ``wrapper`` in its place.

        Args:
            target: The node to be wrapped.
            wrapper: The staged container node to wrap it in.

        Returns:
            The ``wrapper`` node handle, now in the tree.

        Raises:
            EditError: If the edit is invalid.
        """

    def unwrap(self, group: Node) -> list[Node]:
        """Remove a group, splicing its children into the parent in its place.

        Args:
            group: The group node to dissolve.

        Returns:
            The freed child node handles, now attached to the parent.

        Raises:
            EditError: If the edit is invalid.

        Examples:
            doc = texform.Parser().parse(r"{x y}")["document"]
            group = next(n for n in doc.root().descendants() if n.kind() == "Group")
            doc.unwrap(group)
            doc.to_latex()  # 'x y'
        """

    def extract(self, node: Node) -> Node:
        """Detach ``node`` from the tree and return it as a staged node.

        Args:
            node: The node to remove from its position and keep.

        Returns:
            The detached node handle, reusable in a later edit.

        Raises:
            EditError: If the edit is invalid.
        """

    def remove(self, node: Node) -> None:
        """Remove ``node`` from the tree and discard it.

        Args:
            node: The node to remove.

        Raises:
            EditError: If the edit is invalid, such as removing the root.
        """

    def clear(self, container: Node) -> None:
        """Remove every child of ``container``.

        Args:
            container: The node whose children are removed.

        Raises:
            EditError: If the edit is invalid.
        """

    def set_command_name(self, node: Node, name: str) -> None:
        """Rename a command node in place.

        Args:
            node: The ``Command`` or ``Declarative`` node to rename.
            name: The new command name without the leading backslash.

        Raises:
            EditError: If the edit is invalid.
        """

    def set_text(self, node: Node, value: str) -> None:
        """Set a text node's value in place.

        Args:
            node: The ``Text`` node to update.
            value: The new text value.

        Raises:
            EditError: If the edit is invalid.
        """

    def set_char(self, node: Node, value: str) -> None:
        """Set a char node's value in place.

        Args:
            node: The ``Char`` node to update.
            value: The new single character.

        Raises:
            EditError: If the edit is invalid.
        """

    def set_arg(self, node: Node, index: int, value: ArgValueInput) -> None:
        """Set the argument at ``index`` of a command-like node.

        Args:
            node: The command-like node whose argument is set.
            index: The zero-based slot index.
            value: The argument-value dict to install in the slot.

        Raises:
            EditError: If the edit is invalid, including an out-of-bounds index.
        """


class Node:
    """A read-only handle into a ``Document`` for navigation and reading.

    A ``Node`` is a cheap reference — a shared pointer into the owning document
    plus an identity, never a copy of the subtree. It carries no editing methods;
    all mutation goes through the owning ``Document``. Accessors return content
    only for the node kinds that carry it and ``None`` otherwise, so the same
    handle works uniformly across kinds.

    You obtain handles from ``Document.root()``, ``Document.errors()``,
    ``Document.find_commands()``, the ``create_*`` staging constructors, and the
    navigation methods here.

    Examples:
        doc = texform.Parser().parse(r"\\frac{x}{y}")["document"]
        cmd = doc.root().children()[0]
        cmd.kind()  # 'Command'
        cmd.is_command("frac")  # True

    See Also:
        Document
    """

    def kind(self) -> NodeKind:
        """Return the node kind as a string.

        Returns:
            One of the ``NodeKind`` values, such as ``"Root"``, ``"Command"``, or
            ``"Char"``.

        Examples:
            doc = texform.Parser().parse(r"\\frac{x}{y}")["document"]
            doc.root().kind()  # 'Root'
        """

    def is_command(self, name: str | None = None) -> bool:
        """Report whether the node is a command, optionally a specific one.

        Args:
            name: A command name to match, or ``None`` to match any command.

        Returns:
            ``True`` if the node is a command (and matches ``name`` when given).

        Examples:
            node.is_command()        # any command
            node.is_command("frac")  # specifically \\frac
        """

    def is_char(self, value: str | None = None) -> bool:
        """Report whether the node is a char, optionally a specific one.

        Args:
            value: A character to match, or ``None`` to match any char.

        Returns:
            ``True`` if the node is a char (and holds ``value`` when given).

        Examples:
            node.is_char("+")
        """

    def is_error(self) -> bool:
        """Report whether the node is an ``Error`` placeholder.

        Returns:
            ``True`` for an ``Error`` node.
        """

    def parent(self) -> Node | None:
        """Return the parent node, or ``None`` for the root.

        Returns:
            The parent handle, or ``None``.
        """

    def children(self) -> list[Node]:
        """Return the node's direct children.

        Returns:
            The child handles in order.

        Examples:
            doc = texform.Parser().parse(r"\\frac{x}{y} + a")["document"]
            [n.kind() for n in doc.root().children()]  # ['Command', 'Char', 'Char']
        """

    def next_sibling(self) -> Node | None:
        """Return the following sibling, or ``None`` if this is the last child.

        Returns:
            The next sibling handle, or ``None``.
        """

    def prev_sibling(self) -> Node | None:
        """Return the preceding sibling, or ``None`` if this is the first child.

        Returns:
            The previous sibling handle, or ``None``.
        """

    def ancestors(self) -> list[Node]:
        """Return the node's ancestors from the parent upward.

        Returns:
            The ancestor handles, nearest first.
        """

    def descendants(self) -> list[Node]:
        """Return every descendant of the node.

        Returns:
            The descendant handles in document order.
        """

    def command_name(self) -> str | None:
        """Return the command name for a ``Command`` / ``Declarative`` node.

        Returns:
            The name without the leading backslash, or ``None`` for other kinds.
        """

    def env_name(self) -> str | None:
        """Return the environment name for an ``Environment`` node.

        Returns:
            The environment name, or ``None`` for other kinds.
        """

    def text(self) -> str | None:
        """Return the value of a ``Text`` node.

        Returns:
            The text string, or ``None`` for other kinds.
        """

    def char(self) -> str | None:
        """Return the character of a ``Char`` node.

        Returns:
            The single character, or ``None`` for other kinds.
        """

    def prime_count(self) -> int | None:
        """Return the prime count of a ``Prime`` node.

        Returns:
            The count (greater than zero), or ``None`` for other kinds.

        Examples:
            doc = texform.Parser().parse(r"f''")["document"]
            prime = next(n for n in doc.root().descendants() if n.kind() == "Prime")
            prime.prime_count()  # 2
        """

    def error_parts(self) -> ErrorParts | None:
        """Return the decomposed content of an ``Error`` node.

        Returns:
            A dict ``{"message": str, "snippet": str}``, or ``None`` for other
            kinds.

        Examples:
            doc = texform.Parser().parse(r"\\sqrt[")["document"]
            err = next(n for n in doc.root().descendants() if n.is_error())
            err.error_parts()  # {'message': '...', 'snippet': '\\sqrt['}
        """

    def content_mode(self) -> RuntimeContentMode | None:
        """Return the runtime content mode where applicable.

        Returns:
            ``"math"`` or ``"text"`` for nodes that carry a mode, or ``None``.
        """

    def group_kind(self) -> GroupKindRef | None:
        """Return the group kind for a ``Group`` node.

        Returns:
            A dict with a ``kind`` discriminator (``Explicit``, ``Implicit``,
            ``InlineMath``, or ``Delimited``); a ``Delimited`` group also carries
            ``left`` and ``right`` delimiter values. ``None`` for other kinds.

        Examples:
            doc = texform.Parser().parse(r"{x}")["document"]
            group = next(n for n in doc.root().descendants() if n.kind() == "Group")
            group.group_kind()  # {'kind': 'Explicit'}
        """

    def arg_count(self) -> int:
        """Return the number of argument slots for a command-like node.

        Returns:
            The slot count (zero for nodes that take no arguments).
        """

    def arg(self, index: int) -> ArgRef | None:
        """Return the argument at ``index``, or ``None`` if the slot is empty.

        Args:
            index: The zero-based slot index.

        Returns:
            An argument-value dict — content kinds (``Math``, ``Text``) carry a
            live ``node``; leaf kinds carry a ``value`` — or ``None`` for an empty
            slot.

        Examples:
            doc = texform.Parser().parse(r"\\frac{x}{y}")["document"]
            frac = next(n for n in doc.root().descendants() if n.is_command("frac"))
            frac.arg_count()  # 2
            frac.arg(0)       # {'kind': 'Math', 'node': <Node>}
        """

    def arg_slots(self) -> list[ArgRef | None]:
        """Return all argument slots, with ``None`` for any empty slot.

        Returns:
            The list of argument-value dicts and ``None`` placeholders.
        """

    def script_base(self) -> Node | None:
        """Return the base of a ``Scripted`` node.

        Returns:
            The base node handle, or ``None`` for other kinds.
        """

    def subscript(self) -> Node | None:
        """Return the subscript slot of a ``Scripted`` node.

        Returns:
            The subscript node handle, or ``None`` if absent or another kind.

        Examples:
            doc = texform.Parser().parse(r"x_i^2")["document"]
            sc = next(n for n in doc.root().descendants() if n.kind() == "Scripted")
            sc.subscript().kind(), sc.superscript().kind()  # ('Char', 'Char')
        """

    def superscript(self) -> Node | None:
        """Return the superscript slot of a ``Scripted`` node.

        Returns:
            The superscript node handle, or ``None`` if absent or another kind.
        """

    def infix_left(self) -> Node | None:
        """Return the left operand of an ``Infix`` node.

        Returns:
            The left operand handle, or ``None`` for other kinds.
        """

    def infix_right(self) -> Node | None:
        """Return the right operand of an ``Infix`` node.

        Returns:
            The right operand handle, or ``None`` for other kinds.
        """

    def env_body(self) -> Node | None:
        """Return the body group of an ``Environment`` node.

        Returns:
            The body node handle, or ``None`` for other kinds.
        """

    def span(self) -> Span | None:
        """Return the source byte span of the node.

        Returns:
            A ``{"start": int, "end": int}`` dict, or ``None`` if the node has no
            recorded span.

        Examples:
            doc = texform.Parser().parse(r"a + b")["document"]
            doc.root().children()[0].span()  # {'start': 0, 'end': 1}
        """


class ParseOverrides(TypedDict, total=False):
    """Keyword overlays for ``parse``. Omitted keys keep the baseline.

    These are overlay keys, not a complete ``ParseConfig``. Passing a dict as
    the ``config`` argument still raises ``ConfigError``; use these keys as
    keywords (``parse(src, reject_unknown=True)``) or expand a saved dict with
    ``**overrides``.

    Attributes:
        reject_unknown: Overlay for ``ParseConfig.reject_unknown``.
        abort_on_error: Overlay for ``ParseConfig.abort_on_error``.
        max_group_depth: Overlay for ``ParseConfig.max_group_depth``.
    """

    reject_unknown: bool
    abort_on_error: bool
    max_group_depth: int


class LowerAttributesOverrides(TypedDict, total=False):
    """Keyword overlays for the LowerAttributes phase.

    Nested under ``transform`` / ``normalize`` as ``lower_attributes={...}``.
    The nested value must be a dict, not a ``LowerAttributesConfig`` instance.

    Attributes:
        enabled: Overlay for ``LowerAttributesConfig.enabled``.
    """

    enabled: bool


class RewriteOverrides(TypedDict, total=False):
    """Keyword overlays for the Rewrite phase.

    Nested under ``transform`` / ``normalize`` as ``rewrite={...}``. The nested
    value must be a dict, not a ``RewriteConfig`` instance.

    Attributes:
        enabled: Overlay for ``RewriteConfig.enabled``.
        max_iterations: Overlay for ``RewriteConfig.max_iterations``.
    """

    enabled: bool
    max_iterations: int


class FinalizeAstOverrides(TypedDict, total=False):
    """Keyword overlays for the FinalizeAst phase.

    Nested under ``transform`` / ``normalize`` as ``finalize_ast={...}``. The
    nested value must be a dict, not a ``FinalizeAstConfig`` instance.

    Attributes:
        enabled: Overlay for ``FinalizeAstConfig.enabled``.
    """

    enabled: bool


class FlattenGroupsOverrides(TypedDict, total=False):
    """Keyword overlays for the FlattenGroups phase.

    Nested under ``transform`` / ``normalize`` as ``flatten_groups={...}``. The
    nested value must be a dict, not a ``FlattenGroupsConfig`` instance.

    Attributes:
        enabled: Overlay for ``FlattenGroupsConfig.enabled``.
        preserve_group_containing_declarative_command: Overlay for the same-named
            guard.
        preserve_group_in_script_base_slot: Overlay for the same-named guard.
        preserve_group_inside_env_body: Overlay for the same-named guard.
        preserve_group_containing_infix: Overlay for the same-named guard.
        preserve_group_adjacent_to_command_like: Overlay for the same-named
            guard.
        preserve_group_as_argument_of_command: Overlay for the same-named guard.
        preserve_group_after_scripted_command_like: Overlay for the same-named
            guard.
        preserve_empty_group: Overlay for the same-named guard.
        preserve_group_with_lone_atom_spacing_char: Overlay for the same-named
            guard.
        preserve_group_starting_with_atom_spacing_char: Overlay for the
            same-named guard.
        preserve_group_containing_delimited_pair: Overlay for the same-named
            guard.
    """

    enabled: bool
    preserve_group_containing_declarative_command: bool
    preserve_group_in_script_base_slot: bool
    preserve_group_inside_env_body: bool
    preserve_group_containing_infix: bool
    preserve_group_adjacent_to_command_like: bool
    preserve_group_as_argument_of_command: bool
    preserve_group_after_scripted_command_like: bool
    preserve_empty_group: bool
    preserve_group_with_lone_atom_spacing_char: bool
    preserve_group_starting_with_atom_spacing_char: bool
    preserve_group_containing_delimited_pair: bool


class TransformOverrides(TypedDict, total=False):
    """Keyword overlays for ``transform``.

    Each nested value must be a dict of that phase's overlay keys, not a
    complete phase-config class instance.

    Attributes:
        lower_attributes: Overlay dict for the LowerAttributes phase.
        rewrite: Overlay dict for the Rewrite phase.
        finalize_ast: Overlay dict for the FinalizeAst phase.
        flatten_groups: Overlay dict for the FlattenGroups phase.
    """

    lower_attributes: LowerAttributesOverrides
    rewrite: RewriteOverrides
    finalize_ast: FinalizeAstOverrides
    flatten_groups: FlattenGroupsOverrides


class NormalizeOverrides(ParseOverrides, TransformOverrides, total=False):
    """Flat parse+transform overlays for ``normalize``.

    Combines ``ParseOverrides`` keys with nested phase overlays from
    ``TransformOverrides``. A dict passed as ``config`` still raises
    ``ConfigError``; these keys are keywords only
    (``normalize(src, rewrite={"enabled": False}, reject_unknown=True)``).

    Examples:
        engine = texform.TransformEngine(profile="corpus")
        engine.normalize(r"a \\over b", reject_unknown=True)["normalized"]
        # '\\frac { a } { b }'
    """


class Parser:
    """Turn LaTeX source into a parse result.

    A ``Parser`` consults the knowledge base for command and environment
    signatures to build structured parse trees. It never raises on malformed
    input and never fabricates a placeholder tree; instead it reports diagnostics
    and, in lenient mode, preserves unparseable fragments as ``Error`` nodes.

    Since 0.4.0 the default parse configuration is ``LENIENT``
    (``reject_unknown=False``, ``abort_on_error=False``). Per-call ``config``
    must be a complete ``ParseConfig`` instance; a dict passed as ``config``
    raises ``ConfigError``. Other keywords are overlays
    (``parse(src, reject_unknown=True)``). ``packages=None`` loads the default
    runtime packages (six packages, excluding ``braket``), not every built-in
    package. Loading ``braket`` together with ``physics`` is allowed, but
    ``physics`` overrides their shared command definitions. Built-in packages
    are imported in a fixed order, regardless of the supplied list order.

    For the conceptual model, see the Parsing guide.

    Examples:
        parser = texform.Parser()
        result = parser.parse(r"\\frac{x}{y}")
        result["document"].to_latex()  # '\\frac { x } { y }'
        parser.parse(r"\\frac{x}{y}", config=texform.ParseConfig(reject_unknown=True))

    See Also:
        ParseConfig, Document, ParseResult, Parsing
    """

    def __init__(
        self,
        packages: list[str] | None = None,
        items: list[ContextItem] | None = None,
        remove_commands: list[str] | None = None,
        remove_environments: list[str] | None = None,
        remove_delimiter_controls: list[str] | None = None,
        default_parse_config: ParseConfig | None = None,
    ) -> None:
        """Construct a parser, optionally restricting or customizing knowledge.

        Args:
            packages: Package names to load. ``None`` loads the default runtime
                packages, not every package in the catalog. Use
                ``list_packages()`` to see the available names. An unknown name
                raises ``ConfigError``.
            items: Context-item dicts that inject custom command, environment, or
                delimiter-control knowledge into the parser.
            remove_commands: Command names to drop from the loaded knowledge.
            remove_environments: Environment names to drop from the loaded
                knowledge.
            remove_delimiter_controls: Delimiter-control names to drop from the
                loaded knowledge.
            default_parse_config: Complete parse configuration used when
                ``parse`` is called without ``config`` or overrides.

        Raises:
            ConfigError: If a requested package name is unknown, or an item is
                malformed.
        """

    def parse(
        self,
        src: str,
        config: ParseConfig | None = None,
        **overrides: Unpack[ParseOverrides],
    ) -> ParseResult:
        """Parse a LaTeX string into a parse result.

        Parsing never raises on malformed input. The result has exactly three
        honest states: a hard failure (``document`` is ``None``), a clean parse
        (a ``Document`` whose ``has_errors()`` is ``False``), or a partial parse
        (a read-only ``Document`` whose ``has_errors()`` is ``True``). Empty
        input is a clean parse, not ``None``.

        Args:
            src: The LaTeX source string.
            config: A complete ``ParseConfig``; ``**overrides`` are layered on
                top. ``None`` uses the parser default. A dict here raises
                ``ConfigError``.
            overrides: Keyword overlay from ``ParseOverrides``. Nested values
                must be dicts, not config class instances.

        Returns:
            A ``ParseResult`` dict with two keys: ``document`` (a ``Document`` or
            ``None``) and ``diagnostics`` (a list of diagnostic dicts).

        Raises:
            ConfigError: If ``config`` is not a ``ParseConfig``, a dict is
                passed as ``config``, or an override key or value is invalid.

        Examples:
            result = texform.Parser().parse(r"\\frac{x}{y}")
            document = result["document"]
            diagnostics = result["diagnostics"]

        See Also:
            ParseConfig, ParseResult
        """

    def default_parse_config(self) -> ParseConfig:
        """Return a new ``ParseConfig`` holding this parser's defaults."""

    def lookup_command(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None:
        """Look up the full knowledge entry for a command in a given mode.

        Args:
            name: The command name without the leading backslash.
            mode: ``"math"`` or ``"text"``.

        Returns:
            A dict describing the command, including its parsed ``args`` slots, or
            ``None`` if the command is unknown in that mode.
        """

    def lookup_explicit_command(
        self, name: str, mode: Literal["math", "text"]
    ) -> dict[str, Any] | None:
        """Look up the knowledge entry for an explicit command in a given mode.

        Args:
            name: The command name without the leading backslash.
            mode: ``"math"`` or ``"text"``.

        Returns:
            A knowledge-entry dict, or ``None`` if unknown in that mode.
        """

    def lookup_character(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None:
        """Look up the knowledge entry for a character in a given mode.

        Args:
            name: The character name without the leading backslash.
            mode: ``"math"`` or ``"text"``.

        Returns:
            A knowledge-entry dict, or ``None`` if unknown in that mode.
        """

    def lookup_env(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None:
        """Look up the knowledge entry for an environment in a given mode.

        Args:
            name: The environment name.
            mode: ``"math"`` or ``"text"``.

        Returns:
            A knowledge-entry dict, or ``None`` if unknown in that mode.
        """

    def is_delimiter_control(self, name: str) -> bool:
        """Report whether a name is a delimiter control such as ``langle``.

        Args:
            name: The control name without the leading backslash.

        Returns:
            ``True`` if the name is a known delimiter control.

        Examples:
            texform.Parser().is_delimiter_control("langle")  # True
        """

    def knows_command_name(self, name: str) -> bool:
        """Report whether a command name is known, ignoring mode.

        Args:
            name: The command name without the leading backslash.

        Returns:
            ``True`` if the command is known in any mode.
        """

    def knows_env_name(self, name: str) -> bool:
        """Report whether an environment name is known, ignoring mode.

        Args:
            name: The environment name.

        Returns:
            ``True`` if the environment is known in any mode.
        """

    def knows_character_name(self, name: str) -> bool:
        """Report whether a character name is known, ignoring mode.

        Args:
            name: The character name without the leading backslash.

        Returns:
            ``True`` if the character is known in any mode.
        """


class TransformEngine:
    """Normalize a formula into the canonical form selected by a profile.

    The engine runs a multi-phase pipeline (LowerAttributes, a fixed-point
    Rewrite loop, FinalizeAst, FlattenGroups). A profile picks rule levels;
    ``TransformConfig`` is nested by phase and controls per-run switches
    without changing the selected rule set. The engine also bundles a parser, so
    it exposes ``parse`` and the same knowledge-base lookups as ``Parser``.

    Since 0.4.0 the bundled parser defaults to ``ParseConfig`` ``LENIENT``, and
    ``TransformConfig`` is the nested four-phase object (not flat
    ``rewrite_enabled`` / ``max_iterations`` keys). Per-call ``config`` must be a
    complete ``TransformConfig`` (or ``ParseConfig`` on ``parse``); a dict
    passed as ``config`` raises ``ConfigError``. Other keywords are overlays
    (``normalize(src, rewrite={"enabled": False})``). Nested overlay values must
    be dicts, not phase-config class instances. Construct a full object with
    ``TransformConfig(...)`` (all four phases required) or the profile
    classmethods ``authoring()`` / ``faithful()`` / ``corpus()`` / ``equiv()``.

    Normalization is gated on a complete tree: an incomplete parse raises
    ``ParseError``, since normalizing a tree with holes is meaningless. For the
    conceptual model — profiles, the pipeline, and the eliminated-form contract —
    see the Transforms guide.

    Examples:
        import texform

        engine = texform.TransformEngine(profile="corpus")
        engine.normalize(r"a \\over b")["normalized"]  # '\\frac { a } { b }'
        engine.normalize(r"a \\over b", rewrite={"enabled": False})
        engine.normalize(r"a \\over b", reject_unknown=True, flatten_groups={"enabled": False})
        cfg = engine.default_transform_config()
        cfg.rewrite.max_iterations = 50
        result = engine.parse(r"a \\over b")
        engine.transform(result["document"], cfg)
        engine.transform(result["document"], texform.TransformConfig.faithful(), rewrite={"enabled": False})

    See Also:
        TransformConfig, TransformResult, TransformReport, Parser, Transforms
    """

    def __init__(
        self,
        profile: TransformProfile,
        packages: list[str] | None = None,
        items: list[ContextItem] | None = None,
        remove_commands: list[str] | None = None,
        remove_environments: list[str] | None = None,
        remove_delimiter_controls: list[str] | None = None,
        disable_rules: list[str] | None = None,
        default_parse_config: ParseConfig | None = None,
    ) -> None:
        """Construct a transform engine for a profile.

        Args:
            profile: The normalization profile: ``"authoring"``, ``"faithful"``,
                ``"corpus"``, or ``"equiv"``. An unknown profile raises
                ``ConfigError``.
            packages: Package names to load; ``None`` loads the default runtime
                packages, not every package in the catalog.
            items: Context-item dicts injecting custom knowledge.
            remove_commands: Command names to drop from the knowledge.
            remove_environments: Environment names to drop from the knowledge.
            remove_delimiter_controls: Delimiter-control names to drop.
            disable_rules: Rewrite rule keys to disable, such as
                ``"physics/dv-to-frac-d"``.
            default_parse_config: Complete parse configuration used by
                ``parse`` / ``normalize`` when no per-call parse overlay is given.

        Raises:
            ConfigError: If the profile or a package name is unknown, or an item
                is malformed.
        """

    def normalize(
        self,
        src: str,
        config: TransformConfig | None = None,
        **overrides: Unpack[NormalizeOverrides],
    ) -> TransformResult:
        """Parse, transform, and serialize a formula in one call.

        Normalization is gated on a complete tree. If the input cannot produce a
        complete tree, ``normalize`` raises ``ParseError`` carrying the
        diagnostics and the partial document. Empty input is complete and
        normalizes normally.

        Args:
            src: The LaTeX source string.
            config: A complete ``TransformConfig``; it replaces only the
                transform half of the baseline. ``**overrides`` are layered on
                top. A dict here raises ``ConfigError``.
            overrides: Keyword overlay from ``NormalizeOverrides`` (parse keys
                plus nested phase dicts). Nested values must be dicts, not
                config class instances.

        Returns:
            A ``TransformResult`` dict with ``normalized`` (the canonical LaTeX
            string) and ``report`` (the phase-oriented transform report).

        Raises:
            ParseError: If the source does not parse into a complete tree.
            ConfigError: If ``config`` is not a ``TransformConfig``, a dict is
                passed as ``config``, or an override key or value is invalid.

        Examples:
            engine = texform.TransformEngine(profile="corpus")
            engine.normalize(r"\\dv{f}{x}")["normalized"]
            # '\\frac { \\mathrm { d } f } { \\mathrm { d } x }'

        See Also:
            TransformEngine.transform, TransformResult
        """

    def transform(
        self,
        document: Document,
        config: TransformConfig | None = None,
        **overrides: Unpack[TransformOverrides],
    ) -> TransformReport:
        """Transform a live ``Document`` in place and return the report.

        ``transform`` accepts only documents produced by this engine's own
        ``parse``. A document from ``Document()``, ``Document.from_syntax()``, or
        another parser can still be edited and serialized, but ``transform``
        rejects it with ``TransformError``. The document must also be complete;
        a document that ``has_errors()`` is rejected with ``TransformError``.

        Args:
            document: The live ``Document`` to update in place.
            config: A complete ``TransformConfig``; ``**overrides`` are layered
                on top. It does not accept parse options. A dict here raises
                ``ConfigError``.
            overrides: Keyword overlay from ``TransformOverrides``. Nested
                values must be dicts, not config class instances.

        Returns:
            The phase-oriented transform report dict.

        Raises:
            TransformError: If the document is foreign to this engine, has parse
                errors, or on a contract violation.
            ConfigError: If ``config`` is not a ``TransformConfig``, a dict is
                passed as ``config``, or an override key or value is invalid.

        Examples:
            engine = texform.TransformEngine(profile="corpus")
            result = engine.parse(r"a \\over b")
            document = result["document"]
            assert document is not None
            report = engine.transform(document)
            document.to_latex()  # '\\frac { a } { b }'

        See Also:
            TransformEngine.normalize, TransformReport
        """

    def parse(
        self,
        src: str,
        config: ParseConfig | None = None,
        **overrides: Unpack[ParseOverrides],
    ) -> ParseResult:
        """Parse a LaTeX string using the engine's bundled parser.

        Uses this engine's parser and default parse config. Engines default to
        lenient parsing; pass ``config`` to override it per call. A document
        produced here is owned by this engine and is the only kind ``transform``
        accepts.

        Args:
            src: The LaTeX source string.
            config: A complete ``ParseConfig``; ``**overrides`` are layered on
                top. ``None`` uses the engine default. A dict here raises
                ``ConfigError``.
            overrides: Keyword overlay from ``ParseOverrides``.

        Returns:
            A ``ParseResult`` dict with ``document`` and ``diagnostics``.

        Raises:
            ConfigError: If ``config`` is not a ``ParseConfig``, a dict is
                passed as ``config``, or an override key or value is invalid.

        See Also:
            Parser.parse, TransformEngine.transform
        """

    def default_parse_config(self) -> ParseConfig:
        """Return a new ``ParseConfig`` holding this engine's parse defaults."""

    def default_transform_config(self) -> TransformConfig:
        """Return a new ``TransformConfig`` holding this engine's transform defaults."""

    def lookup_command(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None:
        """Look up the full knowledge entry for a command in a given mode.

        Args:
            name: The command name without the leading backslash.
            mode: ``"math"`` or ``"text"``.

        Returns:
            A dict describing the command, including its parsed ``args`` slots, or
            ``None`` if unknown in that mode.
        """

    def lookup_explicit_command(
        self, name: str, mode: Literal["math", "text"]
    ) -> dict[str, Any] | None:
        """Look up the knowledge entry for an explicit command in a given mode.

        Args:
            name: The command name without the leading backslash.
            mode: ``"math"`` or ``"text"``.

        Returns:
            A knowledge-entry dict, or ``None`` if unknown in that mode.
        """

    def lookup_character(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None:
        """Look up the knowledge entry for a character in a given mode.

        Args:
            name: The character name without the leading backslash.
            mode: ``"math"`` or ``"text"``.

        Returns:
            A knowledge-entry dict, or ``None`` if unknown in that mode.
        """

    def lookup_env(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None:
        """Look up the knowledge entry for an environment in a given mode.

        Args:
            name: The environment name.
            mode: ``"math"`` or ``"text"``.

        Returns:
            A knowledge-entry dict, or ``None`` if unknown in that mode.
        """

    def is_delimiter_control(self, name: str) -> bool:
        """Report whether a name is a delimiter control such as ``langle``.

        Args:
            name: The control name without the leading backslash.

        Returns:
            ``True`` if the name is a known delimiter control.
        """

    def knows_command_name(self, name: str) -> bool:
        """Report whether a command name is known, ignoring mode.

        Args:
            name: The command name without the leading backslash.

        Returns:
            ``True`` if the command is known in any mode.
        """

    def knows_env_name(self, name: str) -> bool:
        """Report whether an environment name is known, ignoring mode.

        Args:
            name: The environment name.

        Returns:
            ``True`` if the environment is known in any mode.
        """

    def knows_character_name(self, name: str) -> bool:
        """Report whether a character name is known, ignoring mode.

        Args:
            name: The character name without the leading backslash.

        Returns:
            ``True`` if the character is known in any mode.
        """


class ParseConfig:
    """Complete parse configuration. The only accepted ``config=`` type for parse.

    Since 0.4.0 this is a complete configuration class, not a dict. Passing a
    dict as ``config`` raises ``ConfigError``; use keyword overlays
    (``parse(src, reject_unknown=True)``) or construct this class and pass it as
    ``config``. Constructor defaults match ``LENIENT`` (the parser and engine
    baseline on both language bindings): ``reject_unknown=False``,
    ``abort_on_error=False``, ``max_group_depth=128``.

    ``reject_unknown`` and ``abort_on_error`` are independent: the former decides
    how unknown names are handled, the latter is a strictness knob for error
    recovery. Neither is equivalent to a parsed tree's ``has_errors()``.

    Attributes:
        reject_unknown: When ``True``, an unknown command or environment becomes a
            diagnostic; when ``False`` (default), unknown names are preserved as
            nodes for lenient exploration.
        abort_on_error: When ``True`` (strict), the parser stops at the first error
            per item and produces no recovery ``Error`` nodes; when ``False``
            (default, lenient), it keeps collecting diagnostics and emits ``Error``
            nodes. The max-group-depth guard emits an ``Error`` node regardless of
            this setting.
        max_group_depth: The maximum group nesting depth before the parser aborts a
            group. Defaults to ``128``.

    Examples:
        cfg = texform.ParseConfig()
        cfg.reject_unknown  # False
        cfg.abort_on_error  # False
        texform.Parser().parse(r"x", config=texform.ParseConfig(reject_unknown=True))

    See Also:
        Parser
    """

    reject_unknown: bool
    abort_on_error: bool
    max_group_depth: int

    def __init__(
        self,
        reject_unknown: bool = False,
        abort_on_error: bool = False,
        max_group_depth: int = 128,
    ) -> None:
        """Construct a parse configuration.

        Args:
            reject_unknown: Whether to reject unknown command and environment names
                with diagnostics instead of preserving them as nodes.
            abort_on_error: Whether to stop at the first error per item (strict)
                instead of collecting diagnostics and emitting ``Error`` nodes
                (lenient).
            max_group_depth: The maximum group nesting depth before aborting.
        """


class LowerAttributesConfig:
    """Complete LowerAttributes phase configuration.

    This is a nested field of ``TransformConfig``, not a dict overlay. Pass the
    parent ``TransformConfig`` as ``config``; to overlay only this phase use
    ``lower_attributes={"enabled": False}``. A dict passed as ``config`` on
    ``transform`` / ``normalize`` raises ``ConfigError``.

    Attributes:
        enabled: Whether the phase runs. Defaults to ``True``.

    Examples:
        texform.LowerAttributesConfig().enabled  # True
        texform.LowerAttributesConfig(enabled=False).enabled  # False
    """

    enabled: bool

    def __init__(self, enabled: bool = True) -> None:
        """Construct a LowerAttributes phase configuration.

        Args:
            enabled: Whether the phase runs.
        """


class RewriteConfig:
    """Complete Rewrite phase configuration.

    Nested under ``TransformConfig.rewrite``. Disabling this phase leaves
    legacy syntax such as ``\\over`` untouched; it does not change which rule
    levels the profile selected. Overlay with ``rewrite={"enabled": False}``;
    do not pass this class as a nested overlay value.

    Attributes:
        enabled: Whether the phase runs. Defaults to ``True``.
        max_iterations: The cap on fixed-point passes. Defaults to ``100``.

    Examples:
        cfg = texform.RewriteConfig(max_iterations=50)
        cfg.enabled  # True
        cfg.max_iterations  # 50
    """

    enabled: bool
    max_iterations: int

    def __init__(
        self,
        enabled: bool = True,
        max_iterations: int = 100,
    ) -> None:
        """Construct a Rewrite phase configuration.

        Args:
            enabled: Whether the phase runs.
            max_iterations: The cap on fixed-point passes.
        """


class FinalizeAstConfig:
    """Complete FinalizeAst phase configuration.

    Nested under ``TransformConfig.finalize_ast``. The phase performs local AST
    cleanup such as merging adjacent ``Prime`` nodes and normalizing text
    sequences. Overlay with ``finalize_ast={"enabled": False}``.

    Attributes:
        enabled: Whether the phase runs. Defaults to ``True``.

    Examples:
        texform.FinalizeAstConfig().enabled  # True
    """

    enabled: bool

    def __init__(self, enabled: bool = True) -> None:
        """Construct a FinalizeAst phase configuration.

        Args:
            enabled: Whether the phase runs.
        """


class FlattenGroupsConfig:
    """Complete FlattenGroups phase configuration.

    Nested under ``TransformConfig.flatten_groups``. Each ``preserve_*`` guard,
    when ``True``, keeps a group matching the named structural condition instead
    of flattening it. Constructor defaults equal the authoring / faithful strict
    guard set (every guard ``True``); the ``corpus`` and ``equiv`` profiles use
    a structural-only subset. Overlay with ``flatten_groups={"enabled": False}``;
    a dict passed as ``config`` still raises ``ConfigError``.

    Attributes:
        enabled: Whether the phase runs.
        preserve_group_containing_declarative_command: Keep a group holding a
            declarative command, whose scope braces are meaningful.
        preserve_group_in_script_base_slot: Keep a group used as a script base, so
            script binding is not changed.
        preserve_group_inside_env_body: Keep a group inside an environment body, so
            cell boundaries are not changed.
        preserve_group_containing_infix: Keep a group containing an infix operator
            such as ``\\over``.
        preserve_group_adjacent_to_command_like: Keep a group adjacent to a
            command-like node where flattening would change association.
        preserve_group_as_argument_of_command: Keep a group serving as a command
            argument.
        preserve_group_after_scripted_command_like: Keep a group following a
            scripted command-like node.
        preserve_empty_group: Keep an empty group ``{}``.
        preserve_group_with_lone_atom_spacing_char: Keep a group whose sole content
            is an atom-spacing character.
        preserve_group_starting_with_atom_spacing_char: Keep a group that begins
            with an atom-spacing character.
        preserve_group_containing_delimited_pair: Keep a group containing a
            delimited pair such as ``\\left( ... \\right)``.

    Examples:
        texform.FlattenGroupsConfig().enabled  # True
        texform.FlattenGroupsConfig(enabled=False).enabled  # False
        texform.TransformConfig.corpus().flatten_groups.preserve_empty_group  # False
    """

    enabled: bool
    preserve_group_containing_declarative_command: bool
    preserve_group_in_script_base_slot: bool
    preserve_group_inside_env_body: bool
    preserve_group_containing_infix: bool
    preserve_group_adjacent_to_command_like: bool
    preserve_group_as_argument_of_command: bool
    preserve_group_after_scripted_command_like: bool
    preserve_empty_group: bool
    preserve_group_with_lone_atom_spacing_char: bool
    preserve_group_starting_with_atom_spacing_char: bool
    preserve_group_containing_delimited_pair: bool

    def __init__(
        self,
        enabled: bool = True,
        preserve_group_containing_declarative_command: bool = True,
        preserve_group_in_script_base_slot: bool = True,
        preserve_group_inside_env_body: bool = True,
        preserve_group_containing_infix: bool = True,
        preserve_group_adjacent_to_command_like: bool = True,
        preserve_group_as_argument_of_command: bool = True,
        preserve_group_after_scripted_command_like: bool = True,
        preserve_empty_group: bool = True,
        preserve_group_with_lone_atom_spacing_char: bool = True,
        preserve_group_starting_with_atom_spacing_char: bool = True,
        preserve_group_containing_delimited_pair: bool = True,
    ) -> None:
        """Construct a FlattenGroups phase configuration.

        Args:
            enabled: Whether the phase runs.
            preserve_group_containing_declarative_command: Keep a group holding a
                declarative command.
            preserve_group_in_script_base_slot: Keep a group used as a script base.
            preserve_group_inside_env_body: Keep a group inside an environment body.
            preserve_group_containing_infix: Keep a group containing an infix
                operator.
            preserve_group_adjacent_to_command_like: Keep a group adjacent to a
                command-like node.
            preserve_group_as_argument_of_command: Keep a group serving as a command
                argument.
            preserve_group_after_scripted_command_like: Keep a group following a
                scripted command-like node.
            preserve_empty_group: Keep an empty group.
            preserve_group_with_lone_atom_spacing_char: Keep a group whose sole
                content is an atom-spacing character.
            preserve_group_starting_with_atom_spacing_char: Keep a group beginning
                with an atom-spacing character.
            preserve_group_containing_delimited_pair: Keep a group containing a
                delimited pair.
        """


class TransformConfig:
    """Complete transform configuration, nested by pipeline phase.

    Since 0.4.0 this is the only accepted ``config=`` type for ``transform`` /
    ``normalize``. A dict passed as ``config`` raises ``ConfigError``; use
    keyword overlays (``normalize(src, rewrite={"enabled": False})``) or
    construct this class. The constructor requires all four phase configs; the
    classmethods ``authoring()`` / ``faithful()`` / ``corpus()`` / ``equiv()``
    return the defaults a profile actually uses.

    The four attributes are shared references: ``cfg.rewrite.enabled = False``
    is visible through ``cfg.rewrite.enabled`` and takes effect when the object
    is passed as ``config``. Nested overlay values must be dicts, not these
    phase-config instances.

    Attributes:
        lower_attributes: The LowerAttributes phase config (shared reference).
        rewrite: The Rewrite phase config (shared reference).
        finalize_ast: The FinalizeAst phase config (shared reference).
        flatten_groups: The FlattenGroups phase config (shared reference).

    Examples:
        config = texform.TransformConfig(
            lower_attributes=texform.LowerAttributesConfig(),
            rewrite=texform.RewriteConfig(max_iterations=50),
            finalize_ast=texform.FinalizeAstConfig(),
            flatten_groups=texform.FlattenGroupsConfig(enabled=False),
        )
        config.rewrite.max_iterations  # 50
        texform.TransformConfig.corpus().flatten_groups.preserve_empty_group  # False

    See Also:
        TransformEngine, RewriteConfig, LowerAttributesConfig, FinalizeAstConfig, FlattenGroupsConfig
    """

    lower_attributes: LowerAttributesConfig
    rewrite: RewriteConfig
    finalize_ast: FinalizeAstConfig
    flatten_groups: FlattenGroupsConfig

    def __init__(
        self,
        *,
        lower_attributes: LowerAttributesConfig,
        rewrite: RewriteConfig,
        finalize_ast: FinalizeAstConfig,
        flatten_groups: FlattenGroupsConfig,
    ) -> None:
        """Construct a transform configuration. All four phases are required.

        Args:
            lower_attributes: The LowerAttributes phase config.
            rewrite: The Rewrite phase config.
            finalize_ast: The FinalizeAst phase config.
            flatten_groups: The FlattenGroups phase config.

        Raises:
            TypeError: If a required phase argument is missing. Start with a
                profile classmethod such as ``TransformConfig.corpus()`` to
                change only selected fields.
        """

    @classmethod
    def authoring(cls) -> TransformConfig:
        """Return the transform config the ``authoring`` profile uses by default."""

    @classmethod
    def faithful(cls) -> TransformConfig:
        """Return the transform config the ``faithful`` profile uses by default."""

    @classmethod
    def corpus(cls) -> TransformConfig:
        """Return the transform config the ``corpus`` profile uses by default."""

    @classmethod
    def equiv(cls) -> TransformConfig:
        """Return the transform config the ``equiv`` profile uses by default."""


def count_targets(
    src: str,
    config: ParseConfig | None = None,
    packages: list[str] | None = None,
) -> dict[str, int]:
    """Count command, environment, and character targets in a LaTeX formula.

    This is a Python-only helper for corpus analysis. The JavaScript binding
    does not provide an equivalent function. It parses the source and reports
    aggregate counts rather than returning a tree, so it requires a complete
    parse.

    Args:
        src: The LaTeX source string.
        config: A ``ParseConfig``, or ``None`` for the defaults.
        packages: Package names to load; ``None`` loads the default runtime
            packages, not every package in the catalog.

    Returns:
        A dict mapping a prefixed target key to its occurrence count. Keys are
        prefixed by kind: ``cmd:`` for commands, ``env:`` for environments, and
        ``char:`` for named character commands such as ``\\alpha``.

    Raises:
        ParseError: If the source does not parse into a complete tree.

    Examples:
        texform.count_targets(r"\\frac{x}{y} + \\alpha")  # {'cmd:frac': 1, 'char:alpha': 1}

    See Also:
        list_packages
    """

def validate_argspec(spec: str) -> ValidateArgspecResult:
    """Validate an xparse-style argspec string and report its parsed slots.

    Use it to self-check an argspec before injecting a custom command into a
    ``Parser``. It never raises on a malformed spec; instead the result's
    ``valid`` is ``False`` and ``error`` carries the reason.

    Args:
        spec: The argspec string, such as ``"o m"``, ``"s m{}"``, or
            ``"d<(,)><[,]>"``.

    Returns:
        A ``ValidateArgspecResult`` dict: ``valid``, ``error``, ``arg_count``, and
        ``parsed`` (the per-slot breakdown when valid).

    Examples:
        texform.validate_argspec("o m")  # {'valid': True, 'error': None, 'arg_count': 2, 'parsed': [...]}

    See Also:
        ValidateArgspecResult, ParsedArgSpecSlot
    """

def list_packages() -> list[PackageInfo]:
    """List the built-in knowledge packages with their record counts.

    The returned names are the package identifiers accepted by the ``packages``
    argument of ``Parser`` and ``TransformEngine``.

    Returns:
        A list of ``PackageInfo`` dicts, each ``{"name": str, "commands": int,
        "environments": int}``.

    Examples:
        texform.list_packages()  # [{'name': 'ams', 'commands': 35, 'environments': 28}, ...]

    See Also:
        PackageInfo, Parser, TransformEngine
    """

CommandSpacing = Literal["spaced", "minimal"]
"""Spacing around control sequences in serialized output: ``"spaced"`` pads
around them; ``"minimal"`` emits only the separation LaTeX requires."""

MathGroupInnerSpacing = Literal["padded", "compact"]
"""Inner spacing of math groups: ``"padded"`` writes ``{ x }``; ``"compact"``
writes ``{x}``."""

AdjacentCharSpacing = Literal["spaced", "compact"]
"""Spacing between adjacent characters: ``"spaced"`` writes ``a b c``;
``"compact"`` writes ``abc``."""

ScriptSpacing = Literal["spaced", "compact"]
"""Spacing around script operators: ``"spaced"`` writes ``x _ { i } ^ { 2 }``;
``"compact"`` writes ``x_{ i }^{ 2 }``."""

ScriptOrder = Literal["sub_first", "sup_first"]
"""Order of subscript and superscript in serialized output: ``"sub_first"``
writes ``x _ { i } ^ { 2 }``; ``"sup_first"`` writes ``x ^ { 2 } _ { i }``."""

InfixGrouping = Literal["always_explicit", "when_required"]
"""Bracing of infix operands (``\\over``, ``\\atop``, ...):
``"when_required"`` braces an operand only when the surrounding syntax needs
it; ``"always_explicit"`` braces every non-empty operand."""

EnvironmentNameSpacing = Literal["spaced", "compact"]
"""Spacing after ``\\begin`` / ``\\end``: ``"spaced"`` writes
``\\begin {matrix}``; ``"compact"`` writes ``\\begin{matrix}``."""


class SerializeOptions(TypedDict, total=False):
    """Options controlling serialized LaTeX output style.

    A flat dict keyed by snake_case names. Passed as keywords to
    ``Document.to_latex`` and ``serialize``. Unknown keys and wrong types raise
    ``ConfigError``. For a task-oriented walkthrough, see the Serialization
    guide.

    Attributes:
        command_spacing: Spacing around control sequences. Default ``"spaced"``.
        group_inner_spacing: Inner spacing of math groups. Default ``"padded"``.
        adjacent_char_spacing: Spacing between adjacent characters. Default ``"spaced"``.
        script_spacing: Spacing around script operators. Default ``"spaced"``.
        script_order: Order of subscript and superscript. Default ``"sub_first"``.
        infix_operand_grouping: Bracing of infix operands. Default ``"when_required"``.
        environment_name_spacing: Spacing after ``\\begin`` / ``\\end``. Default ``"spaced"``.

    Examples:
        result = texform.Parser().parse(r"x_i^2")
        syntax = result["document"].to_syntax()
        texform.serialize(syntax, script_order="sup_first")  # 'x ^ { 2 } _ { i }'

    See Also:
        Document.to_latex, serialize
    """

    command_spacing: CommandSpacing | None
    group_inner_spacing: MathGroupInnerSpacing | None
    adjacent_char_spacing: AdjacentCharSpacing | None
    script_spacing: ScriptSpacing | None
    script_order: ScriptOrder | None
    infix_operand_grouping: InfixGrouping | None
    environment_name_spacing: EnvironmentNameSpacing | None


def serialize(node: SyntaxNode, **options: Unpack[SerializeOptions]) -> str:
    """Render a ``SyntaxNode`` dict to LaTeX text using the canonical serializer.

    This is the free-function counterpart to ``Document.to_latex()``; both take
    the same keyword options. ``Error`` nodes round-trip their captured snippet,
    pure prime superscripts serialize compactly as ``f'`` / ``f''``, and the
    serializer guarantees text idempotency. For the conceptual model, see the
    Serialization guide.

    Args:
        node: A ``SyntaxNode`` dict, typically from ``Document.to_syntax()`` or a
            stored snapshot.
        **options: Serializer style overlays. Omitted keys and explicit ``None``
            keep the default (spaced) style.

    Returns:
        The serialized LaTeX string.

    Raises:
        ConfigError: If an option key is unknown or a value has the wrong type.

    Examples:
        result = texform.Parser().parse(r"x^2")
        document = result["document"]
        assert document is not None
        syntax = document.to_syntax()
        texform.serialize(syntax)                                            # 'x ^ { 2 }'
        texform.serialize(syntax, script_spacing="compact")                  # 'x^{ 2 }'

    See Also:
        SerializeOptions, Document.to_latex, Serialization
    """
