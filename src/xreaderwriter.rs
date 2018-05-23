use models::*;
use protocol;

use std::str;
use std::os::unix::net::UnixStream;
use std::io::prelude::*;
use std::io::{BufReader};

pub trait XBufferedWriter {
    fn write_sequence(&mut self, rtype: ServerReplyType) -> u16;
    fn write_request(&mut self);
    fn write_raw(&mut self, buf: &[u8]);
    fn write_pad(&mut self, len: usize);
    fn write_pad_op(&mut self, len: usize);
    fn write_dynamic_len(&mut self, base: u16, len: usize) -> usize;
    fn write_bool(&mut self, input: bool);
    fn write_u8(&mut self, input: u8);
    fn write_i8(&mut self, input: i8);
    fn write_char(&mut self, input: char);
    fn write_i16(&mut self, input: i16);
    fn write_u16(&mut self, input: u16);
    fn write_i32(&mut self, input: i32);
    fn write_u32(&mut self, input: u32);
    fn write_str(&mut self, input: &str);
    fn write_mask_u16(&mut self, input: &Vec<u16>);
    fn write_mask_u32(&mut self, input: &Vec<u32>);
    fn write_val_bool(&mut self, input: bool);
    fn write_val_u8(&mut self, input: u8);
    fn write_val_i16(&mut self, input: i16);
    fn write_val_u16(&mut self, input: u16);
    fn write_val_i32(&mut self, input: i32);
    fn write_val_u32(&mut self, input: u32);
    fn write_val(&mut self, input: u32);
    fn write_values<T: Value>(&mut self, values: &Vec<T>, mask_size: u8);
}

pub trait XBufferedReader {
    fn prep_read(&mut self, len: usize);
    fn prep_read_extend(&mut self, len: usize);
    fn read_pad(&mut self, len: usize);
    fn read_bool(&mut self) -> bool;
    fn read_u8(&mut self) -> u8;
    fn read_i16(&mut self) -> i16;
    fn read_u16(&mut self) -> u16;
    fn read_i32(&mut self) -> i32;
    fn read_u32(&mut self) -> u32;
    fn read_char(&mut self) -> char;
    fn read_str(&mut self, len: usize) -> String;
    fn read_raw(&mut self, len: usize) -> Vec<u8>;
    fn read_raw_buf(&mut self, buf: &mut [u8]);
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

// Errors and replies
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

