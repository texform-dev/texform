from collections.abc import Iterable
from typing import Any, Literal

RewriteClass = Literal["standard", "expand", "drop", "equiv"]
TransformProfile = Literal["authoring", "corpus", "corpus-drop", "equiv"]

__all__ = [
    "FlattenGroupsConfig",
    "LowerAttributesConfig",
    "ParseConfig",
    "ParseError",
    "RewriteConfig",
    "TransformConfig",
    "count_targets",
    "normalize",
    "parse",
    "transform",
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
    classes: list[RewriteClass]
    max_iterations: int

    def __init__(
        self,
        enabled: bool = True,
        classes: Iterable[RewriteClass] | None = None,
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


def parse(
    src: str,
    config: ParseConfig | None = None,
    packages: list[str] | None = None,
) -> dict[str, Any]:
    """Parse a LaTeX formula.

    Args:
        src: Source string of the LaTeX formula.
        config: Parse configuration. Defaults to ``ParseConfig()``.

    Returns:
        A dict with ``node`` (root AST node) and ``span`` (byte range) keys.

    Raises:
        ParseError: If the parser emits any diagnostics. The exception
            carries ``diagnostics`` and ``partial_result`` attributes.
    """
    ...


def normalize(
    src: str,
    profile: TransformProfile = "authoring",
    packages: list[str] | None = None,
) -> dict[str, Any]:
    """Normalize a LaTeX formula and return the normalized source plus report."""
    ...


def transform(
    src: str,
    config: TransformConfig | None = None,
    packages: list[str] | None = None,
) -> dict[str, Any]:
    """Transform a LaTeX formula and return the normalized source plus report."""
    ...


def count_targets(
    src: str,
    config: ParseConfig | None = None,
    packages: list[str] | None = None,
) -> dict[str, int]:
    """Count command, environment, and character targets in a LaTeX formula."""
    ...
