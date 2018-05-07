use models::*;
use protocol;

use std::os::unix::net::UnixStream;
use std::io::prelude::*;
use std::io::{BufReader};

pub trait XBufferedWriter {
    fn write_sequence(&mut self) -> u16;
    fn write_pad(&mut self, len: usize);
    fn write_bool(&mut self, input: bool);
    fn write_u8(&mut self, input: u8);
    fn write_i16(&mut self, input: i16);
    fn write_u16(&mut self, input: u16);
    fn write_i32(&mut self, input: i32);
    fn write_u32(&mut self, input: u32);
    fn write_val_bool(&mut self, input: bool);
    fn write_val_u8(&mut self, input: u8);
    fn write_val_i16(&mut self, input: i16);
    fn write_val_u16(&mut self, input: u16);
    fn write_val_i32(&mut self, input: i32);
    fn write_val_u32(&mut self, input: u32);
    fn write_val(&mut self, input: u32);
    fn write_values<T: Value>(&mut self, values: &Vec<T>);
}

pub trait XBufferedReader {
    fn read_pad(&mut self, len: usize);
    fn read_bool(&mut self) -> bool;
    fn read_u8(&mut self) -> u8;
    fn read_i16(&mut self) -> i16;
    fn read_u16(&mut self) -> u16;
    fn read_u32(&mut self) -> u32;
    fn read_char(&mut self) -> char;
    fn read_str(&mut self, len: usize) -> String;
}

pub struct XReadHelper {
    buf_in: BufReader<UnixStream>,
    buf_one_byte: Vec<u8>,
    buf_two_byte: Vec<u8>,
    buf_four_byte: Vec<u8>
}

impl XReadHelper {
    pub fn new(buf_in: BufReader<UnixStream>) -> XReadHelper {
        XReadHelper {
            buf_in,
            buf_one_byte: vec![0u8; 1],
            buf_two_byte: vec![0u8; 2],
            buf_four_byte: vec![0u8; 4]
        }
    }
}

