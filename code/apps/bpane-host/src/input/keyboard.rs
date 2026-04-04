use super::InputBackend;
use bpane_protocol::{InputMessage, Modifiers};

#[cfg(target_os = "linux")]
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{self, ConnectionExt as XProtoExt},
        xtest::ConnectionExt as XTestExt,
    },
    rust_connection::RustConnection,
    CURRENT_TIME,
};

/// X11 keycodes for modifier keys (evdev keycode + 8).
#[cfg(target_os = "linux")]
const MODIFIER_X_KEYCODES: [u8; 8] = [
    50,  // ShiftLeft   (evdev 42)
    62,  // ShiftRight  (evdev 54)
    37,  // ControlLeft (evdev 29)
    105, // ControlRight(evdev 97)
    64,  // AltLeft     (evdev 56)
    108, // AltRight    (evdev 100)
    133, // MetaLeft    (evdev 125)
    134, // MetaRight   (evdev 126)
];

#[cfg(target_os = "linux")]
fn is_modifier_x_keycode(x_kc: u8) -> bool {
    MODIFIER_X_KEYCODES.contains(&x_kc)
}

#[cfg(target_os = "linux")]
pub struct KeyboardInjector {
    conn: RustConnection,
    root: u32,
    /// Cached mapping from keysym → (X keycode, shift_required).
    keysym_cache: std::collections::HashMap<u32, (u8, bool)>,
    /// Currently held modifier X11 keycodes, tracked for temporary
    /// release during character injection.
    held_modifiers: Vec<u8>,
    /// Unused X11 keycode reserved for temporary keysym bindings
    /// (characters not present in the host keymap like ö, ä, ü, °).
    scratch_keycode: u8,
    /// Original keysyms for the scratch keycode so the mapping can always
    /// be restored, even if no unused keycode exists on the server.
    scratch_original_keysyms: Vec<u32>,
    /// Server's keysyms-per-keycode, used when binding scratch keysym
    /// to ensure all columns (unshifted + shifted) map to the same keysym.
    keysyms_per_keycode: u8,
    /// Keysym currently bound to the scratch keycode while a synthetic
    /// key press is active. Keeping the mapping stable across press/release
    /// avoids races where clients refresh to the restored keymap too early.
    scratch_active_keysym: Option<u32>,
}

#[cfg(target_os = "linux")]
impl KeyboardInjector {
    pub fn new() -> anyhow::Result<Self> {
        let (conn, screen) = RustConnection::connect(None)?;
        let root = conn.setup().roots[screen].root;
        conn.xtest_get_version(1, 0)?.reply()?;
        let keysym_cache = Self::build_keysym_cache(&conn)?;
        let (scratch_keycode, scratch_original_keysyms, keysyms_per_keycode) =
            Self::find_scratch_keycode(&conn)?;
        Ok(Self {
            conn,
            root,
            keysym_cache,
            held_modifiers: Vec::new(),
            scratch_keycode,
            scratch_original_keysyms,
            keysyms_per_keycode,
            scratch_active_keysym: None,
        })
    }

    /// Build a lookup from keysym → (X keycode, shift_required) by walking
    /// the server's keyboard mapping.
    fn build_keysym_cache(
        conn: &RustConnection,
    ) -> anyhow::Result<std::collections::HashMap<u32, (u8, bool)>> {
        let setup = conn.setup();
        let min_kc = setup.min_keycode;
        let max_kc = setup.max_keycode;
        let reply = conn
            .get_keyboard_mapping(min_kc, max_kc - min_kc + 1)?
            .reply()?;
        let kpk = reply.keysyms_per_keycode as usize;
        let mut map = std::collections::HashMap::new();
        for kc_offset in 0..=(max_kc - min_kc) as usize {
            let kc = min_kc + kc_offset as u8;
            let base = kc_offset * kpk;
            // Column 0 = unshifted, column 1 = shifted
            if base < reply.keysyms.len() {
                let ks0 = reply.keysyms[base];
                if ks0 != 0 {
                    map.entry(ks0).or_insert((kc, false));
                }
            }
            if base + 1 < reply.keysyms.len() {
                let ks1 = reply.keysyms[base + 1];
                if ks1 != 0 {
                    map.entry(ks1).or_insert((kc, true));
                }
            }
        }
        Ok(map)
    }

