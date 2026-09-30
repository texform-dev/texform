from typing_extensions import assert_type
import texform

tables = texform.Document().to_columnar()
assert_type(tables, texform.ColumnarTree)
assert_type(tables["nodes"]["parent"], list[int])
assert_type(tables["nodes"]["kind"], list[texform.NodeKind])
assert_type(tables["nodes"]["slot_index"], list[int])
assert_type(tables["args"]["value"], list[str | None])
assert_type(tables["args"]["present"], list[bool])
