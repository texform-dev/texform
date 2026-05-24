from typing import Any, Literal

TransformProfile = Literal["authoring", "corpus", "corpus-drop", "equiv"]
ContextItem = dict[str, Any]

__all__ = [
    "FlattenGroupsConfig",
    "Engine",
    "LowerAttributesConfig",
    "ParseConfig",
    "ParseError",
    "Parser",
    "RewriteConfig",
    "TransformConfig",
    "count_targets",
    "serialize",
    "validate_argspec",
]


class ParseError(Exception):
    """Raised when parsing produces diagnostics.

    Attributes:
        diagnostics: Ordered list of diagnostic dicts emitted by the parser.
        partial_result: Best-effort partial AST, or ``None`` if the parser
            could not recover any tree.
    """

    diagnostics: list[dict[str, Any]]
    partial_result: dict[str, Any] | None


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
    ) -> dict[str, Any]: ...
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


class Engine:
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
    ) -> dict[str, Any]: ...
    def parse(
        self,
        src: str,
        config: ParseConfig | dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> dict[str, Any]: ...
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
    strict: bool
    recover: bool
    max_group_depth: int

    def __init__(
        self,
        strict: bool = False,
        recover: bool = True,
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
    flatten_groups: FlattenGroupsConfig

    def __init__(
        self,
        lower_attributes: LowerAttributesConfig | None = None,
        rewrite: RewriteConfig | None = None,
        flatten_groups: FlattenGroupsConfig | None = None,
    ) -> None: ...

    @classmethod
    def authoring(cls) -> "TransformConfig": ...
    @classmethod
    def corpus(cls) -> "TransformConfig": ...
    @classmethod
    def corpus_drop(cls) -> "TransformConfig": ...
    @classmethod
    def equiv(cls) -> "TransformConfig": ...


def count_targets(
    src: str,
    config: ParseConfig | None = None,
    packages: list[str] | None = None,
) -> dict[str, int]:
    """Count command, environment, and character targets in a LaTeX formula."""
    ...


def validate_argspec(spec: str) -> dict[str, Any]: ...


def serialize(node: dict[str, Any], options: dict[str, Any] | None = None) -> str: ...
