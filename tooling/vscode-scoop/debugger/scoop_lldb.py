"""Read-only Scoop formatters for the bundled LLVM 22 LLDB.

Use emitted DWARF members for layouts. Never evaluate inferior expressions,
retain managed addresses across stops, or add runtime calls to display a value.
"""

import codecs
import json

import lldb

TEXT_PREVIEW_BYTES = 4096
# Scoop runtime metadata ABI 9, ScoopTypeDescriptor.diagnostic_name.
TYPE_NAME_OFFSET = 112


class Unavailable(Exception):
    pass


def record_type(type_):
    type_ = type_.GetCanonicalType()
    if type_.IsPointerType() or type_.IsReferenceType():
        type_ = type_.GetPointeeType().GetCanonicalType()
    return type_


def fields(type_):
    type_ = record_type(type_)
    return [type_.GetFieldAtIndex(i) for i in range(type_.GetNumberOfFields())]


def kind(type_):
    members = {member.GetName() for member in fields(type_)}
    if "__scoop_tag" in members or "__scoop_niche" in members:
        return "enum"
    if "__scoop_object" in members and "__scoop_itable" in members:
        return "interface"
    if {"__scoop_type", "__scoop_gc"} <= members:
        if {"length", "data"} <= members:
            return "string" if record_type(type_).GetName() == "String" else "array"
        return "object"
    return None


def recognizes(type_, *_):
    return kind(type_) is not None


def checked(value):
    if not value.IsValid() or value.GetError().Fail():
        raise Unavailable("value is unavailable")
    return value.GetNonSyntheticValue()


def unsigned(value):
    value = checked(value)
    error = lldb.SBError()
    result = value.GetValueAsUnsigned(error)
    if error.Fail():
        raise Unavailable("value has no readable location")
    return result


def object_value(value):
    value = checked(value)
    type_ = value.GetType().GetCanonicalType()
    if type_.IsPointerType() or type_.IsReferenceType():
        if unsigned(value) == 0:
            raise Unavailable("null")
        value = checked(value.Dereference())
    return value


def member(value, name):
    return checked(value.GetChildMemberWithName(name))


def read(value, address, count):
    process = value.GetProcess()
    if process.GetState() not in (lldb.eStateStopped, lldb.eStateCrashed, lldb.eStateSuspended):
        raise Unavailable("process is not stopped")
    if count == 0:
        return b""
    error = lldb.SBError()
    data = process.ReadMemory(address, count, error)
    if error.Fail() or len(data) != count:
        raise Unavailable("memory is unavailable")
    return data


def length(value):
    error = lldb.SBError()
    size = member(value, "length").GetValueAsSigned(error)
    if error.Fail() or size < 0:
        raise Unavailable("length is unavailable")
    return size


def array_layout(value):
    field = next(field for field in fields(value.GetType()) if field.GetName() == "data")
    element = field.GetType().GetArrayElementType()
    if not element.IsValid():
        raise Unavailable("element type is unavailable")
    address = value.GetLoadAddress()
    if address == lldb.LLDB_INVALID_ADDRESS:
        raise Unavailable("array has no address")
    return address + field.GetOffsetInBytes(), element


def string_summary(value):
    size = length(value)
    address, _ = array_layout(value)
    data = read(value, address, min(size, TEXT_PREVIEW_BYTES))
    truncated = size > len(data)
    text = codecs.getincrementaldecoder("utf-8")("replace").decode(data, final=not truncated)
    result = json.dumps(text, ensure_ascii=False)
    return result + (f"… ({size} bytes)" if truncated else "")


def active_variant(value):
    tag = value.GetChildMemberWithName("__scoop_tag")
    if tag.IsValid():
        number = unsigned(tag)
        for variant in tag.GetType().GetEnumMembers():
            if variant.GetValueAsUnsigned() == number:
                return member(value, variant.GetName())
        raise Unavailable(f"unknown enum tag {number}")
    niche = member(value, "__scoop_niche")
    error = lldb.SBError()
    # Pointer and interface niches use their first pointer word (metadata ABI 9).
    word = niche.GetData().GetUnsignedInt64(error, 0)
    if error.Fail():
        raise Unavailable("enum niche is unavailable")
    for field in fields(value.GetType()):
        if field.GetName() != "__scoop_niche":
            has_payload = field.GetType().GetNumberOfFields() != 0
            if has_payload == (word != 0):
                return member(value, field.GetName())
    raise Unavailable("enum variant is unavailable")


