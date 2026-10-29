"""Run inside LLDB against program.scoop's real DWARF, without launching it."""

import lldb


def __lldb_init_module(debugger, _internal_dict):
    target = debugger.GetSelectedTarget()
    for name in ("Choice", "Option<String>", "String", "MutableArray<Long>", "Box", "Value"):
        assert target.FindFirstType(name).IsValid(), name

    def value(name, words):
        data = lldb.SBData.CreateDataFromUInt64Array(target.GetByteOrder(), 8, words)
        return target.CreateValueFromData("test", data, target.FindFirstType(name))

    tagged = value("Choice", [0, 7, 0])
    assert tagged.GetSummary() == "Choice.Number", str(tagged)
    assert tagged.GetNumChildren() == 1
    payload = tagged.GetChildAtIndex(0)
    assert payload.GetName() == "Number"
    assert payload.GetChildAtIndex(0).GetValueAsSigned() == 7
    empty = value("Choice", [2, 0, 0])
    assert empty.GetSummary() == "Choice.Empty", str(empty)
    assert empty.GetChildAtIndex(0).GetNumChildren() == 0
    invalid = value("Choice", [90, 0, 0])
    assert "unknown enum tag" in invalid.GetSummary(), str(invalid)
    assert invalid.GetNumChildren() == 0
    absent = value("Option<String>", [0])
    assert absent.GetSummary() == "Option<String>.None", str(absent)
    assert absent.GetChildAtIndex(0).GetName() == "None"
    present = value("Option<String>", [0x1234])
    assert present.GetSummary() == "Option<String>.Some", str(present)
    assert present.GetChildAtIndex(0).GetName() == "Some"
    null_string = value("String", [0])
    assert null_string.GetSummary() == "<null>", str(null_string)
    assert null_string.GetNumChildren() == 0
    array = target.FindFirstType("MutableArray<Long>").GetCanonicalType().GetPointeeType()
    data = next(field for field in array.fields if field.GetName() == "data")
    assert data.GetOffsetInBytes() == 24
    assert data.GetType().GetArrayElementType().GetByteSize() == 8
    print("SCOOP_FORMATTERS_OK")
