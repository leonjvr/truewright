//! The small named-key table `Session::press` supports (browser-actions
//! spec: "Key press"). Real per-keystroke synthesis with timing/curves is
//! Phase 2's human-motion engine, not this.

pub struct KeySpec {
    pub key: &'static str,
    pub code: &'static str,
    pub windows_virtual_key_code: i64,
}

pub fn lookup(name: &str) -> Option<KeySpec> {
    let spec = match name {
        "Enter" => KeySpec {
            key: "Enter",
            code: "Enter",
            windows_virtual_key_code: 13,
        },
        "Tab" => KeySpec {
            key: "Tab",
            code: "Tab",
            windows_virtual_key_code: 9,
        },
        "Escape" => KeySpec {
            key: "Escape",
            code: "Escape",
            windows_virtual_key_code: 27,
        },
        "ArrowDown" => KeySpec {
            key: "ArrowDown",
            code: "ArrowDown",
            windows_virtual_key_code: 40,
        },
        "ArrowUp" => KeySpec {
            key: "ArrowUp",
            code: "ArrowUp",
            windows_virtual_key_code: 38,
        },
        "Backspace" => KeySpec {
            key: "Backspace",
            code: "Backspace",
            windows_virtual_key_code: 8,
        },
        _ => return None,
    };
    Some(spec)
}