def dynamic_name(value, address):
    target = value.GetTarget()
    if target.GetAddressByteSize() != 8 or target.GetByteOrder() != lldb.eByteOrderLittle:
        raise Unavailable("unsupported runtime layout")
    descriptor = int.from_bytes(read(value, address, 8), "little")
    span = read(value, descriptor + TYPE_NAME_OFFSET, 16)
    data, size = int.from_bytes(span[:8], "little"), int.from_bytes(span[8:], "little")
    name = read(value, data, min(size, TEXT_PREVIEW_BYTES)).decode("utf-8", "replace")
    return name + ("…" if size > TEXT_PREVIEW_BYTES else "")


def summary(value, _internal_dict):
    try:
        value = object_value(value)
        shape = kind(value.GetType())
        if shape == "string":
            return string_summary(value)
        if shape == "array":
            return f"size={length(value)}"
        if shape == "enum":
            return f"{value.GetTypeName()}.{active_variant(value).GetName()}"
        address = (
            unsigned(member(value, "__scoop_object"))
            if shape == "interface"
            else value.GetLoadAddress()
        )
        if address == 0:
            return "null"
        if address == lldb.LLDB_INVALID_ADDRESS:
            return "<unavailable>"
        try:
            return f"{dynamic_name(value, address)} @ 0x{address:x}"
        except Unavailable:
            return f"0x{address:x}"
    except Unavailable as error:
        return f"<{error}>"


class Children:
    def __init__(self, value, _internal_dict):
        self.value = value.GetNonSyntheticValue()

    def update(self):
        # Do not cache SBValues, object addresses or discriminants between stops.
        return False

    def has_children(self):
        return self.num_children() != 0

    def num_children(self, max_count=None):
        try:
            value = object_value(self.value)
            shape = kind(value.GetType())
            if shape == "array":
                count = length(value)
            elif shape == "enum":
                count = int(active_variant(value).IsValid())
            else:
                count = len(self.visible_fields(value, shape))
            return min(count, max_count) if max_count is not None else count
        except Unavailable:
            return 0

    @staticmethod
    def visible_fields(value, shape):
        if shape == "interface":
            return ["__scoop_object"]
        if shape == "string":
            return ["length"]
        return [
            field.GetName()
            for field in fields(value.GetType())
            if not field.GetName().startswith("__scoop_")
        ]

    def get_child_index(self, name):
        try:
            value = object_value(self.value)
            shape = kind(value.GetType())
            if shape == "array" and name.startswith("[") and name.endswith("]"):
                index = int(name[1:-1])
                return index if 0 <= index < length(value) else -1
            if shape == "enum":
                return 0 if name == active_variant(value).GetName() else -1
            return self.visible_fields(value, shape).index(name)
        except (Unavailable, ValueError):
            return -1

    def get_child_at_index(self, index):
        if index < 0:
            return None
        try:
            value = object_value(self.value)
            shape = kind(value.GetType())
            if shape == "array":
                if index >= length(value):
                    return None
                address, element = array_layout(value)
                return value.CreateValueFromAddress(
                    f"[{index}]", address + index * element.GetByteSize(), element
                )
            if shape == "enum":
                return active_variant(value) if index == 0 else None
            names = self.visible_fields(value, shape)
            return member(value, names[index]) if index < len(names) else None
        except Unavailable:
            return None


def __lldb_init_module(debugger, _internal_dict):
    for command in (
        "type category define Scoop",
        f"type summary add -w Scoop -e --recognizer-function -F {__name__}.summary {__name__}.recognizes",
        f"type synthetic add -w Scoop --recognizer-function -l {__name__}.Children {__name__}.recognizes",
        "type category enable Scoop",
    ):
        result = lldb.SBCommandReturnObject()
        debugger.GetCommandInterpreter().HandleCommand(command, result)
        if not result.Succeeded():
            raise RuntimeError(result.GetError())
