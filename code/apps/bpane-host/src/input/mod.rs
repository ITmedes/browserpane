pub mod keyboard;
pub mod mouse;

use bpane_protocol::InputMessage;

/// Trait for input injection backends.
pub trait InputBackend: Send {
    fn inject(&mut self, msg: &InputMessage) -> anyhow::Result<()>;
}

/// A test input backend that records injected events.
#[derive(Default)]
pub struct TestInputBackend {
    pub events: Vec<InputMessage>,
}

impl InputBackend for TestInputBackend {
    fn inject(&mut self, msg: &InputMessage) -> anyhow::Result<()> {
        self.events.push(msg.clone());
        Ok(())
    }
}

/// Combined injector that routes to mouse + keyboard backends.
pub struct CombinedInjector<M, K> {
    pub mouse: M,
    pub keyboard: K,
}

impl<M: InputBackend, K: InputBackend> InputBackend for CombinedInjector<M, K> {
    fn inject(&mut self, msg: &InputMessage) -> anyhow::Result<()> {
        match msg {
            InputMessage::MouseMove { .. }
            | InputMessage::MouseButton { .. }
            | InputMessage::MouseScroll { .. } => self.mouse.inject(msg),
            InputMessage::KeyEvent { .. } | InputMessage::KeyEventEx { .. } => {
                self.keyboard.inject(msg)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_backend_records_events() {
        let mut backend = TestInputBackend::default();
        let msg = InputMessage::MouseMove { x: 100, y: 200 };
        backend.inject(&msg).unwrap();
        assert_eq!(backend.events.len(), 1);
        assert_eq!(backend.events[0], msg);
    }

    #[test]
    fn test_input_backend_multiple_events() {
        let mut backend = TestInputBackend::default();
        backend
            .inject(&InputMessage::MouseMove { x: 10, y: 20 })
            .unwrap();
        backend
            .inject(&InputMessage::KeyEvent {
                keycode: 30,
                down: true,
                modifiers: 0,
            })
            .unwrap();
        backend
            .inject(&InputMessage::MouseButton {
                button: 0,
                down: true,
                x: 10,
                y: 20,
            })
            .unwrap();
        assert_eq!(backend.events.len(), 3);
    }

    #[test]
    fn test_input_backend_records_key_event_ex() {
        let mut backend = TestInputBackend::default();
        let msg = InputMessage::KeyEventEx {
            keycode: 16,
            down: true,
            modifiers: 0,
            key_char: 0x61, // 'a'
        };
        backend.inject(&msg).unwrap();
        assert_eq!(backend.events.len(), 1);
        assert_eq!(backend.events[0], msg);
    }

    #[test]
    fn test_combined_injector_routes_key_event_ex_to_keyboard() {
        let mut combined = CombinedInjector {
            mouse: TestInputBackend::default(),
            keyboard: TestInputBackend::default(),
        };

        let key_ex = InputMessage::KeyEventEx {
            keycode: 16,
            down: true,
            modifiers: 0,
            key_char: 0x61,
        };
        combined.inject(&key_ex).unwrap();

        assert_eq!(combined.mouse.events.len(), 0);
        assert_eq!(combined.keyboard.events.len(), 1);
        assert_eq!(combined.keyboard.events[0], key_ex);
    }

    #[test]
    fn test_combined_injector_routes_legacy_key_event_to_keyboard() {
        let mut combined = CombinedInjector {
            mouse: TestInputBackend::default(),
            keyboard: TestInputBackend::default(),
        };

        let key = InputMessage::KeyEvent {
            keycode: 30,
            down: true,
            modifiers: 0,
        };
        combined.inject(&key).unwrap();

        assert_eq!(combined.mouse.events.len(), 0);
        assert_eq!(combined.keyboard.events.len(), 1);
        assert_eq!(combined.keyboard.events[0], key);
    }

    #[test]
    fn test_combined_injector_mixed_events() {
        let mut combined = CombinedInjector {
            mouse: TestInputBackend::default(),
            keyboard: TestInputBackend::default(),
        };

        combined
            .inject(&InputMessage::MouseMove { x: 10, y: 20 })
            .unwrap();
        combined
            .inject(&InputMessage::KeyEvent {
                keycode: 30,
                down: true,
                modifiers: 0,
            })
            .unwrap();
        combined
            .inject(&InputMessage::KeyEventEx {
                keycode: 16,
                down: true,
                modifiers: 0,
                key_char: 0x61,
            })
            .unwrap();
        combined
            .inject(&InputMessage::MouseScroll { dx: 0, dy: -3 })
            .unwrap();

        assert_eq!(combined.mouse.events.len(), 2);
        assert_eq!(combined.keyboard.events.len(), 2);
    }
}
