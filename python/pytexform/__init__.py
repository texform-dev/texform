"""pytexform — Python bindings for the TeXForm LaTeX parser."""

from ._native import ParseError, parse

__all__ = ["ParseError", "parse"]
