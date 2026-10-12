from typing_extensions import assert_type
import texform

doc = texform.Document()
options: texform.SerializeOptions = {"style": "compact"}
assert_type(doc.to_latex(**options), str)
assert_type(doc.to_tokenized_latex(style="spaced")["latex"], str)
assert_type(texform.serialize(doc.to_syntax(), style="compact"), str)
doc.to_latex(style=None)
doc.to_latex(script_order="sup_first")  # type: ignore[call-arg]
doc.to_latex(style="pretty")  # type: ignore[arg-type]
