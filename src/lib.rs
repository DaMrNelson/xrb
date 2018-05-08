pub mod models;
mod protocol;
mod xreaderwriter;

use std::os::unix::net::UnixStream;
use std::io::prelude::*;
use std::io::{BufWriter, BufReader};

use std::thread;
use std::sync::mpsc;

use self::models::*;
use self::xreaderwriter::{XBufferedWriter, XBufferedReader, XReadHelper};

pub struct XClient {
    pub connected: bool,
    pub connect_info: ConnectInfo,
    buf_out: BufWriter<UnixStream>,
    resp_receiver: mpsc::Receiver<ServerResponse>, // Receive errors, replies, and events from the X Server
    sq_sender: mpsc::Sender<(u16, ServerReplyType)>, // Send sequence IDs to the reader thread so it can properly parse replies
    next_resource_id: u32,
    current_sequence: u16,
    buf_one_byte: Vec<u8>,
    buf_two_byte: Vec<u8>,
    buf_four_byte: Vec<u8>
}

impl XClient {
    /**
     * Connects to the given X server.
     * Blocks until the connection is complete.
     * Spawns a new 1:1 thread to constantly read input from the X Server, which prevents deadlocks.
     */
    pub fn connect(host: String) -> XClient {
        let stream = UnixStream::connect(host).unwrap();
        let (resp_sender, resp_receiver) = mpsc::channel();
        let (sq_sender, sq_receiver) = mpsc::channel();
        let mut client = XClient {
            connected: false,
            connect_info: ConnectInfo::empty(),
            buf_out: BufWriter::new(stream),
            resp_receiver: resp_receiver,
            sq_sender: sq_sender,
            next_resource_id: 0,
            current_sequence: 0,
            buf_one_byte: vec![0u8; 1],
            buf_two_byte: vec![0u8; 2],
            buf_four_byte: vec![0u8; 4]
        };
        client.setup(resp_sender, sq_receiver);
        client
    }

    /** Sends the connection parameters and returns if it connected or not. */
    fn setup(&mut self, resp_sender: mpsc::Sender<ServerResponse>, sq_receiver: mpsc::Receiver<(u16, ServerReplyType)>) {
        let mut reader = XReadHelper::new(
            BufReader::new(
                self.buf_out.get_ref().try_clone().unwrap()
            )
        );

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

            self.write_sequence(ServerReplyType::None);
        }

