extern crate bufstream;

pub mod models;
mod protocol;
mod xwriter;

use std::os::unix::net::UnixStream;
use std::io::prelude::*;

use self::bufstream::BufStream;
use self::models::*;
use self::xwriter::XBufferedWriter;

pub struct XClient {
    pub connected: bool,
    pub connect_info: ConnectInfo,
    buf: BufStream<UnixStream>,
    buf_one_byte: Vec<u8>,
    buf_two_byte: Vec<u8>,
    buf_four_byte: Vec<u8>,
    next_resource_id: u32
}

impl XClient {
    pub fn new(host: String) -> XClient {
        let stream = UnixStream::connect(host).unwrap();
        XClient {
            connected: false,
            connect_info: ConnectInfo::empty(),
            buf: BufStream::new(stream),
            buf_one_byte: vec![0u8; 1],
            buf_two_byte: vec![0u8; 2],
            buf_four_byte: vec![0u8; 4],
            next_resource_id: 0
        }
    }

    /** Sends the connection parameters and returns if it connected or not. */
    pub fn connect(&mut self) {
        // Send connection string
        {
            self.write_u8(protocol::CONNECT_LSB);
            self.write_pad(1);
            self.write_u16(protocol::CONNECT_MAJOR);
            self.write_u16(protocol::CONNECT_MINOR);
            self.write_u16(0);
            self.write_u16(0);
            //self.write_pad(4); // Pad empty string
            //self.write_pad(4); // Pad empty string
            self.write_pad(1); // Pad empty string
            self.write_pad(1); // Pad empty string
            self.write_flush();
        }

        // Read response header
        {
            // Read the head
            self.connect_info.status_code = self.read_u8();
            self.read_pad(1);
            self.connect_info.protocol_major_version = self.read_u16();
            self.connect_info.protocol_minor_version = self.read_u16();
            self.connect_info.additional_data_len = self.read_u16();

            // Check if the connection was a success
            // TODO: Parse body of failures
            match self.connect_info.status_code {
                protocol::CONNECT_SUCCESS => (),
                protocol::CONNECT_FAILED => panic!("Got CONNECT_FAILED"),
                protocol::CONNECT_AUTHENTICATE => panic!("Got CONNECT_AUTHENTICATE"),
                code => panic!("Got unexpected value {}", code),
            };

            // Parse success info
            println!("Server Protocol: {}.{}", self.connect_info.protocol_major_version, self.connect_info.protocol_minor_version);
            self.connect_info.release_number = self.read_u32();
            self.connect_info.resource_id_base = self.read_u32();
            self.connect_info.resource_id_mask = self.read_u32();
            self.connect_info.motion_buffer_size = self.read_u32();
            let vendor_length = self.read_u16();
            self.connect_info.max_request_length = self.read_u16();
            self.connect_info.num_screens = self.read_u8();
            self.connect_info.num_formats = self.read_u8();
            self.connect_info.image_byte_order = match self.read_u8() {
                0 => ByteOrder::LSBFirst,
                1 => ByteOrder::MSBFirst,
                order => panic!("Unknown image byte order {}", order),
            };
            self.connect_info.bitmap_format_bit_order = match self.read_u8() {
                0 => BitOrder::LeastSignificant,
                1 => BitOrder::MostSignificant,
                order => panic!("Unknown bitmap format bit order {}", order)
            };
            self.connect_info.bitmap_format_scanline_unit = self.read_u8();
            self.connect_info.bitmap_format_scanline_pad = self.read_u8();
            self.connect_info.min_keycode = self.read_char();
            self.connect_info.max_keycode = self.read_char();
            self.read_pad(4);

            self.connect_info.vendor = self.read_str(vendor_length as usize);
            self.read_pad((vendor_length as usize) % 4);
            println!("Server Vendor: {}", self.connect_info.vendor);

            // Formats (8 bytes each)
            for _ in 0..self.connect_info.num_formats {
                let mut format = Format::empty();
                format.depth = self.read_u8();
                format.bits_per_pixel = self.read_u8();
                format.scanline_pad = self.read_u8();
                self.read_pad(5);

                self.connect_info.formats.push(format);
            }

            // Read screens (x bytes, where x is a multiple of 4)
            for _ in 0..self.connect_info.num_screens {
                let mut screen = Screen::empty();
                screen.root = self.read_u32();
                screen.default_colormap = self.read_u32();
                screen.white_pixel = self.read_u32();
                screen.black_pixel = self.read_u32();
                screen.current_input_masks = self.read_u32();
                screen.width_in_pixels = self.read_u16();
                screen.height_in_pixels = self.read_u16();
                screen.width_in_millimeters = self.read_u16();
                screen.height_in_millimeters = self.read_u16();
                screen.min_installed_maps = self.read_u16();
                screen.max_installed_maps = self.read_u16();
                screen.root_visual = self.read_u32();
                screen.backing_stores = match self.read_u8() {
                    0 => ScreenBackingStores::Never,
                    1 => ScreenBackingStores::WhenMapped,
                    2 => ScreenBackingStores::Always,
                    store => panic!("Unknown backing score {}", store)
                };
                screen.save_unders = self.read_bool();
                screen.root_depth = self.read_u8();
                screen.num_depths = self.read_u8();

                // Read depths (x bytes, where x is a multiple of 4)
                for _ in 0..screen.num_depths {
                    let mut depth = Depth::empty();
                    depth.depth = self.read_u8();
                    self.read_pad(1);
                    depth.num_visuals = self.read_u16();
                    self.read_pad(4); // Unused
                    
                    // Read visuals (24 x num visuals bytes)
                    for _ in 0..depth.num_visuals {
                        let mut visual = Visual::empty();
                        visual.id = self.read_u32();
                        visual.class = match self.read_u8() {
                            0 => VisualType::StaticGray,
                            1 => VisualType::GrayScale,
                            2 => VisualType::StaticColor,
                            3 => VisualType::PseudoColor,
                            4 => VisualType::TrueColor,
                            5 => VisualType::DirectColor,
                            class => panic!("Unknown visual class {}", class)
                        };
                        visual.bits_per_rgb_value = self.read_u8();
                        visual.colormap_entries = self.read_u16();
                        visual.red_mask = self.read_u32();
                        visual.green_mask = self.read_u32();
                        visual.blue_mask = self.read_u32();
                        self.read_pad(4); // Unused

                        depth.visuals.push(visual);
                    }

                    screen.depths.push(depth);
                }

                self.connect_info.screens.push(screen);
            }
        }
    }