    /// Find an unused X11 keycode to use as scratch for temporary keysym bindings.
    /// Searches from the top of the keycode range for a completely unmapped keycode.
    /// Returns (scratch_keycode, original_keysyms, keysyms_per_keycode).
    fn find_scratch_keycode(conn: &RustConnection) -> anyhow::Result<(u8, Vec<u32>, u8)> {
        let setup = conn.setup();
        let min_kc = setup.min_keycode;
        let max_kc = setup.max_keycode;
        let reply = conn
            .get_keyboard_mapping(min_kc, max_kc - min_kc + 1)?
            .reply()?;
        let kpk = reply.keysyms_per_keycode;
        let kpk_usize = kpk as usize;
        // Search from top for a keycode with all keysyms = 0 (unused)
        for kc_offset in (0..=(max_kc - min_kc) as usize).rev() {
            let base = kc_offset * kpk_usize;
            let all_zero = (0..kpk_usize)
                .all(|i| base + i >= reply.keysyms.len() || reply.keysyms[base + i] == 0);
            if all_zero {
                return Ok((min_kc + kc_offset as u8, vec![0; kpk_usize], kpk));
            }
        }
        // Fallback: use the highest keycode, but preserve and restore its
        // original mapping so we do not permanently corrupt a real key.
        let base = (max_kc - min_kc) as usize * kpk_usize;
        let mut original = vec![0; kpk_usize];
        for (i, slot) in original.iter_mut().enumerate() {
            if base + i < reply.keysyms.len() {
                *slot = reply.keysyms[base + i];
            }
        }
        Ok((max_kc, original, kpk))
    }

    /// Inject a key event using physical keycode (evdev keycode + 8).
    /// Also tracks modifier key state for release/restore during character injection.
    fn inject_physical(&mut self, keycode: u32, down: bool) -> anyhow::Result<()> {
        // X11 keycodes range 8–255, so evdev keycode must be ≤ 247
        let x_keycode = keycode + 8;
        if x_keycode > 255 {
            return Ok(()); // Out of X11 keycode range, ignore
        }
        let detail = x_keycode as u8;

        // Track modifier state
        if is_modifier_x_keycode(detail) {
            if down {
                if !self.held_modifiers.contains(&detail) {
                    self.held_modifiers.push(detail);
                }
            } else {
                self.held_modifiers.retain(|&kc| kc != detail);
            }
        }

        let evtype = if down {
            xproto::KEY_PRESS_EVENT
        } else {
            xproto::KEY_RELEASE_EVENT
        };
        self.conn
            .xtest_fake_input(evtype, detail, CURRENT_TIME, self.root, 0, 0, 0)?
            .check()?;
        Ok(())
    }

    /// Temporarily release all held modifiers so they don't interfere
    /// with character injection (e.g., Alt held from Mac Option, or Shift
    /// from a DE shifted key that maps to a different position on US).
    fn release_held_modifiers(&self) -> anyhow::Result<()> {
        for &mod_kc in &self.held_modifiers {
            self.conn
                .xtest_fake_input(
                    xproto::KEY_RELEASE_EVENT,
                    mod_kc,
                    CURRENT_TIME,
                    self.root,
                    0,
                    0,
                    0,
                )?
                .check()?;
        }
        Ok(())
    }

    /// Re-press all held modifiers after character injection completes.
    fn restore_held_modifiers(&self) -> anyhow::Result<()> {
        for &mod_kc in &self.held_modifiers {
            self.conn
                .xtest_fake_input(
                    xproto::KEY_PRESS_EVENT,
                    mod_kc,
                    CURRENT_TIME,
                    self.root,
                    0,
                    0,
                    0,
                )?
                .check()?;
        }
        Ok(())
    }

