use std::collections::HashMap;
use std::sync::OnceLock;

// Static keymap — built exactly once on first keystroke, never rebuilt.
// Eliminates ~50 HashMap::insert() allocations per keypress.
static KEYMAP: OnceLock<HashMap<u16, u64>> = OnceLock::new();

pub fn get_key_map() -> &'static HashMap<u16, u64> {
    KEYMAP.get_or_init(|| {
        let standard: &[(u16, u64)] = &[
            (53, 1),   // Escape
            (122, 59), // F1
            (120, 60), // F2
            (99, 61),  // F3
            (118, 62), // F4
            (96, 63),  // F5
            (97, 64),  // F6
            (98, 65),  // F7
            (100, 66), // F8
            (101, 67), // F9
            (109, 68), // F10
            (103, 87), // F11
            (111, 88), // F12
            (50, 41),  // BackQuote
            (18, 2),   // 1
            (19, 3),   // 2
            (20, 4),   // 3
            (21, 5),   // 4
            (23, 6),   // 5
            (22, 7),   // 6
            (26, 8),   // 7
            (28, 9),   // 8
            (25, 10),  // 9
            (29, 11),  // 0
            (27, 12),  // Minus
            (24, 13),  // Equal
            (51, 14),  // Backspace
            (48, 15),  // Tab
            (57, 58),  // CapsLock
            (0, 30),   // A
            (11, 48),  // B
            (8, 46),   // C
            (2, 32),   // D
            (14, 18),  // E
            (3, 33),   // F
            (5, 34),   // G
            (4, 35),   // H
            (34, 23),  // I
            (38, 36),  // J
            (40, 37),  // K
            (37, 38),  // L
            (46, 50),  // M
            (45, 49),  // N
            (31, 24),  // O
            (35, 25),  // P
            (12, 16),  // Q
            (15, 19),  // R
            (1, 31),   // S
            (17, 20),  // T
            (32, 22),  // U
            (9, 47),   // V
            (13, 17),  // W
            (7, 45),   // X
            (16, 21),  // Y
            (6, 44),   // Z
            (33, 26),  // LeftBracket
            (30, 27),  // RightBracket
            (42, 43),  // BackSlash
            (41, 39),  // SemiColon
            (39, 40),  // Quote
            (36, 28),  // Return
            (43, 51),  // Comma
            (47, 52),  // Dot
            (44, 53),  // Slash
            (49, 57),  // Space
            (114, 3666), // Insert (Help on mac)
            (117, 3667), // Delete (Forward Delete)
            (115, 3655), // Home
            (119, 3663), // End
            (116, 3657), // PageUp
            (121, 3665), // PageDown
            (126, 57416),// UpArrow
            (123, 57419),// LeftArrow
            (124, 57421),// RightArrow
            (125, 57424),// DownArrow
            (56, 42),  // ShiftLeft
            (60, 54),  // ShiftRight
            (59, 29),  // ControlLeft
            (62, 3613),// ControlRight
            (58, 56),  // AltLeft (Option)
            (61, 3640),// AltRight
            (55, 91),  // CommandLeft
            (54, 92),  // CommandRight
            (71, 69),  // NumLock (Clear)
            (75, 3637),// KpDivide
            (67, 55),  // KpMultiply
            (78, 74),  // KpMinus
            (69, 78),  // KpPlus
            (76, 3612),// KpReturn
            (82, 82),  // Kp0
            (83, 79),  // Kp1
            (84, 80),  // Kp2
            (85, 81),  // Kp3
            (86, 75),  // Kp4
            (87, 76),  // Kp5
            (88, 77),  // Kp6
            (89, 71),  // Kp7
            (91, 72),  // Kp8
            (92, 73),  // Kp9
            (65, 52),  // KpDecimal
        ];
        let mut map = HashMap::with_capacity(standard.len());
        for &(cg_code, mech_code) in standard {
            map.insert(cg_code, mech_code);
        }
        map
    })
}
