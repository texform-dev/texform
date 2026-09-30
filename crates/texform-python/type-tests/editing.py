from typing_extensions import assert_type
import texform

doc = texform.Document()
base = doc.create_char("x")
script = doc.set_subscript(base, "i")
assert_type(script, texform.Node)
assert_type(doc.set_superscript(base, "2"), texform.Node)
assert_type(doc.set_subscript(script, None), texform.Node)
assert_type(doc.clone_node(script), texform.Node)
assert_type(doc.import_node(script), texform.Node)
assert_type(doc.node_at("root"), texform.Node)
assert_type(script.path(), str | None)
assert_type(script.slot(), texform.NodeSlot | None)
assert_type(script.is_known(), bool | None)
doc.set_prime_count(script, 2)
doc.set_delimiters(script, "(", ")")
doc.set_arg_delimiters(script, 0, "[", "]")