    /// Inject a character by looking up the keysym in the host keymap.
    /// Temporarily releases held modifiers to prevent interference, then
    /// restores them afterwards.
    ///
    /// For keysyms not in the host keymap (e.g., ö, ä, ü, °), uses
    /// XChangeKeyboardMapping to temporarily bind the keysym to a scratch
    /// keycode.
    fn inject_character(&mut self, key_char: u32, down: bool, _keycode: u32) -> anyhow::Result<()> {
        let keysym = unicode_to_keysym(key_char);

        // Release all held modifiers so they don't contaminate the injection.
        // E.g., if Shift is held (DE Shift+2 → '"'), we release it, inject '"'
        // using the US keymap position (Shift+Quote), then restore Shift.
        self.release_held_modifiers()?;

        let inject_result = if let Some(&(x_keycode, need_shift)) = self.keysym_cache.get(&keysym) {
            if down {
                if need_shift {
                    self.conn
                        .xtest_fake_input(
                            xproto::KEY_PRESS_EVENT,
                            50, // ShiftLeft
                            CURRENT_TIME,
                            self.root,
                            0,
                            0,
                            0,
                        )?
                        .check()?;
                }
                self.conn
                    .xtest_fake_input(
                        xproto::KEY_PRESS_EVENT,
                        x_keycode,
                        CURRENT_TIME,
                        self.root,
                        0,
                        0,
                        0,
                    )?
                    .check()?;
                if need_shift {
                    // Release our own Shift immediately — we only needed it for this keypress.
                    self.conn
                        .xtest_fake_input(
                            xproto::KEY_RELEASE_EVENT,
                            50,
                            CURRENT_TIME,
                            self.root,
                            0,
                            0,
                            0,
                        )?
                        .check()?;
                }
                Ok(())
            } else {
                self.conn
                    .xtest_fake_input(
                        xproto::KEY_RELEASE_EVENT,
                        x_keycode,
                        CURRENT_TIME,
                        self.root,
                        0,
                        0,
                        0,
                    )?
                    .check()?;
                Ok(())
            }
        } else {
            // Keysym not in host keymap (e.g., ö ä ü ° on US layout).
            // Temporarily bind it to the scratch keycode via XChangeKeyboardMapping.
            self.inject_via_scratch(keysym, down)
        };

        // Always restore held modifiers, even if injection failed.
        let restore_result = self.restore_held_modifiers();

        inject_result?;
        restore_result?;
        Ok(())
    }

    /// Inject a keysym that isn't present in the host keymap by temporarily
    /// binding it to the scratch keycode via XChangeKeyboardMapping.
    ///
    /// The X server delivers MappingNotify before the KeyPress in the client's
    /// event stream, so the application will see the correct keysym.
    ///
    /// We fill ALL columns (keysyms_per_keycode) with the same keysym so the
    /// character is produced regardless of Shift state in the X11 modifier mask
    /// (prevents stale shifted-column reads that caused § → Ö).
    fn bind_scratch_keysym(&mut self, keysym: u32) -> anyhow::Result<()> {
        if self.scratch_active_keysym == Some(keysym) {
            return Ok(());
        }

        if self.scratch_active_keysym.is_some() {
            self.restore_scratch_keymap()?;
        }

        let kpk = self.keysyms_per_keycode;
        let keysyms: Vec<u32> = vec![keysym; kpk as usize];
        self.conn
            .change_keyboard_mapping(1, self.scratch_keycode, kpk, &keysyms)?
            .check()?;
        self.scratch_active_keysym = Some(keysym);
        Ok(())
    }

    fn restore_scratch_keymap(&mut self) -> anyhow::Result<()> {
        if self.scratch_active_keysym.is_none() {
            return Ok(());
        }

        self.conn
            .change_keyboard_mapping(
                1,
                self.scratch_keycode,
                self.keysyms_per_keycode,
                &self.scratch_original_keysyms,
            )?
            .check()?;
        self.scratch_active_keysym = None;
        Ok(())
    }

