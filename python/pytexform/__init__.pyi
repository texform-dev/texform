from typing import Any, Literal

__all__ = ["ParseError", "count_targets", "normalize", "parse"]


class ParseError(Exception):
    """Raised when parsing produces diagnostics.

    Attributes:
        diagnostics: Ordered list of diagnostic dicts emitted by the parser.
        partial_result: Best-effort partial AST, or ``None`` if the parser
            could not recover any tree.
    """

    diagnostics: list[dict[str, Any]]
    partial_result: dict[str, Any] | None


def parse(
    src: str,
    strict: bool = False,
    packages: list[str] | None = None,
) -> dict[str, Any]:
    """Parse a LaTeX formula.

    Args:
        src: Source string of the LaTeX formula.
        strict: When ``True``, unknown commands raise :class:`ParseError`.
            Defaults to ``False``.

    Returns:
        A dict with ``node`` (root AST node) and ``span`` (byte range) keys.

    Raises:
        ParseError: If the parser emits any diagnostics. The exception
            carries ``diagnostics`` and ``partial_result`` attributes.
    """
    ...


def normalize(
    src: str,
    profile: Literal["authoring", "corpus", "corpus-drop", "equiv"] = "authoring",
    strict: bool = True,
    packages: list[str] | None = None,
) -> dict[str, Any]:
    """Normalize a LaTeX formula and return the normalized source plus report."""
    ...


def count_targets(
    src: str,
    strict: bool = False,
    packages: list[str] | None = None,
) -> dict[str, int]:
    """Count command, environment, and character targets in a LaTeX formula."""
    ...
