from typing import Any, Literal, TypeAlias, TypedDict

TransformProfile = Literal["authoring", "faithful", "corpus", "equiv"]
RuntimeContentMode = Literal["math", "text"]
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
ContextItem = dict[str, Any]
SyntaxNode: TypeAlias = dict[str, Any]
Span: TypeAlias = dict[str, int]
ArgSpecKindType = Literal[
    "content", "delimiter", "csname", "dimension", "integer", "keyval", "column", "star"
]
ArgSpecFormType = Literal["standard", "star", "group", "delimited", "paired"]


class ParseDiagnostic(TypedDict, total=False):
    kind: ParseDiagnosticKind
    message: str
    span: Span
    expected: list[str]
    found: str
    contexts: list[dict[str, Any]]


class ParseResult(TypedDict):
    document: Document | None
    diagnostics: list[ParseDiagnostic]


class NodeSpanEntry(TypedDict):
    id: str
    span: Span


class PackageInfo(TypedDict):
    name: str
    commands: int
    environments: int


class ErrorParts(TypedDict):
    message: str
    snippet: str


class FinalizeAstStepReport(TypedDict):
    applied_count: int


class FinalizeAstStepReports(TypedDict):
    merge_adjacent_primes: FinalizeAstStepReport


class FinalizeAstReport(TypedDict):
    steps: FinalizeAstStepReports


class TransformReport(TypedDict):
    iterations: int
    rules: list[dict[str, Any]]
    finalize_ast: FinalizeAstReport
    flatten_groups: dict[str, Any]
    lower_attributes: dict[str, Any]


class TransformResult(TypedDict):
    normalized: str
    report: TransformReport


class ParsedArgSpecSlot(TypedDict):
    required: bool
    no_leading_space: bool
    nullable: bool
    kind: dict[str, Any]
    form: dict[str, Any]


class ValidateArgspecResult(TypedDict):
    valid: bool
    error: str | None
    arg_count: int | None
    parsed: list[ParsedArgSpecSlot] | None


class DelimiterNone(TypedDict):
    kind: Literal["None"]


class DelimiterChar(TypedDict):
    kind: Literal["Char"]
    value: str


class DelimiterControl(TypedDict):
    kind: Literal["Control"]
    value: str


DelimiterValue: TypeAlias = DelimiterNone | DelimiterChar | DelimiterControl


class GroupKindExplicit(TypedDict):
    kind: Literal["Explicit"]


class GroupKindImplicit(TypedDict):
    kind: Literal["Implicit"]


class GroupKindDelimited(TypedDict):
    kind: Literal["Delimited"]
    left: DelimiterValue
    right: DelimiterValue


class GroupKindInlineMath(TypedDict):
    kind: Literal["InlineMath"]


GroupKindRef: TypeAlias = (
    GroupKindExplicit | GroupKindImplicit | GroupKindDelimited | GroupKindInlineMath
)


class MathArg(TypedDict):
    kind: Literal["Math"]
    node: Node


class TextArg(TypedDict):
    kind: Literal["Text"]
    node: Node


class DelimiterArg(TypedDict):
    kind: Literal["Delimiter"]
    value: DelimiterValue


class CSNameArg(TypedDict):
    kind: Literal["CSName"]
    value: str


class DimensionArg(TypedDict):
    kind: Literal["Dimension"]
    value: str


class IntegerArg(TypedDict):
    kind: Literal["Integer"]
    value: str


class KeyValArg(TypedDict):
    kind: Literal["KeyVal"]
    value: str


class ColumnArg(TypedDict):
    kind: Literal["Column"]
    value: str


class BooleanArg(TypedDict):
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
ArgValueInput: TypeAlias = ArgRef

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
    "serialize",
    "validate_argspec",
]


class TexformError(Exception): ...


class ParseError(TexformError):
    diagnostics: list[ParseDiagnostic]
    document: Document | None


class EditError(TexformError): ...


class ConfigError(TexformError): ...


class TransformError(TexformError): ...


class Document:
    def __init__(self) -> None: ...
    @staticmethod
    def from_syntax(node: SyntaxNode) -> Document: ...
    def root(self) -> Node: ...
    def has_errors(self) -> bool: ...
    def is_read_only(self) -> bool: ...
    def errors(self) -> list[Node]: ...
    def find_commands(self, name: str) -> list[Node]: ...
    def find_environments(self, name: str) -> list[Node]: ...
    def to_syntax(self) -> SyntaxNode: ...
    def node_spans(self) -> list[NodeSpanEntry]: ...
    def to_latex(self, options: dict[str, Any] | None = None) -> str: ...
    def create_char(self, value: str) -> Node: ...
    def create_text(self, value: str) -> Node: ...
    def create_active_space(self) -> Node: ...
    def create_group(self, mode: RuntimeContentMode) -> Node: ...
    def create_command(
        self, name: str, args: list[ArgValueInput] | None = None
    ) -> Node: ...
    def create_declarative(
        self, name: str, args: list[ArgValueInput] | None = None
    ) -> Node: ...
    def create_environment(
        self, name: str, args: list[ArgValueInput] | None, body: Node
    ) -> Node: ...
    def append_child(self, parent: Node, child: Node) -> None: ...
    def insert_before(self, anchor: Node, new: Node) -> None: ...
    def insert_after(self, anchor: Node, new: Node) -> None: ...
    def insert_child(self, parent: Node, index: int, child: Node) -> None: ...
    def replace_with(self, target: Node, replacement: Node) -> None: ...
    def wrap(self, target: Node, wrapper: Node) -> Node: ...
    def unwrap(self, group: Node) -> list[Node]: ...
    def extract(self, node: Node) -> Node: ...
    def remove(self, node: Node) -> None: ...
    def clear(self, container: Node) -> None: ...
    def set_command_name(self, node: Node, name: str) -> None: ...
    def set_text(self, node: Node, value: str) -> None: ...
    def set_char(self, node: Node, value: str) -> None: ...
    def set_arg(self, node: Node, index: int, value: ArgValueInput) -> None: ...


