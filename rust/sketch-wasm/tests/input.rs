use sketch_core::{Color, PointerKind};
use sketch_wasm::{parse_color, parse_pointer_kind, InputError};

#[test]
fn pen_colors_parse_both_hex_forms_and_round_trip() {
    let cases = [
        (
            "#1A2B3C",
            Color {
                r: 0x1A,
                g: 0x2B,
                b: 0x3C,
                a: 0xFF,
            },
            "#1A2B3C",
        ),
        (
            "#abcdef",
            Color {
                r: 0xAB,
                g: 0xCD,
                b: 0xEF,
                a: 0xFF,
            },
            "#ABCDEF",
        ),
        (
            "#11223380",
            Color {
                r: 0x11,
                g: 0x22,
                b: 0x33,
                a: 0x80,
            },
            "#11223380",
        ),
        (
            "#445566FF",
            Color {
                r: 0x44,
                g: 0x55,
                b: 0x66,
                a: 0xFF,
            },
            "#445566",
        ),
        (
            "#00000000",
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            "#00000000",
        ),
    ];
    for (text, expected, hex) in cases {
        let color = parse_color(text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
        assert_eq!(color, expected, "{text:?} parsed to {color:?}");
        assert_eq!(color.to_hex(), hex, "{text:?} written back");
        assert_eq!(parse_color(hex), Ok(color), "{hex:?} parsed back");
    }
}

#[test]
fn invalid_pen_colors_are_errors_not_defaults() {
    for text in [
        "",
        "red",
        "000000",
        "#",
        "#12345",
        "#1234567",
        "#123456789",
        "#GG0000",
        "#12 345",
        " #123456",
        "#１２３４５６",
    ] {
        let result = parse_color(text);
        assert_eq!(
            result,
            Err(InputError::InvalidColor(text.to_string())),
            "{text:?} gave {result:?}"
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("invalid color {text:?}: expected #RRGGBB or #RRGGBBAA")
        );
    }
}

#[test]
fn pointer_kinds_match_exactly() {
    assert_eq!(parse_pointer_kind("touch"), Ok(PointerKind::Touch));
    assert_eq!(parse_pointer_kind("stylus"), Ok(PointerKind::Stylus));
    for text in ["", "Touch", "STYLUS", "pen", "mouse", " touch"] {
        let result = parse_pointer_kind(text);
        assert_eq!(
            result,
            Err(InputError::InvalidPointerKind(text.to_string())),
            "{text:?} gave {result:?}"
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("invalid pointer kind {text:?}: expected \"touch\" or \"stylus\"")
        );
    }
}