        // Read response header
        {
            // Read the head
            self.connect_info.status_code = reader.read_u8();
            reader.read_pad(1);
            self.connect_info.protocol_major_version = reader.read_u16();
            self.connect_info.protocol_minor_version = reader.read_u16();
            self.connect_info.additional_data_len = reader.read_u16();

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
            self.connect_info.release_number = reader.read_u32();
            self.connect_info.resource_id_base = reader.read_u32();
            self.connect_info.resource_id_mask = reader.read_u32();
            self.connect_info.motion_buffer_size = reader.read_u32();
            let vendor_length = reader.read_u16();
            self.connect_info.max_request_length = reader.read_u16();
            self.connect_info.num_screens = reader.read_u8();
            self.connect_info.num_formats = reader.read_u8();
            self.connect_info.image_byte_order = match reader.read_u8() {
                0 => ByteOrder::LSBFirst,
                1 => ByteOrder::MSBFirst,
                order => panic!("Unknown image byte order {}", order),
            };
            self.connect_info.bitmap_format_bit_order = match reader.read_u8() {
                0 => BitOrder::LeastSignificant,
                1 => BitOrder::MostSignificant,
                order => panic!("Unknown bitmap format bit order {}", order)
            };
            self.connect_info.bitmap_format_scanline_unit = reader.read_u8();
            self.connect_info.bitmap_format_scanline_pad = reader.read_u8();
            self.connect_info.min_keycode = reader.read_char();
            self.connect_info.max_keycode = reader.read_char();
            reader.read_pad(4);

            self.connect_info.vendor = reader.read_str(vendor_length as usize);
            reader.read_pad((vendor_length as usize) % 4);
            println!("Server Vendor: {}", self.connect_info.vendor);

            // Formats (8 bytes each)
            for _ in 0..self.connect_info.num_formats {
                let mut format = Format::empty();
                format.depth = reader.read_u8();
                format.bits_per_pixel = reader.read_u8();
                format.scanline_pad = reader.read_u8();
                reader.read_pad(5);

                self.connect_info.formats.push(format);
            }

            // Read screens (x bytes, where x is a multiple of 4)
            for _ in 0..self.connect_info.num_screens {
                let mut screen = Screen::empty();
                screen.root = reader.read_u32();
                screen.default_colormap = reader.read_u32();
                screen.white_pixel = reader.read_u32();
                screen.black_pixel = reader.read_u32();
                screen.current_input_masks = reader.read_u32();
                screen.width_in_pixels = reader.read_u16();
                screen.height_in_pixels = reader.read_u16();
                screen.width_in_millimeters = reader.read_u16();
                screen.height_in_millimeters = reader.read_u16();
                screen.min_installed_maps = reader.read_u16();
                screen.max_installed_maps = reader.read_u16();
                screen.root_visual = reader.read_u32();
                screen.backing_stores = match reader.read_u8() {
                    0 => ScreenBackingStores::Never,
                    1 => ScreenBackingStores::WhenMapped,
                    2 => ScreenBackingStores::Always,
                    store => panic!("Unknown backing score {}", store)
                };
                screen.save_unders = reader.read_bool();
                screen.root_depth = reader.read_u8();
                screen.num_depths = reader.read_u8();

                // Read depths (x bytes, where x is a multiple of 4)
                for _ in 0..screen.num_depths {
                    let mut depth = Depth::empty();
                    depth.depth = reader.read_u8();
                    reader.read_pad(1);
                    depth.num_visuals = reader.read_u16();
                    reader.read_pad(4); // Unused
                    
                    // Read visuals (24 x num visuals bytes)
                    for _ in 0..depth.num_visuals {
                        let mut visual = Visual::empty();
                        visual.id = reader.read_u32();
                        visual.class = match reader.read_u8() {
                            0 => VisualType::StaticGray,
                            1 => VisualType::GrayScale,
                            2 => VisualType::StaticColor,
                            3 => VisualType::PseudoColor,
                            4 => VisualType::TrueColor,
                            5 => VisualType::DirectColor,
                            class => panic!("Unknown visual class {}", class)
                        };
                        visual.bits_per_rgb_value = reader.read_u8();
                        visual.colormap_entries = reader.read_u16();
                        visual.red_mask = reader.read_u32();
                        visual.green_mask = reader.read_u32();
                        visual.blue_mask = reader.read_u32();
                        reader.read_pad(4); // Unused

                        depth.visuals.push(visual);
                    }

                    screen.depths.push(depth);
                }

                self.connect_info.screens.push(screen);
            }
        }

