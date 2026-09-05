"""Read Fontconfig's configured fonts without building or updating its caches."""

import ctypes
import json
import os


def inspect_fonts():
    fc = ctypes.CDLL("libfontconfig.so.1")
    pointer = ctypes.c_void_p
    signatures = {
        "FcInitLoadConfig": (pointer, []),
        "FcConfigGetFontDirs": (pointer, [pointer]),
        "FcStrListNext": (ctypes.c_char_p, [pointer]),
        "FcStrListDone": (None, [pointer]),
        "FcConfigDestroy": (None, [pointer]),
        "FcFreeTypeQuery": (pointer, [ctypes.c_char_p, ctypes.c_int, pointer, ctypes.POINTER(ctypes.c_int)]),
        "FcPatternGetString": (ctypes.c_int, [pointer, ctypes.c_char_p, ctypes.c_int, ctypes.POINTER(ctypes.c_char_p)]),
        "FcPatternGetCharSet": (ctypes.c_int, [pointer, ctypes.c_char_p, ctypes.c_int, ctypes.POINTER(pointer)]),
        "FcCharSetHasChar": (ctypes.c_int, [pointer, ctypes.c_uint]),
        "FcPatternDestroy": (None, [pointer]),
    }
    for name, (result, arguments) in signatures.items():
        function = getattr(fc, name)
        function.restype = result
        function.argtypes = arguments

    # FcInitLoadConfigAndFonts (used by fc-list) can write caches. Load only
    # configuration, then query individual font files directly in memory.
    config = fc.FcInitLoadConfig()
    if not config:
        raise RuntimeError("Fontconfig configuration unavailable")
    directories = fc.FcConfigGetFontDirs(config)
    families = set()
    has_glyph = False
    visited = set()

    def raise_walk_error(error):
        if not isinstance(error, FileNotFoundError):
            raise error

    try:
        while directory := fc.FcStrListNext(directories):
            for current, children, files in os.walk(os.fsdecode(directory), followlinks=True, onerror=raise_walk_error):
                canonical = os.path.realpath(current)
                if canonical in visited:
                    children.clear()
                    continue
                visited.add(canonical)
                for filename in files:
                    font = os.path.join(current, filename)
                    face = 0
                    count = ctypes.c_int(1)
                    while face < count.value:
                        pattern = fc.FcFreeTypeQuery(os.fsencode(font), face, None, ctypes.byref(count))
                        if not pattern:
                            break
                        try:
                            index = 0
                            family = ctypes.c_char_p()
                            while fc.FcPatternGetString(pattern, b"family", index, ctypes.byref(family)) == 0:
                                families.add(family.value.decode("utf-8"))
                                index += 1
                            charset = pointer()
                            if fc.FcPatternGetCharSet(pattern, b"charset", 0, ctypes.byref(charset)) == 0:
                                has_glyph |= bool(fc.FcCharSetHasChar(charset, 0x6708))
                        finally:
                            fc.FcPatternDestroy(pattern)
                        face += 1
    finally:
        fc.FcStrListDone(directories)
        fc.FcConfigDestroy(config)
    return {"families": sorted(families), "hasMoonGlyph": has_glyph}


if __name__ == "__main__":
    print(json.dumps(inspect_fonts()))