    fn inject_via_scratch(&mut self, keysym: u32, down: bool) -> anyhow::Result<()> {
        self.bind_scratch_keysym(keysym)?;

        let evtype = if down {
            xproto::KEY_PRESS_EVENT
        } else {
            xproto::KEY_RELEASE_EVENT
        };
        self.conn
            .xtest_fake_input(
                evtype,
                self.scratch_keycode,
                CURRENT_TIME,
                self.root,
                0,
                0,
                0,
            )?
            .check()?;

        if !down {
            self.restore_scratch_keymap()?;
        }

        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for KeyboardInjector {
    fn drop(&mut self) {
        let _ = self.restore_scratch_keymap();
    }
}

/// Convert a Unicode codepoint to an X11 keysym.
/// For ASCII printable (U+0020–U+007E), keysym == codepoint.
/// For Latin-1 (U+00A0–U+00FF), keysym == codepoint.
/// For other Unicode, keysym = 0x01000000 + codepoint.
fn unicode_to_keysym(codepoint: u32) -> u32 {
    if (0x0020..=0x007E).contains(&codepoint) || (0x00A0..=0x00FF).contains(&codepoint) {
        codepoint
    } else {
        0x0100_0000 + codepoint
    }
}

/// Returns true if the key event should use physical keycode injection
/// rather than character injection. This is the case for modifier combos
/// (Ctrl+C, Meta+V, etc.) where the character produced is a control code.
/// When AltGr is active, always use character injection since AltGr produces
/// printable characters (e.g., @, €, {, }) that need keysym lookup.
pub(crate) fn should_use_physical(modifiers: u8, key_char: u32) -> bool {
    let has_altgr = modifiers & Modifiers::ALTGR != 0;
    if has_altgr {
        return false;
    }
    let has_ctrl = modifiers & Modifiers::CTRL != 0;
    let has_meta = modifiers & Modifiers::META != 0;
    // Control characters (< 0x20) or modifier combos with Ctrl/Meta
    key_char < 0x20 || has_ctrl || has_meta
}

#[cfg(target_os = "linux")]
impl InputBackend for KeyboardInjector {
    fn inject(&mut self, msg: &InputMessage) -> anyhow::Result<()> {
        match msg {
            InputMessage::KeyEvent { keycode, down, .. } => {
                self.inject_physical(*keycode, *down)?;
            }
            InputMessage::KeyEventEx {
                keycode,
                down,
                modifiers,
                key_char,
            } => {
                let use_physical = *key_char == 0 || should_use_physical(*modifiers, *key_char);
                if !use_physical {
                    self.inject_character(*key_char, *down, *keycode)?;
                } else {
                    self.inject_physical(*keycode, *down)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

// Non-Linux stub
#[cfg(not(target_os = "linux"))]
pub struct KeyboardInjector;
#[cfg(not(target_os = "linux"))]
impl KeyboardInjector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }
}
#[cfg(not(target_os = "linux"))]
impl InputBackend for KeyboardInjector {
    fn inject(&mut self, _msg: &InputMessage) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_to_keysym_ascii() {
        assert_eq!(unicode_to_keysym(b'a' as u32), 0x61);
        assert_eq!(unicode_to_keysym(b'Z' as u32), 0x5A);
        assert_eq!(unicode_to_keysym(b' ' as u32), 0x20);
        assert_eq!(unicode_to_keysym(b'~' as u32), 0x7E);
    }

    #[test]
    fn unicode_to_keysym_non_ascii() {
        // é = U+00E9
        assert_eq!(unicode_to_keysym(0x00E9), 0x00E9);
        // À = U+00C0
        assert_eq!(unicode_to_keysym(0x00C0), 0x00C0);
        // € = U+20AC
        assert_eq!(unicode_to_keysym(0x20AC), 0x0100_20AC);
    }

    #[test]
    fn unicode_to_keysym_german_chars() {
        assert_eq!(unicode_to_keysym(0x00F6), 0x00F6); // ö
        assert_eq!(unicode_to_keysym(0x00E4), 0x00E4); // ä
        assert_eq!(unicode_to_keysym(0x00FC), 0x00FC); // ü
        assert_eq!(unicode_to_keysym(0x00B0), 0x00B0); // °
        assert_eq!(unicode_to_keysym(0x00DF), 0x00DF); // ß
    }

    #[test]
    fn unicode_to_keysym_dead_key_targets() {
        assert_eq!(unicode_to_keysym(0x00F4), 0x00F4); // ô
        assert_eq!(unicode_to_keysym(0x00F3), 0x00F3); // ó
        assert_eq!(unicode_to_keysym(0x00E1), 0x00E1); // á
    }

    #[test]
    fn should_use_physical_ctrl_combos() {
        // Ctrl+C produces key_char 0x03 (control character)
        assert!(should_use_physical(Modifiers::CTRL, 0x03));
        // Ctrl held with printable char
        assert!(should_use_physical(Modifiers::CTRL, b'c' as u32));
        // Meta combos
        assert!(should_use_physical(Modifiers::META, b'v' as u32));
    }

    #[test]
    fn should_use_physical_regular_chars() {
        // Normal printable character, no modifier
        assert!(!should_use_physical(0, b'a' as u32));
        // Shift is fine for character injection
        assert!(!should_use_physical(Modifiers::SHIFT, b'A' as u32));
        // Alt is fine (AltGr)
        assert!(!should_use_physical(Modifiers::ALT, 0x20AC)); // €
    }

    #[test]
    fn should_use_physical_altgr_always_character() {
        // AltGr+Q → '@' on DE layout: must use character injection
        assert!(!should_use_physical(
            Modifiers::ALTGR | Modifiers::ALT,
            b'@' as u32
        ));
        // AltGr+E → '€': must use character injection
        assert!(!should_use_physical(
            Modifiers::ALTGR | Modifiers::ALT,
            0x20AC
        ));
        // AltGr with Ctrl bit (Windows sends fake Ctrl+Alt for AltGr):
        // client strips Ctrl, but even if both are set, AltGr takes priority
        assert!(!should_use_physical(
            Modifiers::ALTGR | Modifiers::CTRL | Modifiers::ALT,
            b'{' as u32
        ));
        // AltGr with control character should still use character injection
        // (AltGr overrides the Ctrl heuristic)
        assert!(!should_use_physical(Modifiers::ALTGR, 0x03));
    }

    #[test]
    fn should_use_physical_german_special_chars() {
        // ö, ä, ü: direct key presses with no modifiers — character injection
        assert!(!should_use_physical(0, 0x00F6)); // ö
        assert!(!should_use_physical(0, 0x00E4)); // ä
        assert!(!should_use_physical(0, 0x00FC)); // ü
                                                  // ° with Shift — still character injection
        assert!(!should_use_physical(Modifiers::SHIFT, 0x00B0)); // °
                                                                 // " with Shift — character injection (Shift+2 on DE = '"')
        assert!(!should_use_physical(Modifiers::SHIFT, b'"' as u32));
        // = with Shift — character injection (Shift+0 on DE = '=')
        assert!(!should_use_physical(Modifiers::SHIFT, b'=' as u32));
    }

    #[test]
    fn should_use_physical_mac_option_chars() {
        // Mac Option+L → '@': client sends ALTGR flag
        assert!(!should_use_physical(Modifiers::ALTGR, b'@' as u32));
        // Mac Option+7 → '|': client sends ALTGR flag
        assert!(!should_use_physical(Modifiers::ALTGR, b'|' as u32));
        // Mac Option+Shift+7 → '\': client sends ALTGR|SHIFT
        assert!(!should_use_physical(
            Modifiers::ALTGR | Modifiers::SHIFT,
            b'\\' as u32
        ));
    }

    // ── Modifier tracking ────────────────────────────────────────────

    #[cfg(target_os = "linux")]
    #[test]
    fn modifier_x_keycode_detection() {
        assert!(is_modifier_x_keycode(50)); // ShiftLeft
        assert!(is_modifier_x_keycode(62)); // ShiftRight
        assert!(is_modifier_x_keycode(37)); // ControlLeft
        assert!(is_modifier_x_keycode(105)); // ControlRight
        assert!(is_modifier_x_keycode(64)); // AltLeft
        assert!(is_modifier_x_keycode(108)); // AltRight
        assert!(is_modifier_x_keycode(133)); // MetaLeft
        assert!(is_modifier_x_keycode(134)); // MetaRight
        assert!(!is_modifier_x_keycode(38)); // KeyA — not a modifier
        assert!(!is_modifier_x_keycode(11)); // Digit2 — not a modifier
    }

    // ── Keycode range validation ─────────────────────────────────────

    #[test]
    fn x11_keycode_range_valid() {
        // Evdev keycode 0 → X11 keycode 8 (valid)
        let x_keycode = 0u32 + 8;
        assert!(x_keycode <= 255);

        // Evdev keycode 247 → X11 keycode 255 (max valid)
        let x_keycode = 247u32 + 8;
        assert_eq!(x_keycode, 255);
        assert!(x_keycode <= 255);
    }

    #[test]
    fn x11_keycode_range_overflow() {
        // Evdev keycode 248 → X11 keycode 256 (out of range, must be rejected)
        let x_keycode = 248u32 + 8;
        assert!(x_keycode > 255);

        // Evdev keycode 300 → X11 keycode 308 (out of range)
        let x_keycode = 300u32 + 8;
        assert!(x_keycode > 255);

        // Before fix: `(keycode as u8).saturating_add(8)` would truncate 256→0,
        // then 0+8=8, injecting keycode 8 instead of rejecting.
        // Keycode 248: (248 as u8) = 248, 248.saturating_add(8) = 255 (wrong keycode!)
        // The correct X11 keycode for evdev 248 would be 256, which is invalid.
        let bad_detail = (248u32 as u8).saturating_add(8);
        assert_eq!(bad_detail, 255); // Wrong! Should be rejected, not mapped to 255
    }
}