    /** Reads a reply to GetWindowAttributes */
    pub fn read_get_window_attributes_reply(&mut self, backing_store: u8) -> Option<ServerReply> {
        let backing_store = match WindowBackingStore::get(backing_store) {
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

    /** Reads TODO */
    pub fn read_get_geometry_reply(&mut self, depth: u8) -> Option<ServerReply> {
        let root = self.read_u32();
        let x = self.read_i16();
        let y = self.read_i16();
        let width = self.read_u16();
        let height = self.read_u16();
        let border_width = self.read_u16();
        self.read_pad(10);
        Some(ServerReply::GetGeometry { depth, root, x, y, width, height, border_width })
    }

    /** Reads TODO */
    pub fn read_query_tree_reply(&mut self) -> Option<ServerReply> {
        let root = self.read_u32();
        let parent = self.read_u32();
        let count = self.read_u16();
        self.read_pad(14);
        let mut wids = Vec::with_capacity(count as usize);
        for _ in 0..count {
            wids.push(self.read_u32());
        }
        Some(ServerReply::QueryTree { root, parent, wids })
    }
    
    /**
     * Reads TODO
     */
    pub fn read_intern_atom_reply(&mut self) -> Option<ServerReply> {
        let atom = self.read_u32();
        self.read_pad(20);
        Some(ServerReply::InternAtom { atom })
    }

    /** Reads TODO */
    pub fn read_get_atom_name_reply(&mut self) -> Option<ServerReply> {
        let len = self.read_u16();
        self.read_pad(22);
        let name = self.read_str(len as usize);
        Some(ServerReply::GetAtomName { name })
    }

    /** Reads TODO */
    pub fn read_get_property_reply(&mut self, format: u8) -> Option<ServerReply> {
        let vtype = self.read_u32();
        let bytes_after = self.read_u32();
        let len = self.read_u32();
        self.read_pad(12);
        let mut len = match format {
            0 => 0,
            8 => len,
            16 => len * 2,
            32 => len * 4,
            _ => return None
        };
        let value = self.read_raw(len as usize);
        Some(ServerReply::GetProperty { vtype, value })
    }

    /** Reads TODO */
    pub fn read_list_properties_reply(&mut self) -> Option<ServerReply> {
        let len = self.read_u16();
        self.read_pad(22);
        let mut atoms = Vec::with_capacity(len as usize);
        for _ in 0..len {
            atoms.push(self.read_u32());
        }
        Some(ServerReply::ListProperties { atoms })
    }

    /** Reads TODO */
    pub fn read_get_selection_owner_reply(&mut self) -> Option<ServerReply> {
        let wid = self.read_u32();
        self.read_pad(20);
        Some(ServerReply::GetSelectionOwner { wid })
    }

    /** Reads TODO */
    pub fn read_grab_pointer_reply(&mut self, status: u8) -> Option<ServerReply> {
        let status = match GrabStatus::get(status) {
            Some(val) => val,
            None => return None
        };
        self.read_pad(24);
        Some(ServerReply::GrabPointer { status })
    }
    
    /** Reads TODO */
    pub fn read_grab_keyboard_reply(&mut self, status: u8) -> Option<ServerReply> {
        let status = match GrabStatus::get(status) {
            Some(val) => val,
            None => return None
        };
        self.read_pad(24);
        Some(ServerReply::GrabKeyboard { status })
    }

    /** Reads TODO */
    pub fn read_query_pointer_reply(&mut self, same_screen: u8) -> Option<ServerReply> {
        let same_screen = match same_screen {
            0 => false,
            1 => true,
            _ => return None
        };
        let root = self.read_u32();
        let child = self.read_u32();
        let root_x = self.read_i16();
        let root_y = self.read_i16();
        let win_x = self.read_i16();
        let win_y = self.read_i16();
        let key_buttons = KeyButton::get(self.read_u16());
        self.read_pad(6);
        Some(ServerReply::QueryPointer { root, child, root_x, root_y, win_x, win_y, key_buttons, same_screen })
    }

    /** Reads TODO */
    pub fn read_get_motion_events_reply(&mut self) -> Option<ServerReply> {
        let count = self.read_u32();
        self.read_pad(20);
        let mut events = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let time = self.read_u32();
            let x = self.read_i16();
            let y = self.read_i16();
            events.push(TimeCoordinate { time, x, y });
        }
        Some(ServerReply::GetMotionEvents { events })
    }

    /** Reads TODO */
    pub fn read_translate_coordinates_reply(&mut self, same_screen: u8) -> Option<ServerReply> {
        let same_screen = match same_screen {
            0 => false,
            1 => true,
            _ => return None
        };
        let child = self.read_u32();
        let dst_x = self.read_i16();
        let dst_y = self.read_i16();
        self.read_pad(16);
        Some(ServerReply::TranslateCoordinates { child, dst_x, dst_y, same_screen })
    }

    /** Reads TODO */
    pub fn read_get_input_focus_reply(&mut self, revert_to: u8) -> Option<ServerReply> {
        let revert_to = match InputFocusRevert::get(revert_to) {
            Some(val) => val,
            None => return None
        };
        let wid = self.read_u32();
        self.read_pad(20);
        Some(ServerReply::GetInputFocus { wid, revert_to })
    }

    /** Reads TODO */
    pub fn read_query_keymap_reply(&mut self) -> Option<ServerReply> {
        let mut keys = Vec::with_capacity(32);
        for _ in 0..32 {
            keys.push(self.read_u8() as char);
        }
        Some(ServerReply::QueryKeymap { keys })
    }

    /** Reads TODO */
    pub fn read_query_font_reply(&mut self) -> Option<ServerReply> {
        panic!("TODO: QueryFont reply");
    }

    /** Reads TODO */
    pub fn read_query_text_extents_reply(&mut self, draw_direction: u8) -> Option<ServerReply> {
        let draw_direction = match FontDrawDirection::get(draw_direction) {
            Some(val) => val,
            None => return None
        };
        let font_ascent = self.read_i16();
        let font_descent = self.read_i16();
        let overall_ascent = self.read_i16();
        let overall_descent = self.read_i16();
        let overall_width = self.read_i32();
        let overall_left = self.read_i32();
        let overall_right = self.read_i32();
        self.read_pad(4);
        Some(ServerReply::QueryTextExtents { draw_direction, font_ascent, font_descent, overall_ascent, overall_descent, overall_width, overall_left, overall_right })
    }

    /** Reads TODO */
    pub fn read_list_fonts_reply(&mut self) -> Option<ServerReply> {
        let count = self.read_u16();
        self.read_pad(22);
        let mut names = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let len = self.read_u8();
            names.push(self.read_str(len as usize));
        }
        Some(ServerReply::ListFonts { names })
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

    /** Reads TODO */
    pub fn read_get_font_path_reply(&mut self) -> Option<ServerReply> {
        let count = self.read_u16();
        self.read_pad(22);
        let mut path = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let len = self.read_u8();
            path.push(self.read_str(len as usize));
        }
        Some(ServerReply::GetFontPath { path })
    }