        // Start event receiving thread
        {
            thread::spawn(move || { // Moves `resp_sender`, `sq_receiver`, and `reader`
                loop {
                    // Read header
                    let opcode = reader.read_u8();
                    let detail = reader.read_u8();
                    let sequence_number = reader.read_u16();

                    let response = match opcode {
                        protocol::REPLY_ERROR => {
                            match reader.read_error(detail) {
                                Some(err) => ServerResponse::Error(err, sequence_number),
                                None => continue
                            }
                        },
                        protocol::REPLY_REPLY => ServerResponse::Reply({
                            let reply_length = reader.read_u32();
                            let (seq, method) = sq_receiver.recv().unwrap();

                            if seq == sequence_number {
                                match match method {
                                    ServerReplyType::GetWindowAttributes => reader.read_get_window_attributes_reply(detail),
                                    ServerReplyType::ListFontsWithInfo => reader.read_list_fonts_with_info_reply(detail),
                                    // TODO: More
                                    _ => panic!("Reply not implemented yet.")
                                } {
                                    Some(x) => x,
                                    None => continue
                                }
                            } else {
                                eprintln!("Unexpected reply sequence. Expected {}, got {}. Will not attempt to parse.", sequence_number, seq);
                                eprintln!("If this occurs again after restarting the server please submit an issue to: https://github.com/DaMrNelson/xrb");
                                reader.read_pad((32 - 4 + reply_length * 4) as usize);
                                continue
                            }
                        }, sequence_number),
                        other => ServerResponse::Event(
                            match match other { // MY EYES
                                protocol::REPLY_KEY_PRESS => reader.read_key_press(detail),
                                protocol::REPLY_KEY_RELEASE => reader.read_key_release(detail),
                                protocol::REPLY_BUTTON_PRESS => reader.read_button_press(detail),
                                protocol::REPLY_BUTTON_RELEASE => reader.read_button_release(detail),
                                protocol::REPLY_MOTION_NOTIFY => reader.read_motion_notify(detail),
                                protocol::REPLY_ENTER_NOTIFY => reader.read_enter_notify(detail),
                                protocol::REPLY_LEAVE_NOTIFY => reader.read_leave_notify(detail),
                                protocol::REPLY_FOCUS_IN => reader.read_focus_in(detail),
                                protocol::REPLY_FOCUS_OUT => reader.read_focus_out(detail),
                                protocol::REPLY_KEYMAP_NOTIFY => reader.read_keymap_notify(detail),
                                protocol::REPLY_EXPOSE => reader.read_expose(),
                                protocol::REPLY_GRAPHICS_EXPOSURE => reader.read_graphics_exposure(),
                                protocol::REPLY_NO_EXPOSURE => reader.read_no_exposure(),
                                protocol::REPLY_VISIBILITY_NOTIFY => reader.read_visibility_notify(),
                                protocol::REPLY_CREATE_NOTIFY => reader.read_create_notify(),
                                protocol::REPLY_DESTROY_NOTIFY => reader.read_destroy_notify(),
                                protocol::REPLY_UNMAP_NOTIFY => reader.read_unmap_notify(),
                                protocol::REPLY_MAP_NOTIFY => reader.read_map_notify(),
                                protocol::REPLY_MAP_REQUEST => reader.read_map_request(),
                                protocol::REPLY_REPART_NOTIFY => reader.read_reparent_notify(),
                                protocol::REPLY_CONFIGURE_NOTIFY => reader.read_configure_notify(),
                                protocol::REPLY_CONFIGURE_REQUEST => reader.read_configure_request(detail),
                                protocol::REPLY_GRAVITY_NOTIFY => reader.read_gravity_notify(),
                                protocol::REPLY_RESIZE_REQUEST => reader.read_resize_request(),
                                protocol::REPLY_CIRCULATE_NOTIFY => reader.read_circulate_notify(),
                                protocol::REPLY_CIRCULATE_REQUEST => reader.read_circulate_request(),
                                protocol::REPLY_PROPERTY_NOTIFY => reader.read_property_notify(),
                                protocol::REPLY_SELECTION_CLEAR => reader.read_selection_clear(),
                                protocol::REPLY_SELECTION_REQUEST => reader.read_selection_request(),
                                protocol::REPLY_SELECTION_NOTIFY => reader.read_selection_notify(),
                                protocol::REPLY_COLORMAP_NOTIFY => reader.read_colormap_notify(),
                                protocol::REPLY_CLIENT_MESSAGE => reader.read_client_message(detail),
                                protocol::REPLY_MAPPING_NOTIFY => reader.read_mapping_notify(),
                                _ => continue
                            } {
                                Some(event) => event,
                                None => continue
                            }
                        , sequence_number)
                    };

                    match resp_sender.send(response) {
                        Ok(_) => (),
                        Err(e) => eprintln!("Failed to forward error, reply, or event to main thread: {:?}", e)
                    }
                }
            });
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

    /** Waits for the next available event, error, or reply. Blocks.
     * Use get_message if you do not want to block.
     */
    pub fn wait_for_message(&mut self) -> ServerResponse {
        loop {
            match self.resp_receiver.recv() {
                Ok(x) => return x,
                Err(e) => eprintln!("Failed to get message from the receiver. Will try again. Error: {:?}", e)
            };
        }
    }

    /**
     * Returns the next available event, error, or reply, or None if there is nothing yet. Does not block.
     * Use wait_for_message to block until a new message is received.
     */
    pub fn get_message(&mut self) -> Option<ServerResponse> {
        match self.resp_receiver.try_recv() {
            Ok(x) => Some(x),
            Err(_) => None
        }
    }
}

impl XClient { // This is actually a pretty nice feature for organization
    /** Tells the X Server to create a window */
    pub fn create_window(&mut self, window: &Window) -> u16 {
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

        self.write_sequence(ServerReplyType::None)
    }