impl XReadHelper {
    /** Reads an error from the server (assumes first byte read) */
    pub fn read_error(&mut self, code: u8) -> Option<ServerError> {
        let info = self.read_u32(); // Always u32 or unused
        let minor_opcode = self.read_u16();
        let major_opcode = self.read_u8();
        self.read_pad(21);

        match code {
            protocol::ERROR_REQUEST => Some(ServerError::Request { minor_opcode, major_opcode }),
            protocol::ERROR_VALUE => Some(ServerError::Value { minor_opcode, major_opcode, bad_value: info }),
            protocol::ERROR_WINDOW => Some(ServerError::Window { minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_PIXMAP => Some(ServerError::Pixmap { minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_ATOM => Some(ServerError::Atom { minor_opcode, major_opcode, bad_atom_id: info }),
            protocol::ERROR_CURSOR => Some(ServerError::Cursor { minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_FONT => Some(ServerError::Font { minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_MATCH => Some(ServerError::Match { minor_opcode, major_opcode }),
            protocol::ERROR_DRAWABLE => Some(ServerError::Drawable { minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_ACCESS => Some(ServerError::Access { minor_opcode, major_opcode }),
            protocol::ERROR_ALLOC => Some(ServerError::Alloc { minor_opcode, major_opcode }),
            protocol::ERROR_COLORMAP => Some(ServerError::Colormap { minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_G_CONTEXT => Some(ServerError::GContext { minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_ID_CHOICE => Some(ServerError::IDChoice { minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_NAME => Some(ServerError::Name { minor_opcode, major_opcode }),
            protocol::ERROR_LENGTH => Some(ServerError::Length { minor_opcode, major_opcode }),
            protocol::ERROR_IMPLEMENTATION => Some(ServerError::Implementation { minor_opcode, major_opcode }),
            _ => None
        }
    }

    /** Reads a generic pointer event (assumes first byte read) and returns the results. This also reads the extra padding byte at the end, if there is one
     * Returns detail, sequence_number, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen, extra
     */
    pub fn read_pointer_event(&mut self) -> (u32, u32, u32, u32, i16, i16, i16, i16, u16, bool, u8) {
        (
            self.read_u32(),
            self.read_u32(),
            self.read_u32(),
            self.read_u32(),
            self.read_i16(),
            self.read_i16(),
            self.read_i16(),
            self.read_i16(),
            self.read_u16(),
            self.read_bool(),
            self.read_u8()
        )
    }

    /** Reads a generic pointer event (assumes first byte read) and returns the results. This also reads the extra padding byte at the end, if there is one
     * Returns detail, sequence_number, time, root, event, child, root_x, root_y, event_x, event_y, state, mode, [same_screen+focus byte]
     */
    pub fn read_pointer_event_with_mode(&mut self) -> (u32, u32, u32, u32, i16, i16, i16, i16, u16, u8, u8) {
        (
            self.read_u32(),
            self.read_u32(),
            self.read_u32(),
            self.read_u32(),
            self.read_i16(),
            self.read_i16(),
            self.read_i16(),
            self.read_i16(),
            self.read_u16(),
            self.read_u8(),
            self.read_u8()
        )
    }

    /** Reads a generic focu sevent (assumes first byte read) and returns the results. Also reads the padding. */
    pub fn read_focus_event(&mut self) -> (u32, u8, ()) {
        (
            self.read_u32(),
            self.read_u8(),
            self.read_pad(23)
        )
    }

    /** Reads a key press from the server (assumes first byte read) */
    pub fn read_key_press(&mut self, key_code: u8) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, same_screen, _)
            = self.read_pointer_event();
        let state = KeyButton::get(state_pre);
        Some(ServerEvent::KeyPress { key_code, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen })
    }

    /** Reads a key release from the server (assumes first byte read) */
    pub fn read_key_release(&mut self, key_code: u8) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, same_screen, _)
            = self.read_pointer_event();
        let state = KeyButton::get(state_pre);
        Some(ServerEvent::KeyRelease { key_code, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen })
    }

    /** Reads a button press from the server (assumes first byte read) */
    pub fn read_button_press(&mut self, button: u8) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, same_screen, _)
            = self.read_pointer_event();
        let state = KeyButton::get(state_pre);
        Some(ServerEvent::ButtonPress { button, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen })
    }

    /** Reads a button release from the server (assumes first byte read) */
    pub fn read_button_release(&mut self, button: u8) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, same_screen, _)
            = self.read_pointer_event();
        let state = KeyButton::get(state_pre);
        Some(ServerEvent::ButtonRelease { button, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen })
    }

    /** Reads a motion notify from the server (assumes first byte read) */
    pub fn read_motion_notify(&mut self, detail_pre: u8) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, same_screen, _)
            = self.read_pointer_event();
        let detail = match MotionNotifyType::get(detail_pre) {
            Some(x) => x,
            None => return None
        };
        let state = KeyButton::get(state_pre);
        Some(ServerEvent::MotionNotify { detail, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen })
    }

    /** Reads a motion notify from the server (assumes first byte read) */
    pub fn read_enter_notify(&mut self, detail_pre: u8) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, mode_pre, extra)
            = self.read_pointer_event_with_mode();
        let detail = match NotifyType::get(detail_pre) {
            Some(x) => x,
            None => return None
        };
        let state = KeyButton::get(state_pre);
        let mode = match NotifyMode::get(mode_pre) {
            Some(x) => x,
            None => return None
        };
        let (same_screen, focus) = match extra {
            0x01 => (true, false),
            0x02 => (false, true),
            0x03 => (true, true),
            _ => (false, false)
        };
        Some(ServerEvent::EnterNotify { detail, time, root, event, child, root_x, root_y, event_x, event_y, state, mode, same_screen, focus })
    }

    /** Reads a leave notify from the server (assumes first byte read) */
    pub fn read_leave_notify(&mut self, detail_pre: u8) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, mode_pre, extra)
            = self.read_pointer_event_with_mode();
        let detail = match NotifyType::get(detail_pre) {
            Some(x) => x,
            None => return None
        };
        let state = KeyButton::get(state_pre);
        let mode = match NotifyMode::get(mode_pre) {
            Some(x) => x,
            None => return None
        };
        let (same_screen, focus) = match extra {
            0x01 => (true, false),
            0x02 => (false, true),
            0x03 => (true, true),
            _ => (false, false)
        };
        Some(ServerEvent::LeaveNotify { detail, time, root, event, child, root_x, root_y, event_x, event_y, state, mode, same_screen, focus })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_focus_in(&mut self, detail_pre: u8) -> Option<ServerEvent> {
        let (event, mode_pre, _) = self.read_focus_event();
        let detail = match FocusType::get(detail_pre) {
            Some(x) => x,
            None => return None
        };
        let mode = match FocusMode::get(mode_pre) {
            Some(x) => x,
            None => return None
        };
        Some(ServerEvent::FocusIn { detail, event, mode })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_focus_out(&mut self, detail_pre: u8) -> Option<ServerEvent> {
        let (event, mode_pre, _) = self.read_focus_event();
        let detail = match FocusType::get(detail_pre) {
            Some(x) => x,
            None => return None
        };
        let mode = match FocusMode::get(mode_pre) {
            Some(x) => x,
            None => return None
        };
        Some(ServerEvent::FocusIn { detail, event, mode })
    }

    /** Reads an event from the server (assumes first byte read) */
    #[allow(unused_variables)]
    pub fn read_keymap_notify(&mut self, detail: u8) -> Option<ServerEvent> {
        panic!("Not implemented yet. Go write an Issue on GitHub please."); // Going to need some research. Doesn't have have a sequence number... is this just 31 bytes?
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_expose(&mut self) -> Option<ServerEvent> {
        let window = self.read_u32();
        let x = self.read_u16();
        let y = self.read_u16();
        let width = self.read_u16();
        let height = self.read_u16();
        let count = self.read_u16();
        self.read_pad(14);
        Some(ServerEvent::Expose { window, x, y, width, height, count })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_graphics_exposure(&mut self) -> Option<ServerEvent> {
        let drawable = self.read_u32();
        let x = self.read_u16();
        let y = self.read_u16();
        let width = self.read_u16();
        let height = self.read_u16();
        let minor_opcode = self.read_u16();
        let count = self.read_u16();
        let major_opcode = self.read_u8();
        self.read_pad(11);
        Some(ServerEvent::GraphicsExposure { drawable, x, y, width, height, minor_opcode, count, major_opcode })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_no_exposure(&mut self) -> Option<ServerEvent> {
        let drawable = self.read_u32();
        let minor_opcode = self.read_u16();
        let major_opcode = self.read_u8();
        self.read_pad(21);
        Some(ServerEvent::NoExposure { drawable, minor_opcode, major_opcode })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_visibility_notify(&mut self) -> Option<ServerEvent> {
        let window = self.read_u32();
        let state = match VisibilityState::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        self.read_pad(23);
        Some(ServerEvent::VisibilityNotify { window, state })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_create_notify(&mut self) -> Option<ServerEvent> {
        let parent = self.read_u32();
        let window = self.read_u32();
        let x = self.read_i16();
        let y = self.read_i16();
        let width = self.read_u16();
        let height = self.read_u16();
        let border_width = self.read_u16();
        let override_redirect = self.read_bool();
        self.read_pad(9);
        Some(ServerEvent::CreateNotify { parent, window, x, y, width, height, border_width, override_redirect })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_destroy_notify(&mut self) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        self.read_pad(20);
        Some(ServerEvent::DestroyNotify { event, window })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_unmap_notify(&mut self) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        let from_configure = self.read_bool();
        self.read_pad(19);
        Some(ServerEvent::UnmapNotify { event, window, from_configure })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_map_notify(&mut self) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        let override_redirect = self.read_bool();
        self.read_pad(19);
        Some(ServerEvent::MapNotify { event, window, override_redirect })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_map_request(&mut self) -> Option<ServerEvent> {
        let parent = self.read_u32();
        let window = self.read_u32();
        self.read_pad(20);
        Some(ServerEvent::MapRequest { parent, window })
    }

    pub fn read_reparent_notify(&mut self) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        let parent = self.read_u32();
        let x = self.read_i16();
        let y = self.read_i16();
        let override_redirect = self.read_bool();
        self.read_pad(11);
        Some(ServerEvent::ReparentNotify { event, window, parent, x, y, override_redirect })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_configure_notify(&mut self) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        let above_sibling = self.read_u32();
        let x = self.read_i16();
        let y = self.read_i16();
        let width = self.read_u16();
        let height = self.read_u16();
        let border_width = self.read_u16();
        let override_redirect = self.read_bool();
        self.read_pad(5);
        Some(ServerEvent::ConfigureNotify { event, window, above_sibling, x, y, width, height, border_width, override_redirect })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_configure_request(&mut self, stack_mode_pre: u8) -> Option<ServerEvent> {
        let stack_mode = match StackMode::get(stack_mode_pre) {
            Some(x) => x,
            None => return None
        };
        let parent = self.read_u32();
        let window = self.read_u32();
        let sibling = self.read_u32();
        let x = self.read_i16();
        let y = self.read_i16();
        let width = self.read_u16();
        let height = self.read_u16();
        let border_width = self.read_u16();
        let values = ConfigureRequestValues::get(self.read_u16());
        self.read_pad(4);
        Some(ServerEvent::ConfigureRequest { stack_mode, parent, window, sibling, x, y, width, height, border_width, values })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_gravity_notify(&mut self) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        let x = self.read_i16();
        let y = self.read_i16();
        self.read_pad(16);
        Some(ServerEvent::GravityNotify { event, window, x, y })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_resize_request(&mut self) -> Option<ServerEvent> {
        let window = self.read_u32();
        let width = self.read_u16();
        let height = self.read_u16();
        self.read_pad(20);
        Some(ServerEvent::ResizeRequest { window, width, height })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_circulate_notify(&mut self) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        self.read_pad(4); // Spec says "4 bytes, WINDOW, unused"... wut
        let place = match CirculatePlace::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        self.read_pad(15);
        Some(ServerEvent::CirculateNotify { event, window, place })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_circulate_request(&mut self) -> Option<ServerEvent> {
        let parent = self.read_u32();
        let window = self.read_u32();
        self.read_pad(4);
        let place = match CirculatePlace::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        self.read_pad(15);
        Some(ServerEvent::CirculateRequest { parent, window, place })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_property_notify(&mut self) -> Option<ServerEvent> {
        let window = self.read_u32();
        let atom = self.read_u32();
        let time = self.read_u32();
        let state = match PropertyState::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        self.read_pad(15);
        Some(ServerEvent::PropertyNotify { window, atom, time, state })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_selection_clear(&mut self) -> Option<ServerEvent> {
        let time = self.read_u32();
        let owner = self.read_u32();
        let selection = self.read_u32();
        self.read_pad(16);
        Some(ServerEvent::SelectionClear { time, owner, selection })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_selection_request(&mut self) -> Option<ServerEvent> {
        let time = self.read_u32();
        let owner = self.read_u32();
        let requestor = self.read_u32();
        let selection = self.read_u32();
        let target = self.read_u32();
        let property = self.read_u32();
        self.read_pad(4);
        Some(ServerEvent::SelectionRequest { time, owner, requestor, selection, target, property })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_selection_notify(&mut self) -> Option<ServerEvent> {
        let time = self.read_u32();
        let requestor = self.read_u32();
        let selection = self.read_u32();
        let target = self.read_u32();
        let property = self.read_u32();
        self.read_pad(8);
        Some(ServerEvent::SelectionNotify { time, requestor, selection, target, property })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_colormap_notify(&mut self) -> Option<ServerEvent> {
        let window = self.read_u32();
        let colormap = self.read_u32();
        let new = self.read_bool();
        let state = match ColormapState::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        self.read_pad(18);
        Some(ServerEvent::ColormapNotify { window, colormap, new, state})
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_client_message(&mut self, format: u8) -> Option<ServerEvent> {
        let window = self.read_u32();
        let mtype = self.read_u32();
        self.read_pad(20);
        Some(ServerEvent::ClientMessage { format, window, mtype })
    }

    /** Reads an event from the server (assumes first byte read) */
    pub fn read_mapping_notify(&mut self) -> Option<ServerEvent> {
        let request = match MappingType::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        let first_keycode = self.read_char();
        let count = self.read_u8();
        self.read_pad(25);
        Some(ServerEvent::MappingNotify { request, first_keycode, count })
    }
}

impl XBufferedReader for XReadHelper {
    /**
     * Reads X bytes and ignores them.
     */
    fn read_pad(&mut self, len: usize) {
        self.buf_in.consume(len);
    }

    /**
     * Reads a bool from the buffer.
     */
    fn read_bool(&mut self) -> bool {
        self.buf_in.read_exact(&mut self.buf_one_byte).unwrap();
        match self.buf_one_byte[0] {
            1 => true,
            0 => false,
            other => panic!("Invalid integer for boolean: {}", other)
        }
    }

    /**
     * Reads a u8 from the buffer.
     */
    fn read_u8(&mut self) -> u8 {
        self.buf_in.read_exact(&mut self.buf_one_byte).unwrap();
        self.buf_one_byte[0]
    }

    /**
     * Reads an i16 from the buffer.
     * Expects little endian.
     */
    fn read_i16(&mut self) -> i16 {
        self.read_u16() as i16
    }

    /**
     * Reads a u16 from the buffer.
     * Expects little endian.
     */
    fn read_u16(&mut self) -> u16 {
        self.buf_in.read_exact(&mut self.buf_two_byte).unwrap();
        (self.buf_two_byte[0] as u16) + ((self.buf_two_byte[1] as u16) << 8)
    }

    /**
     * Reads a u32 from the buffer.
     * Expects little endian.
     */
    fn read_u32(&mut self) -> u32 {
        self.buf_in.read_exact(&mut self.buf_four_byte).unwrap();
        (self.buf_four_byte[0] as u32) + ((self.buf_four_byte[1] as u32) << 8) + ((self.buf_four_byte[2] as u32) << 16) + ((self.buf_four_byte[3] as u32) << 24)
    }

    /**
     * Reads a one-byte characters from the buffer.
     */
    fn read_char(&mut self) -> char {
        self.read_u8() as char
    }

    /**
     * Reads a string from the buffer.
     */
    fn read_str(&mut self, len: usize) -> String {
        let mut buf = vec![0u8; len];
        self.buf_in.read_exact(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }
}