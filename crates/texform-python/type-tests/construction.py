from typing_extensions import assert_type
import texform

doc = texform.Document()
node = doc.create_command("frac", ["a", "b"])
assert_type(node, texform.Node)
pair = texform.Paired(node, "(", ")")
assert_type(pair.value, texform.Node | str)
assert_type(pair.open, str)
doc.create_command("qty", [pair, None, True])
doc.create_group(children=[node, "x"])
doc.create_delimited_group("(", ")", ["x"])
doc.create_inline_math(["x"], mode="text")
doc.create_scripted("x", sub=node, sup="2")
doc.create_prime(2, mode="math")
doc.create_infix("over", "x", node)
doc.create_environment("matrix", body=[node, "x"])
doc.create_environment("matrix", body="x")
doc.create_environment("matrix", body=None)
doc.create_text("hello", mode="text")
doc.parse_fragment("x", mode="math")
doc.set_arg(node, 0, "y")
doc.set_env_name(node, "matrix")
try:
    doc.create_char("xx")
except texform.ConformanceError as error:
    assert_type(error.path, str)
    assert_type(error.rule, str)
except texform.ParseError as parse_error:
    assert_type(parse_error.diagnostics, list[texform.ParseDiagnostic])