    /** Reads TODO */
    pub fn read_get_image_reply(&mut self, depth: u8) -> Option<ServerReply> {
        panic!("TODO: GetImage reply");
    }

    /** Reads TODO */
    pub fn read_list_installed_colormaps_reply(&mut self) -> Option<ServerReply> {
        let count = self.read_u16();
        self.read_pad(22);
        let mut cmids = Vec::with_capacity(count as usize);
        for _ in 0..count {
            cmids.push(self.read_u32());
        }
        Some(ServerReply::ListInstalledColormaps { cmids })
    }

    /** Reads TODO */
    pub fn read_alloc_color_reply(&mut self) -> Option<ServerReply> {
        let red = self.read_u16();
        let green = self.read_u16();
        let blue = self.read_u16();
        self.read_pad(2);
        let pixel = self.read_u32();
        self.read_pad(12);
        Some(ServerReply::AllocColor { red, green, blue, pixel })
    }

    /** Reads TODO */
    pub fn read_alloc_named_color_reply(&mut self) -> Option<ServerReply> {
        let pixel = self.read_u32();
        let exact_red = self.read_u16();
        let exact_green = self.read_u16();
        let exact_blue = self.read_u16();
        let visual_red = self.read_u16();
        let visual_green = self.read_u16();
        let visual_blue = self.read_u16();
        self.read_pad(8);
        Some(ServerReply::AllocNamedColor { pixel, exact_red, exact_green, exact_blue, visual_red, visual_green, visual_blue })
    }

    /** Reads TODO */
    pub fn read_alloc_color_cells_reply(&mut self) -> Option<ServerReply> {
        let pcount = self.read_u16();
        let mcount = self.read_u16();
        let mut pixels = Vec::with_capacity(pcount as usize);
        let mut masks = Vec::with_capacity(mcount as usize);
        self.read_pad(20);
        for _ in 0..pcount {
            pixels.push(self.read_u32());
        }
        for _ in 0..mcount {
            masks.push(self.read_u32());
        }
        Some(ServerReply::AllocColorCells { pixels, masks })
    }

    /** Reads TODO */
    pub fn read_alloc_color_planes_reply(&mut self) -> Option<ServerReply> {
        let count = self.read_u16();
        self.read_pad(2);
        let red_mask = self.read_u32();
        let green_mask = self.read_u32();
        let blue_mask = self.read_u32();
        self.read_pad(8);
        let mut pixels = Vec::with_capacity(count as usize);
        for _ in 0..count {
            pixels.push(self.read_u32());
        }
        Some(ServerReply::AllocColorPlanes { pixels, red_mask, green_mask, blue_mask })
    }