    /** Generates a new resource ID */
    pub fn new_resource_id(&mut self) -> u32 {
        // TODO: Thread lock
        // TODO: Allow re-use of released resources

        let id = self.next_resource_id;

        if id > self.connect_info.resource_id_mask {
            panic!("Out of resource IDs."); // Hopefully won't happen once re-using resource IDs is done
        }

        self.next_resource_id += 1;
        self.connect_info.resource_id_base | id
    }

    /** Waits for the next available event, error, or reply */
    pub fn wait_for_message(&mut self) -> ServerResponse {
        loop {
            // Read header
            let opcode = self.read_u8();
            let detail = self.read_u8();
            let sequence_number = self.read_u16();

            return match opcode {
                protocol::REPLY_ERROR => {
                    match self.read_error(detail, sequence_number) {
                        Some(err) => ServerResponse::Error(err),
                        None => continue
                    }
                },
                protocol::REPLY_REPLY => ServerResponse::Reply({
                    panic!("Server replies not implemented yet.") // TODO: Parse different types of replies. How do dat? Idfk
                }),
                other => ServerResponse::Event(
                    match match other { // MY EYES
                        protocol::REPLY_KEY_PRESS => self.read_key_press(detail, sequence_number),
                        protocol::REPLY_KEY_RELEASE => self.read_key_release(detail, sequence_number),
                        protocol::REPLY_BUTTON_PRESS => self.read_button_press(detail, sequence_number),
                        protocol::REPLY_BUTTON_RELEASE => self.read_button_release(detail, sequence_number),
                        protocol::REPLY_MOTION_NOTIFY => self.read_motion_notify(detail, sequence_number),
                        protocol::REPLY_ENTER_NOTIFY => self.read_enter_notify(detail, sequence_number),
                        protocol::REPLY_LEAVE_NOTIFY => self.read_leave_notify(detail, sequence_number),
                        protocol::REPLY_FOCUS_IN => self.read_focus_in(detail, sequence_number),
                        protocol::REPLY_FOCUS_OUT => self.read_focus_out(detail, sequence_number),
                        protocol::REPLY_KEYMAP_NOTIFY => self.read_keymap_notify(detail, sequence_number),
                        protocol::REPLY_EXPOSE => self.read_expose(sequence_number),
                        protocol::REPLY_GRAPHICS_EXPOSURE => self.read_graphics_exposure(sequence_number),
                        protocol::REPLY_NO_EXPOSURE => self.read_no_exposure(sequence_number),
                        protocol::REPLY_VISIBILITY_NOTIFY => self.read_visibility_notify(sequence_number),
                        protocol::REPLY_CREATE_NOTIFY => self.read_create_notify(sequence_number),
                        protocol::REPLY_DESTROY_NOTIFY => self.read_destroy_notify(sequence_number),
                        protocol::REPLY_UNMAP_NOTIFY => self.read_unmap_notify(sequence_number),
                        protocol::REPLY_MAP_NOTIFY => self.read_map_notify(sequence_number),
                        protocol::REPLY_MAP_REQUEST => self.read_map_request(sequence_number),
                        protocol::REPLY_REPART_NOTIFY => self.read_reparent_notify(sequence_number),
                        protocol::REPLY_CONFIGURE_NOTIFY => self.read_configure_notify(sequence_number),
                        protocol::REPLY_CONFIGURE_REQUEST => self.read_configure_request(detail, sequence_number),
                        protocol::REPLY_GRAVITY_NOTIFY => self.read_gravity_notify(sequence_number),
                        protocol::REPLY_RESIZE_REQUEST => self.read_resize_request(sequence_number),
                        protocol::REPLY_CIRCULATE_NOTIFY => self.read_circulate_notify(sequence_number),
                        protocol::REPLY_CIRCULATE_REQUEST => self.read_circulate_request(sequence_number),
                        protocol::REPLY_PROPERTY_NOTIFY => self.read_property_notify(sequence_number),
                        protocol::REPLY_SELECTION_CLEAR => self.read_selection_clear(sequence_number),
                        protocol::REPLY_SELECTION_REQUEST => self.read_selection_request(sequence_number),
                        protocol::REPLY_SELECTION_NOTIFY => self.read_selection_notify(sequence_number),
                        protocol::REPLY_COLORMAP_NOTIFY => self.read_colormap_notify(sequence_number),
                        protocol::REPLY_CLIENT_MESSAGE => self.read_client_message(detail, sequence_number),
                        protocol::REPLY_MAPPING_NOTIFY => self.read_mapping_notify(sequence_number),
                        _ => continue
                    } {
                        Some(event) => event,
                        None => continue
                    }
                )
            }
        }
    }
}

