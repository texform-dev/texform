from typing_extensions import assert_type
import texform

doc = texform.Document()
options: texform.SerializeOptions = {"style": "compact"}
assert_type(doc.to_latex(**options), str)
tokenized = doc.to_tokenized_latex(style="spaced")
assert_type(tokenized["latex"], str)
if tokenized["tokens"]:
    first = tokenized["tokens"][0]
    assert_type(first["role"], texform.SerializationTokenRole)
    assert_type(first["node_path"], str)
    assert_type(first["slot"], texform.NodeSlot | None)
assert_type(texform.serialize(doc.to_syntax(), style="compact"), str)
doc.to_latex(style=None)
doc.to_latex(script_order="sup_first")  # type: ignore[call-arg]
doc.to_latex(style="pretty")  # type: ignore[arg-type]