    /** Reads TODO */
    pub fn read_query_colors_reply(&mut self) -> Option<ServerReply> {
        let count = self.read_u16();
        self.read_pad(22);
        let mut colors = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let red = self.read_u16();
            let green = self.read_u16();
            let blue = self.read_u16();
            self.read_pad(2);
            colors.push(Color { red, green, blue });
        }
        Some(ServerReply::QueryColors { colors })
    }

    /** Reads TODO */
    pub fn read_lookup_color_reply(&mut self) -> Option<ServerReply> {
        let exact_red = self.read_u16();
        let exact_green = self.read_u16();
        let exact_blue = self.read_u16();
        let visual_red = self.read_u16();
        let visual_green = self.read_u16();
        let visual_blue = self.read_u16();
        self.read_pad(12);
        Some(ServerReply::LookupColor { exact_red, exact_green, exact_blue, visual_red, visual_green, visual_blue })
    }

    /** Reads TODO */
    pub fn read_query_best_size_reply(&mut self) -> Option<ServerReply> {
        let width = self.read_u16();
        let height = self.read_u16();
        self.read_pad(20);
        Some(ServerReply::QueryBestSize { width, height })
    }

    /** Reads TODO */
    pub fn read_query_extension_reply(&mut self) -> Option<ServerReply> {
        let present = self.read_bool();;
        let major_opcode = self.read_u8();
        let first_event = self.read_u8();
        let first_error = self.read_u8();
        self.read_pad(20);
        Some(ServerReply::QueryExtension { present, major_opcode, first_event, first_error })
    }

    /** Reads TODO */
    pub fn read_list_extensions_reply(&mut self, len: u8) -> Option<ServerReply> {
        self.read_pad(24);
        let mut names = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let len = self.read_u8();
            names.push(self.read_str(len as usize));
        }
        Some(ServerReply::ListExtensions { names })
    }

    /** Reads TODO */
    pub fn read_get_keyboard_mapping_reply(&mut self, count: u8) -> Option<ServerReply> {
        panic!("TODO: GetKeyboardMapping reply");
    }

    /** Reads TODO */
    pub fn read_get_keyboard_control_reply(&mut self, global_auto_repeat: u8) -> Option<ServerReply> {
        let global_auto_repeat = match KeyboardControlAutoRepeatMode::get(global_auto_repeat) {
            Some(val) => val,
            None => return None
        };
        let led_mask = self.read_u32();
        let key_click_percent = self.read_u8();
        let bell_percent = self.read_u8();
        let bell_pitch = self.read_u16();
        let bell_duration = self.read_u16();
        self.read_pad(2);
        let mut auto_repeats = Vec::with_capacity(32);
        for _ in 0..32 {
            auto_repeats.push(self.read_u8()); // TODO: Should this be an enum?
        }
        Some(ServerReply::GetKeyboardControl { global_auto_repeat, led_mask, key_click_percent, bell_percent, bell_pitch, bell_duration, auto_repeats })
    }

    /** Reads TODO */
    pub fn read_get_pointer_control_reply(&mut self) -> Option<ServerReply> {
        let acceleration_numerator = self.read_u16();
        let acceleration_denominator = self.read_u16();
        let threshold = self.read_u16();
        self.read_pad(18);
        Some(ServerReply::GetPointerControl { acceleration_numerator, acceleration_denominator, threshold })
    }

    /** Reads TODO */
    pub fn read_get_screen_saver_reply(&mut self) -> Option<ServerReply> {
        let timeout = self.read_u16();
        let interval = self.read_u16();
        let prefer_blanking = match self.read_u8() {
            0 => false,
            1 => true,
            _ => return None
        };
        let allow_exposures = match self.read_u8() {
            0 => false,
            1 => true,
            _ => return None
        };
        self.read_pad(18);
        Some(ServerReply::GetScreenSaver { timeout, interval, prefer_blanking, allow_exposures })
    }

    /** Reads TODO */
    pub fn read_list_hosts_reply(&mut self, mode: u8) -> Option<ServerReply> {
        let enabled = match mode {
            0 => false,
            1 => true,
            _ => return None
        };
        let count = self.read_u16();
        self.read_pad(22);
        let mut hosts = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let family = match HostFamily::get(self.read_u8()) {
                Some(val) => val,
                None => return None
            };
            self.read_pad(1);
            let len = self.read_u16();
            let address = self.read_raw(len as usize);
            hosts.push(Host { family, address });
            self.read_pad((len % 4) as usize);
        }
        Some(ServerReply::ListHosts { enabled, hosts })
    }

    /** Reads TODO */
    pub fn read_set_pointer_mapping_reply(&mut self, status: u8) -> Option<ServerReply> {
        let success = match status {
            0 => true,
            1 => false,
            _ => return None
        };
        self.read_pad(24);
        Some(ServerReply::SetPointerMapping { success })
    }

    /** Reads TODO */
    pub fn read_get_pointer_mapping_reply(&mut self, len: u8) -> Option<ServerReply> {
        self.read_pad(24);
        let map = self.read_raw(len as usize);
        Some(ServerReply::GetPointerMapping { map })
    }

    /** Reads TODO */
    pub fn read_set_modifier_mapping_reply(&mut self, status: u8) -> Option<ServerReply> {
        let status = match SetModifierMappingStatus::get(status) {
            Some(val) => val,
            None => return None
        };
        self.read_pad(24);
        Some(ServerReply::SetModifierMapping { status })
    }

    /** Reads TODO */
    pub fn read_get_modifier_mapping_reply(&mut self, len: u8) -> Option<ServerReply> {
        self.read_pad(24);
        let mut key_codes = Vec::with_capacity((len * 8) as usize);
        for _ in 0..len*8 {
            key_codes.push(self.read_char());
        }
        Some(ServerReply::GetModifierMapping { key_codes })
    }
}