impl XClient { // This is actually a pretty nice feature for organization
    /** Tells the X Server to create a window */
    pub fn create_window(&mut self, window: &Window) {
        // Should be 28 not including values and their mask
        self.write_u8(protocol::OP_CREATE_WINDOW);
        self.write_u8(window.depth);
        self.write_u16(8 + window.values.len() as u16); // data length
        self.write_u32(window.wid);
        self.write_u32(window.parent);
        self.write_i16(window.x);
        self.write_i16(window.y);
        self.write_u16(window.width);
        self.write_u16(window.height);
        self.write_u16(window.border_width);
        self.write_u16(match window.class {
            WindowInputType::CopyFromParent => 0,
            WindowInputType::InputOutput => 1,
            WindowInputType::InputOnly => 2
        });
        self.write_u32(window.visual_id);
        self.write_values(&window.values);

        self.write_flush();
    }

    /** Tells the X Server to change a window's attributes */
    pub fn change_window_attributes(&mut self, wid: u32, values: &Vec<WindowValue>) {
        // Should be 28 not including values and their mask
        self.write_u8(protocol::OP_CHANGE_WINDOW_ATTRIBUTES);
        self.write_pad(1);
        self.write_u16(3 + values.len() as u16); // data length
        self.write_u32(wid);
        self.write_values(&values);

        self.write_flush();
    }

