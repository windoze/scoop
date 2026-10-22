use super::kind::Kind;

struct Entry {
    canonical: &'static str,
    kind: Kind,
    aliases: &'static str,
}

const ENCODINGS: &[Entry] = &[
    Entry {
        canonical: "BIG5",
        kind: Kind::Rs(encoding_rs::BIG5),
        aliases: "BIG-5|BIG-FIVE|BIG5|BIGFIVE|CN-BIG5|CSBIG5",
    },
    Entry {
        canonical: "CP1250",
        kind: Kind::Rs(encoding_rs::WINDOWS_1250),
        aliases: "CP1250|MS-EE|WINDOWS-1250",
    },
    Entry {
        canonical: "CP1251",
        kind: Kind::Rs(encoding_rs::WINDOWS_1251),
        aliases: "CP1251|MS-CYRL|WINDOWS-1251",
    },
    Entry {
        canonical: "CP1252",
        kind: Kind::Rs(encoding_rs::WINDOWS_1252),
        aliases: "CP1252|MS-ANSI|WINDOWS-1252",
    },
    Entry {
        canonical: "CP1253",
        kind: Kind::Rs(encoding_rs::WINDOWS_1253),
        aliases: "CP1253|MS-GREEK|WINDOWS-1253",
    },
    Entry {
        canonical: "CP1254",
        kind: Kind::Rs(encoding_rs::WINDOWS_1254),
        aliases: "CP1254|MS-TURK|WINDOWS-1254",
    },
    Entry {
        canonical: "CP1255",
        kind: Kind::Rs(encoding_rs::WINDOWS_1255),
        aliases: "CP1255|MS-HEBR|WINDOWS-1255",
    },
    Entry {
        canonical: "CP1256",
        kind: Kind::Rs(encoding_rs::WINDOWS_1256),
        aliases: "CP1256|MS-ARAB|WINDOWS-1256",
    },
    Entry {
        canonical: "CP1257",
        kind: Kind::Rs(encoding_rs::WINDOWS_1257),
        aliases: "CP1257|WINBALTRIM|WINDOWS-1257",
    },
    Entry {
        canonical: "CP1258",
        kind: Kind::Rs(encoding_rs::WINDOWS_1258),
        aliases: "CP1258|WINDOWS-1258",
    },
    Entry {
        canonical: "CP866",
        kind: Kind::Rs(encoding_rs::IBM866),
        aliases: "866|CP866|CSIBM866|IBM866",
    },
    Entry {
        canonical: "CP874",
        kind: Kind::Rs(encoding_rs::WINDOWS_874),
        aliases: "CP874|WINDOWS-874",
    },
    Entry {
        canonical: "EUC-JP",
        kind: Kind::Rs(encoding_rs::EUC_JP),
        aliases: "CSEUCPKDFMTJAPANESE|EUC-JP|EUCJP|EXTENDED_UNIX_CODE_PACKED_FORMAT_FOR_JAPANESE",
    },
    Entry {
        canonical: "EUC-KR",
        kind: Kind::Rs(encoding_rs::EUC_KR),
        aliases: "CSEUCKR|EUC-KR|EUCKR",
    },
    Entry {
        canonical: "GB18030",
        kind: Kind::Rs(encoding_rs::GB18030),
        aliases: "GB18030",
    },
    Entry {
        canonical: "GBK",
        kind: Kind::Rs(encoding_rs::GBK),
        aliases: "GBK",
    },
    Entry {
        canonical: "ISO-2022-JP",
        kind: Kind::Rs(encoding_rs::ISO_2022_JP),
        aliases: "CSISO2022JP|ISO-2022-JP",
    },
    Entry {
        canonical: "ISO-8859-1",
        kind: Kind::Latin1,
        aliases: "CP819|CSISOLATIN1|IBM819|ISO-8859-1|ISO-IR-100|ISO8859-1|ISO_8859-1|ISO_8859-1:1987|L1|LATIN1",
    },
    Entry {
        canonical: "ISO-8859-10",
        kind: Kind::Rs(encoding_rs::ISO_8859_10),
        aliases: "CSISOLATIN6|ISO-8859-10|ISO-IR-157|ISO8859-10|ISO_8859-10|ISO_8859-10:1992|L6|LATIN6",
    },
    Entry {
        canonical: "ISO-8859-13",
        kind: Kind::Rs(encoding_rs::ISO_8859_13),
        aliases: "ISO-8859-13|ISO-IR-179|ISO8859-13|ISO_8859-13|L7|LATIN7",
    },
    Entry {
        canonical: "ISO-8859-14",
        kind: Kind::Rs(encoding_rs::ISO_8859_14),
        aliases: "ISO-8859-14|ISO-CELTIC|ISO-IR-199|ISO8859-14|ISO_8859-14|ISO_8859-14:1998|L8|LATIN8",
    },
    Entry {
        canonical: "ISO-8859-15",
        kind: Kind::Rs(encoding_rs::ISO_8859_15),
        aliases: "ISO-8859-15|ISO-IR-203|ISO8859-15|ISO_8859-15|ISO_8859-15:1998|LATIN-9",
    },
    Entry {
        canonical: "ISO-8859-16",
        kind: Kind::Rs(encoding_rs::ISO_8859_16),
        aliases: "ISO-8859-16|ISO-IR-226|ISO8859-16|ISO_8859-16|ISO_8859-16:2001|L10|LATIN10",
    },
    Entry {
        canonical: "ISO-8859-2",
        kind: Kind::Rs(encoding_rs::ISO_8859_2),
        aliases: "CSISOLATIN2|ISO-8859-2|ISO-IR-101|ISO8859-2|ISO_8859-2|ISO_8859-2:1987|L2|LATIN2",
    },
    Entry {
        canonical: "ISO-8859-3",
        kind: Kind::Rs(encoding_rs::ISO_8859_3),
        aliases: "CSISOLATIN3|ISO-8859-3|ISO-IR-109|ISO8859-3|ISO_8859-3|ISO_8859-3:1988|L3|LATIN3",
    },
    Entry {
        canonical: "ISO-8859-4",
        kind: Kind::Rs(encoding_rs::ISO_8859_4),
        aliases: "CSISOLATIN4|ISO-8859-4|ISO-IR-110|ISO8859-4|ISO_8859-4|ISO_8859-4:1988|L4|LATIN4",
    },
    Entry {
        canonical: "ISO-8859-5",
        kind: Kind::Rs(encoding_rs::ISO_8859_5),
        aliases: "CSISOLATINCYRILLIC|CYRILLIC|ISO-8859-5|ISO-IR-144|ISO8859-5|ISO_8859-5|ISO_8859-5:1988",
    },
    Entry {
        canonical: "ISO-8859-6",
        kind: Kind::Rs(encoding_rs::ISO_8859_6),
        aliases: "ARABIC|ASMO-708|CSISOLATINARABIC|ECMA-114|ISO-8859-6|ISO-IR-127|ISO8859-6|ISO_8859-6|ISO_8859-6:1987",
    },
    Entry {
        canonical: "ISO-8859-7",
        kind: Kind::Rs(encoding_rs::ISO_8859_7),
        aliases: "CSISOLATINGREEK|ECMA-118|ELOT_928|GREEK|GREEK8|ISO-8859-7|ISO-IR-126|ISO8859-7|ISO_8859-7|ISO_8859-7:1987|ISO_8859-7:2003",
    },
    Entry {
        canonical: "ISO-8859-8",
        kind: Kind::Rs(encoding_rs::ISO_8859_8),
        aliases: "CSISOLATINHEBREW|HEBREW|ISO-8859-8|ISO-IR-138|ISO8859-8|ISO_8859-8|ISO_8859-8:1988",
    },
    Entry {
        canonical: "KOI8-R",
        kind: Kind::Rs(encoding_rs::KOI8_R),
        aliases: "CSKOI8R|KOI8-R",
    },
    Entry {
        canonical: "KOI8-U",
        kind: Kind::Rs(encoding_rs::KOI8_U),
        aliases: "KOI8-U",
    },
    Entry {
        canonical: "MACCYRILLIC",
        kind: Kind::Rs(encoding_rs::X_MAC_CYRILLIC),
        aliases: "MACCYRILLIC",
    },
    Entry {
        canonical: "MACROMAN",
        kind: Kind::Rs(encoding_rs::MACINTOSH),
        aliases: "CSMACINTOSH|MAC|MACINTOSH|MACROMAN",
    },
    Entry {
        canonical: "SHIFT_JIS",
        kind: Kind::Rs(encoding_rs::SHIFT_JIS),
        aliases: "CSSHIFTJIS|MS_KANJI|SHIFT-JIS|SHIFT_JIS|SJIS",
    },
    Entry {
        canonical: "US-ASCII",
        kind: Kind::Ascii,
        aliases: "ANSI_X3.4-1968|ANSI_X3.4-1986|ASCII|CP367|CSASCII|IBM367|ISO-IR-6|ISO646-US|ISO_646.IRV:1991|US|US-ASCII",
    },
    Entry {
        canonical: "UTF-16",
        kind: Kind::Utf16(0),
        aliases: "UTF-16|UTF16",
    },
    Entry {
        canonical: "UTF-16BE",
        kind: Kind::Utf16(1),
        aliases: "UTF-16BE|UTF16BE",
    },
    Entry {
        canonical: "UTF-16LE",
        kind: Kind::Utf16(2),
        aliases: "UTF-16LE|UTF16LE",
    },
    Entry {
        canonical: "UTF-32",
        kind: Kind::Utf32(0),
        aliases: "UTF-32|UTF32",
    },
    Entry {
        canonical: "UTF-32BE",
        kind: Kind::Utf32(1),
        aliases: "UTF-32BE|UTF32BE",
    },
    Entry {
        canonical: "UTF-32LE",
        kind: Kind::Utf32(2),
        aliases: "UTF-32LE|UTF32LE",
    },
    Entry {
        canonical: "UTF-8",
        kind: Kind::Utf8,
        aliases: "UTF-8|UTF8",
    },
];

pub fn lookup(name: &str) -> Option<(&'static str, Kind)> {
    ENCODINGS
        .iter()
        .find(|entry| {
            entry
                .aliases
                .split('|')
                .any(|alias| alias.eq_ignore_ascii_case(name))
        })
        .map(|entry| (entry.canonical, entry.kind))
}

pub fn names() -> String {
    ENCODINGS
        .iter()
        .flat_map(|entry| entry.aliases.split('|'))
        .collect::<Vec<_>>()
        .join("\n")
}