    /** Tells the X Server to change a window's attributes */
    pub fn change_window_attributes(&mut self, wid: u32, values: &Vec<WindowValue>) -> u16 {
        // Should be 28 not including values and their mask
        self.write_u8(protocol::OP_CHANGE_WINDOW_ATTRIBUTES);
        self.write_pad(1);
        self.write_u16(3 + values.len() as u16); // data length
        self.write_u32(wid);
        self.write_values(&values);

        self.write_sequence(ServerReplyType::None)
    }

    /** Tells the X Server to send us the window's attributes */
    pub fn get_window_attributes(&mut self, wid: u32) -> u16 {
        self.write_u8(protocol::OP_GET_WINDOW_ATTRIBUTES);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(wid);

        self.write_sequence(ServerReplyType::GetWindowAttributes)
    }

    /** Tells the X Server to map a window (makes it visible I think) */
    pub fn map_window(&mut self, window: u32) -> u16 {
        self.write_u8(protocol::OP_MAP_WINDOW);
        self.write_pad(1);
        self.write_u16(2);
        self.write_u32(window);

        self.write_sequence(ServerReplyType::None)
    }

    /** Lists all fonts with the given info */
    pub fn list_fonts_with_info(&mut self, max_names: u16, pattern: &str) -> u16 {
        self.write_u8(protocol::OP_LIST_FONTS_WITH_INFO);
        self.write_pad(1);
        self.write_u16(2 + (pattern.len() + pattern.len() % 4) as u16 / 4);
        self.write_u16(max_names);
        self.write_u16(pattern.len() as u16);
        self.write_str(pattern);
        match pattern.len() % 4 {
            0 => (),
            pad => self.write_pad(pad)
        };

        self.write_sequence(ServerReplyType::ListFontsWithInfo)
    }

    /** Tells the X Server to create a pixmap */
    pub fn create_pixmap(&mut self, pixmap: Pixmap) -> u16 {
        self.write_u8(protocol::OP_CREATE_PIXMAP);
        self.write_u8(pixmap.depth);
        self.write_u16(4); // Request length
        self.write_u32(pixmap.pid);
        self.write_u32(pixmap.drawable);
        self.write_u16(pixmap.width);
        self.write_u16(pixmap.height);

        self.write_sequence(ServerReplyType::None)
    }

    /** Tells the X Server to create a graphics context */
    pub fn create_gc(&mut self, gc: GraphicsContext) -> u16 {
        self.write_u8(protocol::OP_CREATE_GC);
        self.write_pad(1);
        self.write_u16(4 + gc.values.len() as u16);
        self.write_u32(gc.cid);
        self.write_u32(gc.drawable);
        self.write_values(&gc.values);

        self.write_sequence(ServerReplyType::None)
    }
}

impl XBufferedWriter for XClient {
    /** Flushes the buffer. */
    fn write_sequence(&mut self, rtype: ServerReplyType) -> u16 {
        match rtype {
            ServerReplyType::None => (),
            _ => self.sq_sender.send((self.current_sequence, rtype)).unwrap()
        };
        self.buf_out.flush().unwrap();
        let the_sequence = self.current_sequence;
        self.current_sequence += 1;
        the_sequence
    }

    /**
     * Writes X bytes (not guaranteed to be zero).
     */
    fn write_pad(&mut self, len: usize) {
        match len {
            0 => panic!("Cannot write 0 bytes"),
            1 => self.buf_out.write_all(&self.buf_one_byte),
            2 => self.buf_out.write_all(&self.buf_two_byte),
            4 => self.buf_out.write_all(&self.buf_four_byte),
            _ => self.buf_out.write_all(&vec![0u8; len])
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
        self.buf_out.write_all(&self.buf_one_byte).unwrap();
    }

    /**
     * Writes a u8 to the buffer.
     */
    fn write_u8(&mut self, input: u8) {
        self.buf_one_byte[0] = input;
        self.buf_out.write_all(&self.buf_one_byte).unwrap();
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
        self.buf_out.write_all(&self.buf_two_byte).unwrap();
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
        self.buf_out.write_all(&self.buf_four_byte).unwrap();
    }

    /**
     * Writes a string to the buffer.
     * This does not write the length of the string or any padding required after it.
     */
    fn write_str(&mut self, input: &str) {
        match input.len() {
            0 => (),
            _ => self.buf_out.write_all(input.as_bytes()).unwrap()
        };
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
}