    /** Tells the X Server to send us the window's attributes */
    pub fn get_window_attributes(&mut self, wid: u32) {
        self.write_u8(protocol::OP_GET_WINDOW_ATTRIBUTES);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_flush();
    }

    /** Tells the X Server to map a window (makes it visible I think) */
    pub fn map_window(&mut self, window: u32) {
        self.write_u8(protocol::OP_MAP_WINDOW);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(window);

        self.write_flush();
    }

    /** Tells the X Server to create a pixmap */
    pub fn create_pixmap(&mut self, pixmap: Pixmap) {
        self.write_u8(protocol::OP_CREATE_PIXMAP);
        self.write_u8(pixmap.depth);
        self.write_u16(4); // Request length
        self.write_u32(pixmap.pid);
        self.write_u32(pixmap.drawable);
        self.write_u16(pixmap.width);
        self.write_u16(pixmap.height);

        self.write_flush();
    }

    /** Tells the X Server to create a graphics context */
    pub fn create_gc(&mut self, gc: GraphicsContext) {
        self.write_u8(protocol::OP_CREATE_GC);
        self.write_pad(1);
        self.write_u16(4 + gc.values.len() as u16);
        self.write_u32(gc.cid);
        self.write_u32(gc.drawable);
        self.write_values(&gc.values);

        self.write_flush();
    }
}