class Node:
    def kind(self) -> NodeKind: ...
    def is_command(self, name: str | None = None) -> bool: ...
    def is_char(self, value: str | None = None) -> bool: ...
    def is_error(self) -> bool: ...
    def parent(self) -> Node | None: ...
    def children(self) -> list[Node]: ...
    def next_sibling(self) -> Node | None: ...
    def prev_sibling(self) -> Node | None: ...
    def ancestors(self) -> list[Node]: ...
    def descendants(self) -> list[Node]: ...
    def command_name(self) -> str | None: ...
    def env_name(self) -> str | None: ...
    def text(self) -> str | None: ...
    def char(self) -> str | None: ...
    def prime_count(self) -> int | None: ...
    def error_parts(self) -> ErrorParts | None: ...
    def content_mode(self) -> RuntimeContentMode | None: ...
    def group_kind(self) -> GroupKindRef | None: ...
    def arg_count(self) -> int: ...
    def arg(self, index: int) -> ArgRef | None: ...
    def arg_slots(self) -> list[ArgRef | None]: ...
    def script_base(self) -> Node | None: ...
    def subscript(self) -> Node | None: ...
    def superscript(self) -> Node | None: ...
    def infix_left(self) -> Node | None: ...
    def infix_right(self) -> Node | None: ...
    def env_body(self) -> Node | None: ...
    def span(self) -> Span | None: ...


class Parser:
    def __init__(
        self,
        packages: list[str] | None = None,
        items: list[ContextItem] | None = None,
        remove_commands: list[str] | None = None,
        remove_environments: list[str] | None = None,
        remove_delimiter_controls: list[str] | None = None,
    ) -> None: ...
    def parse(
        self,
        src: str,
        config: ParseConfig | dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> ParseResult: ...
    def lookup_command(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None: ...
    def lookup_explicit_command(
        self, name: str, mode: Literal["math", "text"]
    ) -> dict[str, Any] | None: ...
    def lookup_character(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None: ...
    def lookup_env(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None: ...
    def is_delimiter_control(self, name: str) -> bool: ...
    def knows_command_name(self, name: str) -> bool: ...
    def knows_env_name(self, name: str) -> bool: ...
    def knows_character_name(self, name: str) -> bool: ...


class TransformEngine:
    def __init__(
        self,
        profile: TransformProfile,
        packages: list[str] | None = None,
        items: list[ContextItem] | None = None,
        remove_commands: list[str] | None = None,
        remove_environments: list[str] | None = None,
        remove_delimiter_controls: list[str] | None = None,
        disable_rules: list[str] | None = None,
    ) -> None: ...
    def normalize(
        self,
        src: str,
        config: TransformConfig | dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> TransformResult: ...
    def parse(
        self,
        src: str,
        config: ParseConfig | dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> ParseResult: ...
    def lookup_command(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None: ...
    def lookup_explicit_command(
        self, name: str, mode: Literal["math", "text"]
    ) -> dict[str, Any] | None: ...
    def lookup_character(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None: ...
    def lookup_env(self, name: str, mode: Literal["math", "text"]) -> dict[str, Any] | None: ...
    def is_delimiter_control(self, name: str) -> bool: ...
    def knows_command_name(self, name: str) -> bool: ...
    def knows_env_name(self, name: str) -> bool: ...
    def knows_character_name(self, name: str) -> bool: ...


class ParseConfig:
    reject_unknown: bool
    abort_on_error: bool
    max_group_depth: int

    def __init__(
        self,
        reject_unknown: bool = False,
        abort_on_error: bool = False,
        max_group_depth: int = 128,
    ) -> None: ...


class LowerAttributesConfig:
    enabled: bool

    def __init__(self, enabled: bool = True) -> None: ...


class RewriteConfig:
    enabled: bool
    max_iterations: int

    def __init__(
        self,
        enabled: bool = True,
        max_iterations: int = 100,
    ) -> None: ...


class FinalizeAstConfig:
    enabled: bool

    def __init__(self, enabled: bool = True) -> None: ...


class FlattenGroupsConfig:
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
    ) -> None: ...


class TransformConfig:
    lower_attributes: LowerAttributesConfig
    rewrite: RewriteConfig
    finalize_ast: FinalizeAstConfig
    flatten_groups: FlattenGroupsConfig

    def __init__(
        self,
        lower_attributes: LowerAttributesConfig | None = None,
        rewrite: RewriteConfig | None = None,
        finalize_ast: FinalizeAstConfig | None = None,
        flatten_groups: FlattenGroupsConfig | None = None,
    ) -> None: ...

    @classmethod
    def authoring(cls) -> TransformConfig: ...
    @classmethod
    def faithful(cls) -> TransformConfig: ...
    @classmethod
    def corpus(cls) -> TransformConfig: ...
    @classmethod
    def equiv(cls) -> TransformConfig: ...


def count_targets(
    src: str,
    config: ParseConfig | None = None,
    packages: list[str] | None = None,
) -> dict[str, int]:
    """Count command, environment, and character targets in a LaTeX formula."""
    ...


def validate_argspec(spec: str) -> ValidateArgspecResult: ...


def list_packages() -> list[PackageInfo]:
    """List all built-in knowledge packages with record counts."""
    ...


def serialize(node: dict[str, Any], options: dict[str, Any] | None = None) -> str: ...
