from typing import Any

__all__ = ["ParseError", "parse"]


class ParseError(Exception):
    """Raised when parsing produces diagnostics.

    Attributes:
        diagnostics: Ordered list of diagnostic dicts emitted by the parser.
        partial_result: Best-effort partial AST, or ``None`` if the parser
            could not recover any tree.
    """

    diagnostics: list[dict[str, Any]]
    partial_result: dict[str, Any] | None


def parse(src: str, strict: bool = False) -> dict[str, Any]:
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