impl XClient {
    /** Reads an error from the server (assumes first byte read) */
    fn read_error(&mut self, code: u8, sequence_number: u16) -> Option<ServerError> {
        let info = self.read_u32(); // Always u32 or unused
        let minor_opcode = self.read_u16();
        let major_opcode = self.read_u8();
        self.read_pad(21);

        match code {
            protocol::ERROR_REQUEST => Some(ServerError::Request { sequence_number, minor_opcode, major_opcode }),
            protocol::ERROR_VALUE => Some(ServerError::Value { sequence_number, minor_opcode, major_opcode, bad_value: info }),
            protocol::ERROR_WINDOW => Some(ServerError::Window { sequence_number, minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_PIXMAP => Some(ServerError::Pixmap { sequence_number, minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_ATOM => Some(ServerError::Atom { sequence_number, minor_opcode, major_opcode, bad_atom_id: info }),
            protocol::ERROR_CURSOR => Some(ServerError::Cursor { sequence_number, minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_FONT => Some(ServerError::Font { sequence_number, minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_MATCH => Some(ServerError::Match { sequence_number, minor_opcode, major_opcode }),
            protocol::ERROR_DRAWABLE => Some(ServerError::Drawable { sequence_number, minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_ACCESS => Some(ServerError::Access { sequence_number, minor_opcode, major_opcode }),
            protocol::ERROR_ALLOC => Some(ServerError::Alloc { sequence_number, minor_opcode, major_opcode }),
            protocol::ERROR_COLORMAP => Some(ServerError::Colormap { sequence_number, minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_G_CONTEXT => Some(ServerError::GContext { sequence_number, minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_ID_CHOICE => Some(ServerError::IDChoice { sequence_number, minor_opcode, major_opcode, bad_resource_id: info }),
            protocol::ERROR_NAME => Some(ServerError::Name { sequence_number, minor_opcode, major_opcode }),
            protocol::ERROR_LENGTH => Some(ServerError::Length { sequence_number, minor_opcode, major_opcode }),
            protocol::ERROR_IMPLEMENTATION => Some(ServerError::Implementation { sequence_number, minor_opcode, major_opcode }),
            _ => None
        }
    }

    /** Reads a generic pointer event (assumes first byte read) and returns the results. This also reads the extra padding byte at the end, if there is one
     * Returns detail, sequence_number, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen, extra
     */
    fn read_pointer_event(&mut self) -> (u32, u32, u32, u32, i16, i16, i16, i16, u16, bool, u8) {
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
    fn read_pointer_event_with_mode(&mut self) -> (u32, u32, u32, u32, i16, i16, i16, i16, u16, u8, u8) {
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
    fn read_focus_event(&mut self) -> (u32, u8, ()) {
        (
            self.read_u32(),
            self.read_u8(),
            self.read_pad(23)
        )
    }

    /** Reads a key press from the server (assumes first byte read) */
    fn read_key_press(&mut self, key_code: u8, sequence_number: u16) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, same_screen, _)
            = self.read_pointer_event();
        let state = KeyButton::get(state_pre);
        Some(ServerEvent::KeyPress { key_code, sequence_number, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen })
    }

    /** Reads a key release from the server (assumes first byte read) */
    fn read_key_release(&mut self, key_code: u8, sequence_number: u16) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, same_screen, _)
            = self.read_pointer_event();
        let state = KeyButton::get(state_pre);
        Some(ServerEvent::KeyRelease { key_code, sequence_number, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen })
    }

    /** Reads a button press from the server (assumes first byte read) */
    fn read_button_press(&mut self, button: u8, sequence_number: u16) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, same_screen, _)
            = self.read_pointer_event();
        let state = KeyButton::get(state_pre);
        Some(ServerEvent::ButtonPress { button, sequence_number, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen })
    }

    /** Reads a button release from the server (assumes first byte read) */
    fn read_button_release(&mut self, button: u8, sequence_number: u16) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, same_screen, _)
            = self.read_pointer_event();
        let state = KeyButton::get(state_pre);
        Some(ServerEvent::ButtonRelease { button, sequence_number, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen })
    }

    /** Reads a motion notify from the server (assumes first byte read) */
    fn read_motion_notify(&mut self, detail_pre: u8, sequence_number: u16) -> Option<ServerEvent> {
        let (time, root, event, child, root_x, root_y, event_x, event_y, state_pre, same_screen, _)
            = self.read_pointer_event();
        let detail = match MotionNotifyType::get(detail_pre) {
            Some(x) => x,
            None => return None
        };
        let state = KeyButton::get(state_pre);
        Some(ServerEvent::MotionNotify { detail, sequence_number, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen })
    }

    /** Reads a motion notify from the server (assumes first byte read) */
    fn read_enter_notify(&mut self, detail_pre: u8, sequence_number: u16) -> Option<ServerEvent> {
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
        Some(ServerEvent::EnterNotify { detail, sequence_number, time, root, event, child, root_x, root_y, event_x, event_y, state, mode, same_screen, focus })
    }

    /** Reads a leave notify from the server (assumes first byte read) */
    fn read_leave_notify(&mut self, detail_pre: u8, sequence_number: u16) -> Option<ServerEvent> {
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
        Some(ServerEvent::LeaveNotify { detail, sequence_number, time, root, event, child, root_x, root_y, event_x, event_y, state, mode, same_screen, focus })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_focus_in(&mut self, detail_pre: u8, sequence_number: u16) -> Option<ServerEvent> {
        let (event, mode_pre, _) = self.read_focus_event();
        let detail = match FocusType::get(detail_pre) {
            Some(x) => x,
            None => return None
        };
        let mode = match FocusMode::get(mode_pre) {
            Some(x) => x,
            None => return None
        };
        Some(ServerEvent::FocusIn { detail, sequence_number, event, mode })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_focus_out(&mut self, detail_pre: u8, sequence_number: u16) -> Option<ServerEvent> {
        let (event, mode_pre, _) = self.read_focus_event();
        let detail = match FocusType::get(detail_pre) {
            Some(x) => x,
            None => return None
        };
        let mode = match FocusMode::get(mode_pre) {
            Some(x) => x,
            None => return None
        };
        Some(ServerEvent::FocusIn { detail, sequence_number, event, mode })
    }

    /** Reads an event from the server (assumes first byte read) */
    #[allow(unused_variables)]
    fn read_keymap_notify(&mut self, detail: u8, sequence_number: u16) -> Option<ServerEvent> {
        panic!("Not implemented yet. Go write an Issue on GitHub please."); // Going to need some research. Doesn't have have a sequence number... is this just 31 bytes?
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_expose(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let window = self.read_u32();
        let x = self.read_u16();
        let y = self.read_u16();
        let width = self.read_u16();
        let height = self.read_u16();
        let count = self.read_u16();
        self.read_pad(14);
        Some(ServerEvent::Expose { sequence_number, window, x, y, width, height, count })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_graphics_exposure(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let drawable = self.read_u32();
        let x = self.read_u16();
        let y = self.read_u16();
        let width = self.read_u16();
        let height = self.read_u16();
        let minor_opcode = self.read_u16();
        let count = self.read_u16();
        let major_opcode = self.read_u8();
        self.read_pad(11);
        Some(ServerEvent::GraphicsExposure { sequence_number, drawable, x, y, width, height, minor_opcode, count, major_opcode })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_no_exposure(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let drawable = self.read_u32();
        let minor_opcode = self.read_u16();
        let major_opcode = self.read_u8();
        self.read_pad(21);
        Some(ServerEvent::NoExposure { sequence_number, drawable, minor_opcode, major_opcode })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_visibility_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let window = self.read_u32();
        let state = match VisibilityState::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        self.read_pad(23);
        Some(ServerEvent::VisibilityNotify { sequence_number, window, state })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_create_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let parent = self.read_u32();
        let window = self.read_u32();
        let x = self.read_i16();
        let y = self.read_i16();
        let width = self.read_u16();
        let height = self.read_u16();
        let border_width = self.read_u16();
        let override_redirect = self.read_bool();
        self.read_pad(9);
        Some(ServerEvent::CreateNotify { sequence_number, parent, window, x, y, width, height, border_width, override_redirect })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_destroy_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        self.read_pad(20);
        Some(ServerEvent::DestroyNotify { sequence_number, event, window })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_unmap_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        let from_configure = self.read_bool();
        self.read_pad(19);
        Some(ServerEvent::UnmapNotify { sequence_number, event, window, from_configure })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_map_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        let override_redirect = self.read_bool();
        self.read_pad(19);
        Some(ServerEvent::MapNotify { sequence_number, event, window, override_redirect })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_map_request(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let parent = self.read_u32();
        let window = self.read_u32();
        self.read_pad(20);
        Some(ServerEvent::MapRequest { sequence_number, parent, window })
    }

    fn read_reparent_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        let parent = self.read_u32();
        let x = self.read_i16();
        let y = self.read_i16();
        let override_redirect = self.read_bool();
        self.read_pad(11);
        Some(ServerEvent::ReparentNotify { sequence_number, event, window, parent, x, y, override_redirect })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_configure_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
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
        Some(ServerEvent::ConfigureNotify { sequence_number, event, window, above_sibling, x, y, width, height, border_width, override_redirect })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_configure_request(&mut self, stack_mode_pre: u8, sequence_number: u16) -> Option<ServerEvent> {
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
        Some(ServerEvent::ConfigureRequest { stack_mode, sequence_number, parent, window, sibling, x, y, width, height, border_width, values })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_gravity_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        let x = self.read_i16();
        let y = self.read_i16();
        self.read_pad(16);
        Some(ServerEvent::GravityNotify { sequence_number, event, window, x, y })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_resize_request(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let window = self.read_u32();
        let width = self.read_u16();
        let height = self.read_u16();
        self.read_pad(20);
        Some(ServerEvent::ResizeRequest { sequence_number, window, width, height })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_circulate_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let event = self.read_u32();
        let window = self.read_u32();
        self.read_pad(4); // Spec says "4 bytes, WINDOW, unused"... wut
        let place = match CirculatePlace::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        self.read_pad(15);
        Some(ServerEvent::CirculateNotify { sequence_number, event, window, place })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_circulate_request(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let parent = self.read_u32();
        let window = self.read_u32();
        self.read_pad(4);
        let place = match CirculatePlace::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        self.read_pad(15);
        Some(ServerEvent::CirculateRequest { sequence_number, parent, window, place })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_property_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let window = self.read_u32();
        let atom = self.read_u32();
        let time = self.read_u32();
        let state = match PropertyState::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        self.read_pad(15);
        Some(ServerEvent::PropertyNotify { sequence_number, window, atom, time, state })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_selection_clear(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let time = self.read_u32();
        let owner = self.read_u32();
        let selection = self.read_u32();
        self.read_pad(16);
        Some(ServerEvent::SelectionClear { sequence_number, time, owner, selection })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_selection_request(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let time = self.read_u32();
        let owner = self.read_u32();
        let requestor = self.read_u32();
        let selection = self.read_u32();
        let target = self.read_u32();
        let property = self.read_u32();
        self.read_pad(4);
        Some(ServerEvent::SelectionRequest { sequence_number, time, owner, requestor, selection, target, property })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_selection_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let time = self.read_u32();
        let requestor = self.read_u32();
        let selection = self.read_u32();
        let target = self.read_u32();
        let property = self.read_u32();
        self.read_pad(8);
        Some(ServerEvent::SelectionNotify { sequence_number, time, requestor, selection, target, property })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_colormap_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let window = self.read_u32();
        let colormap = self.read_u32();
        let new = self.read_bool();
        let state = match ColormapState::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        self.read_pad(18);
        Some(ServerEvent::ColormapNotify { sequence_number, window, colormap, new, state})
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_client_message(&mut self, format: u8, sequence_number: u16) -> Option<ServerEvent> {
        let window = self.read_u32();
        let mtype = self.read_u32();
        self.read_pad(20);
        Some(ServerEvent::ClientMessage { format, sequence_number, window, mtype })
    }

    /** Reads an event from the server (assumes first byte read) */
    fn read_mapping_notify(&mut self, sequence_number: u16) -> Option<ServerEvent> {
        let request = match MappingType::get(self.read_u8()) {
            Some(x) => x,
            None => return None
        };
        let first_keycode = self.read_char();
        let count = self.read_u8();
        self.read_pad(25);
        Some(ServerEvent::MappingNotify { sequence_number, request, first_keycode, count })
    }
}

impl XBufferedWriter for XClient {
    /** Flushes the buffer. */
    fn write_flush(&mut self) {
        self.buf.flush().unwrap();
    }

    /**
     * Writes X bytes (not guaranteed to be zero).
     */
    fn write_pad(&mut self, len: usize) {
        match len {
            0 => panic!("Cannot write 0 bytes"),
            1 => self.buf.write_all(&self.buf_one_byte),
            2 => self.buf.write_all(&self.buf_two_byte),
            4 => self.buf.write_all(&self.buf_four_byte),
            _ => self.buf.write_all(&vec![0u8; len])
        }.unwrap();
    }

    /**
     * Writes a bool to the buffer.
     */
    fn write_bool(&mut self, input: bool) {
        self.buf_one_byte[0] = match input {
            true => 1,
            false => 0
        };
        self.buf.write_all(&self.buf_one_byte).unwrap();
    }

    /**
     * Writes a u8 to the buffer.
     */
    fn write_u8(&mut self, input: u8) {
        self.buf_one_byte[0] = input;
        self.buf.write_all(&self.buf_one_byte).unwrap();
    }

    /**
     * Writes a i16 to the buffer.
     * Expects little endian.
     */
    fn write_i16(&mut self, input: i16) {
        self.write_u16(input as u16);
    }

    /**
     * Writes a u16 to the buffer.
     * Expects little endian.
     */
    fn write_u16(&mut self, input: u16) {
        self.buf_two_byte[0] = input as u8;
        self.buf_two_byte[1] = (input >> 8) as u8;
        self.buf.write_all(&self.buf_two_byte).unwrap();
    }

    /**
     * Writes a i32 to the buffer.
     * Expects little endian.
     */
    fn write_i32(&mut self, input: i32) {
        self.write_u32(input as u32);
    }

    /**
     * Writes a u32 to the buffer.
     * Expects little endian.
     */
    fn write_u32(&mut self, input: u32) {
        self.buf_four_byte[0] = input as u8;
        self.buf_four_byte[1] = (input >> 8) as u8;
        self.buf_four_byte[2] = (input >> 16) as u8;
        self.buf_four_byte[3] = (input >> 24) as u8;
        self.buf.write_all(&self.buf_four_byte).unwrap();
    }

    /**
     * Writes 4 bytes to the buffer, with the least significant being the bool.
     */
    fn write_val_bool(&mut self, input: bool) {
        match input {
            true => self.write_val(1u8 as u32),
            false => self.write_val(0u8 as u32)
        };
    }

    /**
     * Writes a 4-byte value to the buffer.
     * Expects little endian.
     */
    fn write_val_u8(&mut self, input: u8) {
        self.write_val(input as u32);
    }

    /**
     * Writes a 4-byte value to the buffer.
     * Expects little endian.
     */
    fn write_val_i16(&mut self, input: i16) {
        self.write_val_u16(input as u16);
    }

    /**
     * Writes a 4-byte value to the buffer.
     * Expects little endian.
     */
    fn write_val_u16(&mut self, input: u16) {
        self.write_val(input as u32);
    }

    /**
     * Writes a i32 to the buffer.
     * Expects little endian.
     */
    fn write_val_i32(&mut self, input: i32) {
        self.write_val(input as u32);
    }

    /**
     * Writes a u32 to the buffer.
     * Expects little endian.
     */
    fn write_val_u32(&mut self, input: u32) {
        self.write_val(input);
    }

    /**
     * Writes a u32 to the buffer.
     * Expects little endian.
     */
    fn write_val(&mut self, input: u32) {
        self.write_u32(input);
    }

    /**
     * Writes a bitmap and values to the buffer.
     */
    fn write_values<T: Value>(&mut self, values: &Vec<T>) {
        let mut value_mask: u32 = 0x0;
        let mut order: Vec<usize> = Vec::with_capacity(values.len());

        for (i, value) in values.iter().enumerate() {
            let ordered = value.get_mask();
            value_mask |= ordered;
            let mut pos = order.len();

            for (j, val) in order.iter().enumerate()  {
                if ordered < values[*val].get_mask() {
                    pos = j;
                    break;
                }
            }

            if pos < order.len() {
                order.insert(pos, i);
            } else {
                order.push(pos);
            }
        }

        self.write_u32(value_mask);

        for i in order.iter() {
            values[*i].write(self);
        }
    }

    /**
     * Reads X bytes and ignores them.
     */
    fn read_pad(&mut self, len: usize) {
        self.buf.consume(len);
    }

    /**
     * Reads a bool from the buffer.
     */
    fn read_bool(&mut self) -> bool {
        self.buf.read_exact(&mut self.buf_one_byte).unwrap();
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
        self.buf.read_exact(&mut self.buf_one_byte).unwrap();
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
        self.buf.read_exact(&mut self.buf_two_byte).unwrap();
        (self.buf_two_byte[0] as u16) + ((self.buf_two_byte[1] as u16) << 8)
    }

    /**
     * Reads a u32 from the buffer.
     * Expects little endian.
     */
    fn read_u32(&mut self) -> u32 {
        self.buf.read_exact(&mut self.buf_four_byte).unwrap();
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
        self.buf.read_exact(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }
}