// Events
impl XReadHelper {
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

    /** Reads a generic focus event (assumes first byte read) and returns the results. Also reads the padding. */
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
        let buf = self.read_raw(20);
        let mut data = [0u8; 20];
        data.clone_from_slice(&buf);
        Some(ServerEvent::ClientMessage { format, window, mtype, data })
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
    /** Reads the given bytes into the internal buffer */
    fn prep_read(&mut self, len: usize) {
        self.buf.resize(len, 0);
        self.xin.read_exact(&mut self.buf).unwrap();
        self.pos = 0;
    }

    /** Like prep_read, except it extends the current buffer by X bytes instead of replacing it, and does not reset pos */
    fn prep_read_extend(&mut self, len: usize) {
        let original_len = self.buf.len();
        self.buf.resize(original_len + len, 0);
        self.xin.read_exact(&mut self.buf[original_len..]).unwrap();
    }

    /**
     * Reads X bytes and ignores them. len may be zero.
     */
    fn read_pad(&mut self, len: usize) {
        if self.pos + len > self.buf.len() {
            panic!("Attempt to read out of buffer.");
        }

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
     * Reads an i32 from the buffer.
     * Expects little endian.
     */
    fn read_i32(&mut self) -> i32 {
        self.read_u32() as i32
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

    /**
     * Reads raw bytes from the buffer.
     */
    fn read_raw(&mut self, len: usize) -> Vec<u8> {
        let mut x = Vec::with_capacity(len);

        for i in self.pos..self.pos + len {
            x.push(self.buf[i]);
        }

        self.pos += len;
        x
    }

    /**
     * Reads raw bytes from the buffer into another.
     */
    fn read_raw_buf(&mut self, buf: &mut [u8]) {
        for i in self.pos..self.pos + buf.len() {
            buf[i - self.pos] = self.buf[i];
        }

        self.pos += buf.len();
    }
}
