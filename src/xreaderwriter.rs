use models::*;
use protocol;

use std::str;
use std::os::unix::net::UnixStream;
use std::io::prelude::*;
use std::io::{BufReader};

pub trait XBufferedWriter {
    fn write_sequence(&mut self, rtype: ServerReplyType) -> u16;
    fn write_raw(&mut self, buf: &[u8]);
    fn write_pad(&mut self, len: usize);
    fn write_pad_op(&mut self, len: usize);
    fn write_bool(&mut self, input: bool);
    fn write_u8(&mut self, input: u8);
    fn write_i16(&mut self, input: i16);
    fn write_u16(&mut self, input: u16);
    fn write_i32(&mut self, input: i32);
    fn write_u32(&mut self, input: u32);
    fn write_str(&mut self, input: &str);
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

/**
 * Helps to read X 11 messages.
 * Usage:
 *     1. Specify the number of bytes you are planning to read: prep_read(len)
 *        This will read those bytes into a buffer. read_* operations read from this buffer.
 *        A future prep_read will replace this buffer, regardless of how many bytes have been read from it.
 *        This is useful because you can stop reading at any time (ie if you find a bad character) and it will continue reading the next bit properly.
 *     2. Use read_* and prep_read_extend (for when you want to add X more bytes to the current buffer)
 */
pub struct XReadHelper {
    xin: UnixStream,
    buf: Vec<u8>,
    pos: usize
}

impl XReadHelper {
    pub fn new(xin: UnixStream) -> XReadHelper {
        XReadHelper {
            xin,
            buf: Vec::with_capacity(500), // 500 bytes as default max message length (although this should expand as needed)
            pos: 0
        }
    }
}

impl XReadHelper {
    /** Reads the given bytes into the internal buffer */
    pub fn prep_read(&mut self, len: usize) {
        self.buf.resize(len, 0);
        self.xin.read_exact(&mut self.buf).unwrap();
        self.pos = 0;
    }

    /** Like prep_read, except it extends the current buffer by X bytes instead of replacing it, and does not reset pos */
    pub fn prep_read_extend(&mut self, len: usize) {
        let original_len = self.buf.len();
        self.buf.resize(original_len + len, 0);
        self.xin.read_exact(&mut self.buf[original_len..]).unwrap();
    }

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

    /** Reads a reply to GetWindowAttributes */
    pub fn read_get_window_attributes_reply(&mut self, backing_store_pre: u8) -> Option<ServerReply> {
        let backing_store = match WindowBackingStore::get(backing_store_pre) {
            Some(x) => x,
            None => return None
        };
        let visual = self.read_u32();
        let class = match WindowInputType::get(self.read_u16()) {
            Some(x) => x,
            None => return None
        };
        let bit_gravity = match BitGravity::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        let window_gravity = match WindowGravity::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        let backing_planes = self.read_u32();
        let backing_pixel = self.read_u32();
        let save_under = self.read_bool();
        let map_is_installed = self.read_bool();
        let map_state = match MapState::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        let override_redirect = self.read_bool();
        let colormap = self.read_u32();
        let all_event_masks = self.read_u32();
        let your_event_mask = self.read_u32();
        let do_not_propagate_mask = self.read_u16();
        self.read_pad(2);
        Some(ServerReply::GetWindowAttributes { backing_store, visual, class, bit_gravity, window_gravity, backing_planes, backing_pixel, save_under, map_is_installed, map_state, override_redirect, colormap, all_event_masks, your_event_mask, do_not_propagate_mask })
    }

    /** Reads character info */
    fn read_char_info(&mut self) -> CharInfo {
        return CharInfo {
            left_side_bearing: self.read_i16(),
            right_side_bearingL: self.read_i16(),
            character_width: self.read_i16(),
            ascent: self.read_i16(),
            descent: self.read_i16(),
            attributes: self.read_u16()
        }
    }

    /** Reads a single reply to ListFontsWithInfo (may have multiple) */
    pub fn read_list_fonts_with_info_reply(&mut self, name_len: u8) -> Option<ServerReply> {
        if name_len == 0 { // Last entry
            self.read_pad(52);
            Some(ServerReply::ListFontsWithInfoEnd)
        } else {
            let len = self.read_u16();
            let min_bounds = self.read_char_info();
            self.read_pad(4);
            let max_bounds = self.read_char_info();
            self.read_pad(4);
            let min_char = self.read_u16();
            let max_char = self.read_u16();
            let default_char = self.read_u16();
            let num_font_props = self.read_u16();
            let draw_direction = match FontDrawDirection::get(self.read_u8()) {
                Some(x) => x,
                None => return None
            };
            let min_byte = self.read_u8();
            let max_byte = self.read_u8();
            let all_chars_exist = self.read_bool();
            let font_ascent = self.read_i16();
            let font_descent = self.read_i16();
            let replies_hint = self.read_u32();
            let mut properties = vec![];

            for _ in 0..num_font_props {
                properties.push(FontProperty {
                    name: self.read_u32(),
                    value: self.read_u32()
                });
            }

            let name = self.read_str(name_len as usize);
            match name_len % 4 {
                0 => (),
                pad => self.read_pad(pad as usize)
            };

            Some(ServerReply::ListFontsWithInfoEntry { min_bounds, max_bounds, min_char, max_char, default_char, draw_direction, min_byte, max_byte, all_chars_exist, font_ascent, font_descent, replies_hint, properties, name })
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
        self.pos += len;
    }

    /**
     * Reads a bool from the buffer.
     */
    fn read_bool(&mut self) -> bool {
        let x = match self.buf[self.pos] {
            1 => true,
            0 => false,
            other => panic!("Invalid integer for boolean: {}", other)
        };
        self.pos += 1;
        x
    }

    /**
     * Reads a u8 from the buffer.
     */
    fn read_u8(&mut self) -> u8 {
        let x = self.buf[self.pos];
        self.pos += 1;
        x
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
        let x = (self.buf[self.pos] as u16) + ((self.buf[self.pos + 1] as u16) << 8);
        self.pos += 2;
        x
    }

    /**
     * Reads a u32 from the buffer.
     * Expects little endian.
     */
    fn read_u32(&mut self) -> u32 {
        let x = (self.buf[self.pos] as u32) + ((self.buf[self.pos + 1] as u32) << 8) + ((self.buf[self.pos + 2] as u32) << 16) + ((self.buf[self.pos + 3] as u32) << 24);
        self.pos += 4;
        x
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
        let x = String::from(str::from_utf8(&self.buf[self.pos..self.pos + len]).unwrap());
        self.pos += len;
        x
    }
}
